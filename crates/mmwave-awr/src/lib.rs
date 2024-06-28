mod connection;
mod error;
mod message;

use async_nats::{
    connection::State,
    jetstream::{
        self,
        kv::{Entry, Store, Watch},
    },
    Client,
};
use async_trait::async_trait;
use connection::Connection;
use egui::{TextEdit, Ui};
use futures::StreamExt;
use mmwave_core::{
    address::ServerAddress,
    config::Configuration,
    devices::DeviceDescriptor,
    message::{Id, Message, Tag, TagsToSubject},
    nats::get_store,
    point::Point,
    pointcloud::PointCloud,
    transform::Transform,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    any::{Any, TypeId},
    fs::File,
    io::Read,
};
use std::{error::Error, fmt::Display, panic, time::Duration};
use tokio::{select, task::yield_now};
use tracing::{debug, error, info, instrument, warn};

#[derive(
    PartialEq, Hash, Eq, Debug, Copy, Clone, serde::Serialize, serde::Deserialize, Default,
)]
pub enum Model {
    #[default]
    AWR1843Boost,
    AWR1843AOP,
}

#[derive(PartialEq, Debug, Clone, Serialize, Default)]
pub struct AwrDescriptor {
    pub serial: String, // Serial id for the USB device (can be found with lsusb, etc)
    pub model: Model,   // Model of the USB device
    pub config: String, // Configuration string to initialize device
    pub config_path: String,
    pub transform: Transform, // Transform of this AWR device
}

#[derive(Deserialize)]
struct AwrDescriptorHelper {
    serial: String,
    model: Model,
    config: Option<String>,
    transform: Transform,
    config_path: Option<String>,
}

impl Eq for AwrDescriptor {}

impl std::hash::Hash for AwrDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.serial.hash(state);
        self.model.hash(state);
        self.config.hash(state);
    }
}

impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Model::AWR1843Boost => f.write_str("AWR1843Boost"),
            Model::AWR1843AOP => f.write_str("AWR1843AOP"),
        }
    }
}

impl Display for AwrDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.model, self.serial)
    }
}

impl<'de> Deserialize<'de> for AwrDescriptor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = AwrDescriptorHelper::deserialize(deserializer)?;

        let mut config_path = "".to_owned();
        let mut config = None;
        if let Some(path) = helper.config_path {
            config_path = path.clone();
            if std::fs::metadata(path.clone()).is_ok() {
                config = Some(std::fs::read_to_string(&path).map_err(serde::de::Error::custom)?);
            }
        }
        if let Some(c) = helper.config {
            config = Some(c);
        };
        let Some(config) = config else {
            return Err(serde::de::Error::custom(
                "Missing 'config' or 'config_path'",
            ));
        };

        Ok(AwrDescriptor {
            serial: helper.serial,
            model: helper.model,
            config,
            config_path,
            transform: helper.transform,
        })
    }
}

#[typetag::serde]
#[async_trait]
impl DeviceDescriptor for AwrDescriptor {
    #[instrument(skip_all, fields(self=%self, id=%id))]
    async fn init(self: Box<Self>, id: Id, address: ServerAddress) {
        if let Err(e) = start_awr(*self, id, address).await {
            error!(error=?e, "Awr closed with error");
        }
    }

    fn clone_boxed(&self) -> Box<dyn DeviceDescriptor> {
        Box::new(self.clone())
    }

    fn title(&self) -> String {
        format!("{}", self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Serial Number:");
            ui.text_edit_singleline(&mut self.serial);
        });
        ui.horizontal(|ui| {
            ui.label("Model:");
            egui::ComboBox::from_label("")
                .selected_text(format!("{:?}", self.model))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.model, Model::AWR1843Boost, "BOOST");
                    ui.selectable_value(&mut self.model, Model::AWR1843AOP, "AOP");
                });
        });
        self.transform.ui(ui);

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.config_path);
                if let Ok(mut file) = File::open(self.config_path.clone()) {
                    if ui.button("load").clicked() {
                        self.config.clear();
                        let _ = file.read_to_string(&mut self.config);
                    }
                } else {
                }
            });
            ui.collapsing("config", |ui| {
                TextEdit::multiline(&mut self.config)
                    .desired_rows(10)
                    .desired_width(ui.available_width())
                    .code_editor()
                    .show(ui);
            });
        });
    }

    fn transform(&self) -> Option<Transform> {
        Some(self.transform.clone())
    }

    fn position(&self) -> Option<Point> {
        Some(self.transform.apply([0.0, 0.0, 0.0].into()).into())
    }
}

#[instrument(skip_all)]
async fn start_awr(
    mut descriptor: AwrDescriptor,
    id: Id,
    address: ServerAddress,
) -> Result<(), Box<dyn Error>> {
    // Connect to the NATS server
    let client = async_nats::connect(address.address().to_string()).await?;
    let jetstream = jetstream::new(client.clone());

    // Listen for config updates on a seperate task
    let store = get_store(jetstream).await?;
    let mut entries = store.watch("config").await?;

    let mut interval = tokio::time::interval(Duration::from_millis(5000));
    loop {
        // verify the client
        if client.connection_state() == State::Disconnected {
            return Err(String::from("lost connection to nats").into());
        }

        if let Err(e) = run_awr(
            &client,
            &store,
            &mut entries,
            descriptor.clone(),
            id,
            address,
        )
        .await
        {
            error!(error=%e, "awr stopped running");
        }
        interval.tick().await;
    }
}

#[instrument(skip_all)]
async fn run_awr(
    client: &Client,
    store: &Store,
    entries: &mut Watch,
    mut descriptor: AwrDescriptor,
    id: Id,
    address: ServerAddress,
) -> Result<(), Box<dyn Error>> {
    // Create a connection to the AWR device
    let mut connection = Connection::try_open(descriptor.serial.clone(), descriptor.model)?;
    connection = connection.send_command(descriptor.config.clone())?;

    loop {
        yield_now().await;
        select! {
             Some(config) = entries.next() => {
                 if let Err(()) = maintain_config(config?, &mut descriptor, id.clone()) {
                     info!("restarting awr device with new config");
                     return Ok(());
                 }
            }
            result = maintain_connection(&mut connection, client, id.clone(), descriptor.transform.clone()) => {
                match result {
                    Ok(_) => {  },
                    Err(e) => {
                        error!("Unable to publish to client");
                        return Err(e);
                    },
                }
            }
        }
    }
}

async fn maintain_connection(
    connection: &mut Connection,
    client: &Client,
    id: Id,
    transform: Transform,
) -> Result<(), Box<dyn Error>> {
    yield_now().await;
    let frame = match connection.read_frame() {
        Ok(frame) => frame,
        Err(e) => match e {
            error::RadarReadError::ParseError(e) => {
                warn!(error=%e, "AWR parse error, this is usually fine");
                return Ok(());
            }
            other => return Err(Box::new(other)),
        },
    };
    let mut message = Message {
        content: mmwave_core::message::MessageContent::PointCloud(
            Into::<PointCloud>::into(frame)
                .points
                .iter_mut()
                .map(|&mut pt| transform.apply(pt.into()).into())
                .collect::<Vec<Point>>()
                .into(),
        ),
        tags: Vec::from([Tag::Pointcloud, Tag::FromId(id)]),
        timestamp: chrono::Utc::now(),
    };
    let subject = message.tags.clone().to_subject();
    let payload = bincode::serialize(&message)?.into();
    client.publish(subject, payload).await?;
    Ok(())
}

fn maintain_config(entry: Entry, descriptor: &mut AwrDescriptor, id: Id) -> Result<(), ()> {
    let Ok(configuration) = serde_json::from_slice::<Configuration>(&entry.value) else {
        return Ok(());
    };

    for mut device_config in configuration.descriptors {
        if device_config.id != id {
            continue;
        }

        let erased_desc = device_config.device_descriptor.as_any();

        let updated_desc = match erased_desc.downcast_ref::<AwrDescriptor>() {
            Some(awr_desc) => awr_desc,
            None => {
                tracing::error!(
                    "Failed to downcast: actual type id = {:?}, expected type id = {:?}",
                    erased_desc.type_id(),
                    TypeId::of::<Box<AwrDescriptor>>()
                );
                continue;
            }
        };

        if descriptor.transform != updated_desc.transform.clone() {
            info!("Updated AWR descriptor transform");
            descriptor.transform = updated_desc.transform.clone();
        }

        if descriptor.config != updated_desc.config {
            info!("Updated AWR descriptor config file");
            debug!(oldConfig=%descriptor.config, newConfig=%updated_desc.config);
            descriptor.config = updated_desc.config.clone();
            return Err(());
        }
    }

    Ok(())
}
