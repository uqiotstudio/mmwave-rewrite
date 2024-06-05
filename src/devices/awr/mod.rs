mod connection;
pub mod error;
pub mod message;

use self::connection::Connection;
use self::message::Frame;
use self::message::TlvBody;
use super::Device;
use super::DeviceConfig;
use super::DeviceDescriptor;
use crate::core::data::Data;
use crate::core::message::Destination;
use crate::core::message::Id;
use crate::core::message::Message;
use crate::core::message::MessageContent;
use crate::core::pointcloud::IntoPointCloud;
use crate::core::pointcloud::PointCloud;
use crate::core::pointcloud::PointMetaData;
use crate::core::transform::Transform;
use async_trait::async_trait;
use serde::Deserialize;
use serde::Deserializer;
use tokio::select;
use tokio::time::interval;
use std::collections::HashSet;
use std::fmt::Display;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::instrument;
use tracing::span;
use tracing::warn;
use tracing::Instrument;
use tracing::Level;

#[derive(
    PartialEq, Hash, Eq, Debug, Copy, Clone, serde::Serialize, serde::Deserialize, Default,
)]
pub enum Model {
    #[default]
    AWR1843Boost,
    AWR1843AOP,
}

#[derive(PartialEq, Debug, Clone, serde::Serialize, Default)]
pub struct AwrDescriptor {
    pub serial: String, // Serial id for the USB device (can be found with lsusb, etc)
    pub model: Model,   // Model of the USB device
    pub config: String, // Configuration string to initialize device
    pub transform: Transform, // Transform of this AWR device
}

impl Eq for AwrDescriptor {}

impl std::hash::Hash for AwrDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.serial.hash(state);
        self.model.hash(state);
        self.config.hash(state);
    }
}

#[derive(Deserialize)]
struct AwrDescriptorHelper {
    serial: String,
    model: Model,
    config: Option<String>,
    transform: Transform,
    config_path: Option<String>,
}

impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Model::AWR1843Boost => f.write_str("AWR1843Boost"),
            Model::AWR1843AOP => f.write_str("AWR1843AOP"),
        }
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

#[derive(Debug)]
pub struct Awr {
    id: Id,
    descriptor: AwrDescriptor,
    inbound: broadcast::Sender<Message>,
    outbound: broadcast::Sender<Message>,
}

impl Awr {
    pub fn new(id: Id, descriptor: AwrDescriptor) -> Self {
        Awr { 
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
        let Self {inbound, outbound, id, descriptor } = self;
        let mut inbound_rx = inbound.subscribe();

        let descriptor = Arc::new(Mutex::new(descriptor));
        let connected = Arc::new(std::sync::atomic::AtomicBool::new(false));
  
        let mut t1 = tokio::task::spawn({
            let descriptor = descriptor.clone();
            let connected= connected.clone();
            async move {
                while let Ok(message) = inbound_rx.recv().await {
                    match message.content {
                        MessageContent::ConfigMessage(config) => {
                            info!("awr received config update");
                            for DeviceConfig { id: new_id, device_descriptor } in config.descriptors {
                                if id != new_id {
                                    continue;
                                }
                                let DeviceDescriptor::AWR(awr_desc) = device_descriptor else {
                                    continue;
                                };
                                let mut descriptor = descriptor.lock().await;
                                if descriptor.config != awr_desc.config || descriptor.model != awr_desc.model || descriptor.serial != awr_desc.serial {
                                    // if anything other than the transform is different, we have to disconnect and reconnect
                                    connected.store(false, Ordering::SeqCst);
                                    info!("changes to config require a restart");
                                }
                                *descriptor = awr_desc;
                            }
                        },
                        _other => {
                            error!("Received unsupported message");
                        }
                    }
                }
            }
        }).instrument(tracing::Span::current());

        let mut t2 = tokio::task::spawn({
            let mut interval = tokio::time::interval(Duration::from_millis(1000));
            async move {            
                let mut connection ;
                loop {
                    interval.tick().await;
                    let descriptor = descriptor.lock().await;

                    let transform = descriptor.transform.clone();

                    connection = match Connection::try_open(descriptor.clone().serial, descriptor.model) {
                            Ok(connection) => connection,
                            Err(error) => {
                                error!(error=?error, "unable to establish connection");
                                continue;
                        },
                    };

                    connection = match connection.send_command(descriptor.config.clone()) {
                        Err(e) => {
                            error!("Failed to send config to radar");
                            continue;
                        }
                        Ok(connection) => connection,
                    };

                    std::mem::drop(descriptor);
                    connected.store(true, Ordering::SeqCst);

                    while connected.load(Ordering::SeqCst) {
                        tokio::task::yield_now().await;
                        let frame = match connection.read_frame() {
                            Err(e) => {
                                error!("Failed to read from radar");
                                continue;
                            }
                            Ok(frame) => frame,
                        };

                        let mut pointcloud = frame.into_point_cloud();
                        pointcloud.points = pointcloud
                            .points
                            .iter_mut()
                            .map(|pt| {
                                let transformed = transform.apply([pt[0], pt[1], pt[2]]);
                                [transformed[0], transformed[1], transformed[2], pt[3]]
                            })
                            .collect();

                        let message = Message {
                            content: crate::core::message::MessageContent::DataMessage(
                                Data::PointCloud(pointcloud),
                            ),
                            destination: HashSet::from([Destination::DataListener, Destination::Visualiser]),
                            timestamp: chrono::Utc::now(),
                        };

                        let r = outbound.send(message);

                        match r {
                            Ok(_) => {}
                            Err(e) => {
                                error!(error=?e, "awr device outbound channel closed, this should be impossible");
                                panic!("awr device outbound channel closed");
                            }
                        };
                    }

                    std::mem::drop(connection);
                }
            }
        }).instrument(tracing::Span::current());

        tokio::task::spawn(async move{
            let mut t1 = t1.inner_mut();
            let mut t2 = t2.inner_mut();
            select!(
                _ = &mut t1 => {
                    t2.abort()
                }
                _ = &mut t2 => {
                    t1.abort()
                }
            )
        })
    }
}

impl IntoPointCloud for Frame {
    fn into_point_cloud(self) -> PointCloud {
        // dbg!(self.frame_header.time);
        for tlv in self.frame_body.tlvs {
            if let TlvBody::PointCloud(pc) = tlv.tlv_body {
                return PointCloud {
                    time: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|t| t.as_millis())
                        .unwrap_or(0),
                    // time: self.frame_header.time as u128,
                    metadata: vec![
                        PointMetaData {
                            label: Some("mmwave".to_owned()),
                            device: Some(format!("{}", self.frame_header.version))
                        };
                        pc.len()
                    ],
                    points: pc,
                    ..Default::default()
                };
            }
        }
        PointCloud::default()
    }
}
