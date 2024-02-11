pub mod awr;
pub mod playback;
pub mod zed;

use std::hash::Hash;

use serde::{Deserialize, Serialize};

use crate::core::{pointcloud::PointCloudLike, transform::Transform};

use self::awr::error::{RadarInitError, RadarReadError};
use self::awr::AwrDescriptor;
use self::playback::PlaybackDescriptor;
use self::zed::ZedDescriptor;

pub trait Sensor: Send {
    fn try_read(&mut self) -> Result<PointCloudLike, SensorReadError>;
}

#[derive(PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct SensorConfig {
    pub machine_id: usize,
    pub sensor_descriptor: SensorDescriptor,
    pub transform: Transform,
}

impl Hash for SensorConfig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.machine_id.hash(state);
        self.sensor_descriptor.hash(state);
    }
}

impl SensorConfig {
    pub fn title(&self) -> String {
        self.sensor_descriptor.title()
    }

    /// Attempts to start the sensor described by the sensor config
    /// On success: returns a `Box<dyn Sensor>` which can be read from
    pub fn try_start(&self) -> Result<Box<dyn Sensor>, SensorInitError> {
        self.sensor_descriptor.try_initialize()
    }
}

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub enum SensorDescriptor {
    AWR(AwrDescriptor),
    ZED(ZedDescriptor),
    Playback(PlaybackDescriptor),
}

#[derive(Debug)]
pub enum SensorReadError {
    Failed,
    DeviceFailure,
    RadarError(RadarReadError),
}

#[derive(Debug)]
pub enum SensorInitError {
    InvalidTransform,
    DeviceFailure,
    RadarError(RadarInitError),
}

impl std::hash::Hash for SensorDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Uses the title as the hash.
        self.title().hash(state);
        state.finish();
    }
}

impl SensorDescriptor {
    pub fn title(&self) -> String {
        match &self {
            SensorDescriptor::AWR(desc) => {
                format!("{}@{}", desc.model, desc.serial)
            }
            SensorDescriptor::ZED(_desc) => {
                format! {"ZED Camera"}
            }
            SensorDescriptor::Playback(desc) => {
                format!("Playback {}", desc.path)
            }
        }
    }

    pub fn try_initialize(&self) -> Result<Box<dyn Sensor>, SensorInitError> {
        Ok(match self {
            SensorDescriptor::AWR(descriptor) => {
                Box::new(descriptor.clone().try_initialize().map_err(|e| {
                    dbg!(&e);
                    e.into()
                })?)
            }
            SensorDescriptor::ZED(descriptor) => Box::new(descriptor.clone().try_initialize()?),
            SensorDescriptor::Playback(descriptor) => {
                Box::new(descriptor.clone().try_initialize()?)
            }
        })
    }
}
