mod zed;

use async_nats::{
    connection::State,
    jetstream::{
        self,
        kv::{Entry, Store, Watch},
    },
    Client,
};
use async_trait::async_trait;
use egui::Ui;
use futures::StreamExt;
use mmwave_core::{
    address::ServerAddress,
    config::Configuration,
    devices::DeviceDescriptor,
    message::{Id, Message, Tag, TagsToSubject},
    nats::get_store,
    point::Point,
    transform::Transform,
};
use serde::{Deserialize, Serialize};
use std::{any::Any, fmt::Display, time::Duration};
use tokio::{select, task::yield_now};
use tracing::{error, info, instrument};
use zed::Zed;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ZedDescriptor {
    pub transform: Transform, // Transform of this Zed device
}

impl Eq for ZedDescriptor {}

impl PartialEq for ZedDescriptor {
    fn eq(&self, other: &Self) -> bool {
        self.transform == other.transform
    }
}

impl Display for ZedDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Zed")
    }
}

#[typetag::serde]
#[async_trait]
impl DeviceDescriptor for ZedDescriptor {
    #[instrument(skip_all, fields(self=%self, id=%id))]
    async fn init(self: Box<Self>, id: Id, address: ServerAddress) {
        if let Err(e) = start_zed(*self, id, address).await {
            error!(error=?e, "Zed closed with error");
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
        self.transform.ui(ui);
    }

    fn transform(&self) -> Option<Transform> {
        Some(self.transform.clone())
    }

    fn position(&self) -> Option<Point> {
        Some(self.transform.apply([0.0, 0.0, 0.0].into()).into())
    }
}

#[instrument(skip_all)]
async fn start_zed(
    mut descriptor: ZedDescriptor,
    id: Id,
    address: ServerAddress,
) -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the NATS server
    let client = async_nats::connect(address.address().to_string()).await?;
    let jetstream = jetstream::new(client.clone());

    // Listen for config updates on a separate task
    let store = get_store(jetstream).await?;
    let mut entries = store.watch("config").await?;

    let mut interval = tokio::time::interval(Duration::from_millis(5000));
    loop {
        // verify the client
        if client.connection_state() == State::Disconnected {
            return Err(String::from("lost connection to nats").into());
        }

        if let Err(e) = run_zed(
            &client,
            &store,
            &mut entries,
            descriptor.clone(),
            id,
            address,
        ).await
        {
            error!(error=%e, "zed stopped running");
        }
        interval.tick().await;
    }
}

#[instrument(skip_all)]
async fn run_zed(
    client: &Client,
    store: &Store,
    entries: &mut Watch,
    mut descriptor: ZedDescriptor,
    id: Id,
    address: ServerAddress,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create a Zed camera instance
    let mut zed = Zed::new();

    loop {
        yield_now().await;
        select! {
            Some(config) = entries.next() => {
                if let Err(()) = maintain_config(config?, &mut descriptor, id.clone()) {
                    info!("updating zed device transform");
                    // No need to restart the Zed device, just update the transform
                }
            }
            result = maintain_connection(&mut zed, client, id.clone(), descriptor.transform.clone()) => {
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
    zed: &mut Zed,
    client: &Client,
    id: Id,
    transform: Transform,
) -> Result<(), Box<dyn std::error::Error>> {
    yield_now().await;
    if let Some(message) = zed.try_read() {
        let points: Vec<(usize, Point)> = message.bodies.iter().enumerate().flat_map(|(i, body)| {
                body.keypoints.iter().map(|&pt| (i, transform.apply(pt.into()).into())).collect::<Vec<(usize, Point)>>()}).collect();
        let labels: Vec<String> = points.clone().iter().map(|(i, p)| format!("zedbody:{}", i)).collect();
        let points = points.iter().map(|(i, p)| *p).collect();
        let labels = Vec::new();
        let message = Message {
            content: mmwave_core::message::MessageContent::PointCloud(
                mmwave_core::pointcloud::PointCloud { 
                    time: chrono::Utc::now(),
                    points,
                    labels
                }
            ),
            tags: vec![Tag::Pointcloud, Tag::FromId(id)],
            timestamp: chrono::Utc::now(),
        };
        let subject = message.tags.clone().to_subject();
        let payload = bincode::serialize(&message)?.into();
        client.publish(subject, payload).await?;
    }
    Ok(())
}

fn maintain_config(entry: Entry, descriptor: &mut ZedDescriptor, id: Id) -> Result<(), ()> {
    let Ok(configuration) = serde_json::from_slice::<Configuration>(&entry.value) else {
        return Ok(());
    };

    for mut device_config in configuration.descriptors {
        if device_config.id != id {
            continue;
        }

        let erased_desc = device_config.device_descriptor.as_any();

        let updated_desc = match erased_desc.downcast_ref::<ZedDescriptor>() {
            Some(zed_desc) => zed_desc,
            None => {
                tracing::error!(
                    "Failed to downcast: actual type id = {:?}, expected type id = {:?}",
                    erased_desc.type_id(),
                    std::any::TypeId::of::<Box<ZedDescriptor>>()
                );
                continue;
            }
        };

        if descriptor.transform != updated_desc.transform.clone() {
            info!("Updated Zed descriptor transform");
            descriptor.transform = updated_desc.transform.clone();
        }
    }

    Ok(())
}
