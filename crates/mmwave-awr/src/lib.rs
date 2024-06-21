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
use futures::StreamExt;
use mmwave_core::{
    address::ServerAddress,
    config::Configuration,
    devices::{DeviceConfig, DeviceDescriptor},
    message::{Id, Message, Tag},
    nats::get_store,
    pointcloud::IntoPointCloud,
    transform::Transform,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    any::{Any, TypeId},
    collections::HashSet,
    future,
    io::Read,
};
use std::{error::Error, fmt::Display, ops::Deref, panic, time::Duration};
use tokio::{pin, select, task::yield_now};
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

        let config = if let Some(c) = helper.config {
            c
        } else if let Some(path) = helper.config_path {
            std::fs::read_to_string(&path).map_err(serde::de::Error::custom)?
        } else {
            return Err(serde::de::Error::custom(
                "Missing 'config' or 'config_path'",
            ));
        };

        Ok(AwrDescriptor {
            serial: helper.serial,
            model: helper.model,
            config,
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
            result = maintain_connection(&mut connection, client) => {
                match result {
                    Ok(_) => { info!("ptcld"); },
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
    let message = Message {
        content: mmwave_core::message::MessageContent::PointCloud(frame.into_point_cloud()),
        tags: HashSet::from([Tag::Pointcloud]),
        timestamp: chrono::Utc::now(),
    };
    let payload = bincode::serialize(&message)?.into();
    client.publish("pointcloud.awr", payload).await?;
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
