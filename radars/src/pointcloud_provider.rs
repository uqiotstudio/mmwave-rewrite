use serde::{Deserialize, Serialize};
use std::error::Error;
use ti_device::radar::AwrDescriptor;

use crate::{pointcloud::PointCloudLike, transform::Transform};

pub trait PointCloudProvider: Send {
    fn try_read(&mut self) -> Result<PointCloudLike, Box<dyn Error + Send>>;
}

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub enum DeviceDescriptor {
    AWR(AwrDescriptor),
}

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct PcPDescriptor {
    pub device: DeviceDescriptor,
    pub transform: Transform,
}

impl PcPDescriptor {
    pub fn try_initialize(&mut self) -> Result<Box<dyn PointCloudProvider>, Box<dyn Error>> {
        Ok(match &mut self.device {
            DeviceDescriptor::AWR(descriptor) => Box::new(
                descriptor
                    .clone()
                    .try_initialize()
                    .map_err(|e| Into::<Box<dyn Error>>::into(e))?,
            ),
        })
    }
}
