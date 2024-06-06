pub mod awr;
mod recorder;
pub mod zed;

use crate::core::message::{Destination, Id, Message};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::hash::Hash;
use tokio::{sync::broadcast, task::JoinHandle};

use self::{
    awr::{Awr, AwrDescriptor},
    recorder::{Recorder, RecorderDescriptor},
    zed::{Zed, ZedDescriptor},
};

pub enum Device {
    AWR(Awr),
    Recorder(Recorder),
    Zed(Zed),
}

impl Device {
    pub fn channel(&mut self) -> (broadcast::Sender<Message>, broadcast::Receiver<Message>) {
        match self {
            Device::AWR(awr) => awr.channel(),
            Device::Recorder(recorder) => recorder.channel(),
            Device::Zed(zed) => zed.channel(),
        }
    }

    pub fn start(self) -> JoinHandle<()> {
        match self {
            Device::AWR(awr) => awr.start(),
            Device::Recorder(recorder) => recorder.start(),
            Device::Zed(zed) => zed.start(),
        }
    }
}

#[derive(PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct DeviceConfig {
    pub id: Id,
    pub device_descriptor: DeviceDescriptor,
}

impl Hash for DeviceConfig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.device_descriptor.hash(state);
    }
}

impl DeviceConfig {
    pub fn title(&self) -> String {
        self.device_descriptor.title()
    }

    pub fn init(self) -> Device {
        match self.device_descriptor {
            DeviceDescriptor::AWR(awr_descriptor) => Device::AWR(Awr::new(self.id, awr_descriptor)),
            DeviceDescriptor::Recorder(recorder_descriptor) => {
                Device::Recorder(Recorder::new(self.id, recorder_descriptor))
            }
            DeviceDescriptor::ZED(zed_descriptor) => Device::Zed(Zed::new(self.id, zed_descriptor)),
        }
    }
}

#[derive(Eq, Hash, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub enum DeviceDescriptor {
    AWR(AwrDescriptor),
    Recorder(RecorderDescriptor),
    ZED(ZedDescriptor),
}

impl DeviceDescriptor {
    pub fn title(&self) -> String {
        match &self {
            DeviceDescriptor::AWR(desc) => {
                format!("{}@{}", desc.model, desc.serial)
            }
            DeviceDescriptor::Recorder(recorder) => {
                format!("recorder")
            }
            DeviceDescriptor::ZED(zed) => {
                format!("ZED Camera")
            }
        }
    }
}
