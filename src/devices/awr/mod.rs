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
use crate::core::pointcloud::IntoPointCloud;
use crate::core::pointcloud::PointCloud;
use crate::core::pointcloud::PointMetaData;
use crate::core::transform::Transform;
use async_trait::async_trait;
use serde::Deserialize;
use serde::Deserializer;
use std::collections::HashSet;
use std::fmt::Display;
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
    id: Option<Id>,
    descriptor_buffer: Option<AwrDescriptor>,
    connection: Option<Connection>,
    inbound: broadcast::Sender<Message>,
    outbound: broadcast::Sender<Message>,
}

impl Default for Awr {
    fn default() -> Self {
        Awr {
            id: None,
            descriptor_buffer: None,
            connection: None,
            inbound: broadcast::channel(100).0,
            outbound: broadcast::channel(100).0,
        }
    }
}

#[async_trait]
impl Device for Awr {
    fn channel(&mut self) -> (broadcast::Sender<Message>, broadcast::Receiver<Message>) {
        (self.inbound.clone(), self.outbound.subscribe())
    }

    #[instrument]
    fn configure(&mut self, config: DeviceConfig) {
        let span = span!(Level::INFO, "configure", config = tracing::field::Empty);
        let _enter = span.enter();

        span.record("config", &tracing::field::debug(&config));

        let DeviceDescriptor::AWR(descriptor) = config.device_descriptor else {
            error!("Received invalid config (required DeviceDescriptor::AWR)");
            return;
        };
        self.id = Some(config.id);
        self.descriptor_buffer = Some(descriptor);
    }

    fn destinations(&mut self) -> HashSet<Destination> {
        HashSet::from([Destination::Sensor])
    }

    #[instrument]
    fn start(&mut self) -> JoinHandle<()> {
        let mut self2 = std::mem::take(self);
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(1000));
            let span = span!(
                Level::INFO,
                "sensor running",
                descriptor = tracing::field::Empty,
                error = tracing::field::Empty
            );
            let _enter = span.enter();

            let mut descriptor = None;
            loop {
                interval.tick().await;

                // set up the connection, according to our config
                let descriptor = {
                    if let Some(new_descriptor) = self2.descriptor_buffer.clone() {
                        self2.descriptor_buffer = None;
                        descriptor = Some(new_descriptor.clone());
                        span.record("descriptor", &tracing::field::debug(&descriptor));
                        info!("Updated descriptor");
                        new_descriptor
                    } else if let Some(new_descriptor) = descriptor.clone() {
                        new_descriptor
                    } else {
                        continue;
                    }
                };

                // Initialize the radar
                let connection = match Connection::try_open(descriptor.serial, descriptor.model) {
                    Ok(connection) => connection,
                    Err(err) => {
                        error!("Radar init error");
                        debug!(err=?err, "Radar init error");
                        // TODO this is where we might send a reboot message to parent
                        // depending on the erorr of course
                        continue;
                    }
                };

                let mut connection = match connection.send_command(descriptor.config) {
                    Err(e) => {
                        span.record("error", &tracing::field::debug(&e));
                        error!("Failed to send config to radar");
                        continue;
                    }
                    Ok(connection) => connection,
                };

                loop {
                    let frame = match connection.read_frame() {
                        Err(e) => {
                            span.record("error", &tracing::field::debug(&e));
                            error!("Failed to read from radar");
                            continue;
                        }
                        Ok(frame) => frame,
                    };

                    let r = self2.outbound.send(Message {
                        content: crate::core::message::MessageContent::DataMessage(
                            Data::PointCloud(frame.into_point_cloud()),
                        ),
                        destination: HashSet::from([Destination::Visualiser]),
                        timestamp: chrono::Utc::now(),
                    });

                    match r {
                        Ok(_) => {}
                        Err(e) => {
                            error!(error=?e, "awr device outbound channel closed, this should be impossible");
                            panic!("awr device outbound channel closed");
                        }
                    };
                }
            }
        })
    }

    // fn try_read(&mut self) -> Result<Data, SensorReadError> {
    //     match self.read_frame() {
    //         Ok(frame) => Ok(Data::AWRFrame(frame)),
    //         Err(RadarReadError::ParseError(e)) => {
    //             eprintln!("Parse error reading frame, {:?}", e);
    //             // A parse error isnt serious enough to warrant a restart, so just let us continue with no points for a frame
    //             Ok(Data::PointCloud(PointCloud::default()))
    //         }
    //         // Any other errors should never happen and will require reinitialization
    //         Err(e) => Err(e.into()),
    //     }
    // }
}

impl Awr {
    // fn reset_device(&mut self) -> Result<(), Box<dyn Error>> {
    //     for device in rusb::devices()?.iter() {
    //         let device_desc = match device.device_descriptor() {
    //             Ok(dd) => dd,
    //             Err(_) => continue,
    //         };

    //         let mut handle = match device.open() {
    //             Ok(h) => h,
    //             Err(_) => continue,
    //         };

    //         // dbg!(&handle);

    //         let serial_number = match handle.read_serial_number_string_ascii(&device_desc) {
    //             Ok(sn) => sn,
    //             Err(_) => continue,
    //         };

    //         if serial_number == self.descriptor.serial {
    //             handle.reset();
    //         }
    //     }

    //     Ok(())
    // }

    // pub fn reconnect(mut self) -> Self {
    //     // Restart the device
    //     let Ok(()) = self.reset_device() else {
    //         return self;
    //     };

    //     // Attempt to craete a new instance, else give self back, still broken
    //     self.descriptor.clone().try_initialize().unwrap_or(self)
    // }

    // pub fn read_frame(&mut self) -> Result<Frame, RadarReadError> {
    //     let frame = match self.connection.read_frame() {
    //         Ok(frame) => frame,
    //         Err(e) => return Err(e),
    //     };
    //     Ok(frame)
    // }
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
