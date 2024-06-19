use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;
use tracing::{error, info, instrument, Instrument};

use crate::core::message::{Destination, Id, Message};
use crate::core::pointcloud::{IntoPointCloud, PointCloud};

use super::{DeviceConfig, DeviceDescriptor};

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecorderDescriptor {
    out_path: String,
}

#[derive(Debug)]
pub struct Recorder {
    id: Id,
    descriptor: RecorderDescriptor,
    inbound: broadcast::Sender<Message>,
    outbound: broadcast::Sender<Message>,
}

impl std::hash::Hash for RecorderDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.out_path.hash(state);
    }
}

impl Eq for RecorderDescriptor {}

impl Recorder {
    pub fn new(id: Id, descriptor: RecorderDescriptor) -> Self {
        Self {
            id,
            descriptor,
            inbound: broadcast::channel(100).0,
            outbound: broadcast::channel(100).0,
        }
    }

    #[instrument(skip_all)]
    pub fn channel(&mut self) -> (broadcast::Sender<Message>, broadcast::Receiver<Message>) {
        (self.inbound.clone(), self.outbound.subscribe())
    }

    #[instrument(skip_all)]
    pub fn start(self) -> JoinHandle<()> {
        let Self {
            inbound,
            outbound,
            id,
            descriptor,
        } = self;
        let mut inbound_rx = inbound.subscribe();
        let descriptor = Arc::new(Mutex::new(descriptor));

        // register data messages to come to this id
        let _ = outbound.send(Message {
            content: crate::core::message::MessageContent::RegisterId(
                HashSet::from([id]),
                HashSet::from([Destination::DataListener]),
            ),
            destination: HashSet::from([Destination::Server, Destination::Id(id.to_machine())]),
            timestamp: chrono::Utc::now(),
        });

        // Listen for messages. Specifically interested in DataMessages
        let t1 = tokio::task::spawn(
            {
                async move {
                    let outbound = outbound.subscribe();
                    let mut point_clouds = Vec::new();
                    while let Ok(message) = inbound_rx.recv().await {
                        match message.content {
                            crate::core::message::MessageContent::DataMessage(data) => {
                                let point_cloud = data.into_point_cloud();
                                point_clouds.push(point_cloud);
                                if point_clouds.len() > 100 {
                                    tokio::task::spawn(
                                        save_pointcloud(
                                            descriptor.lock().await.out_path.clone(),
                                            point_clouds,
                                        )
                                        .instrument(tracing::Span::current()),
                                    );
                                    point_clouds = Vec::new();
                                }
                            }
                            crate::core::message::MessageContent::ConfigMessage(config) => {
                                for DeviceConfig {
                                    id: new_id,
                                    device_descriptor,
                                } in config.descriptors
                                {
                                    if id != new_id {
                                        continue;
                                    }
                                    let DeviceDescriptor::Recorder(awr_desc) = device_descriptor
                                    else {
                                        continue;
                                    };

                                    *descriptor.lock().await = awr_desc;
                                }
                            }
                            other => {
                                error!("unsupported message");
                            }
                        }
                    }
                }
            }
            .instrument(tracing::Span::current()),
        );
        t1
    }
}

#[instrument(skip(point_clouds))]
async fn save_pointcloud(file_path: String, point_clouds: Vec<PointCloud>) {
    info!("Saving to pointcloud");
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .open(&file_path)
        .unwrap_or_else(|_| File::create(&file_path).unwrap());
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Unable to read file");
    let mut existing_data: Vec<PointCloud> =
        serde_json::from_str(&contents).unwrap_or_else(|_| Vec::new());

    let len = point_clouds.len();
    existing_data.extend(point_clouds);

    // Reserialize and write back
    let serialized_data =
        serde_json::to_string_pretty(&existing_data).expect("Unable to serialize data");
    let mut file = File::create(&file_path).expect("Unable to create file");
    file.write_all(serialized_data.as_bytes())
        .expect("Unable to write data");

    info!("Updated out.json with {} items", len);
}
