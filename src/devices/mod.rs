pub mod awr;

use crate::core::message::{Destination, Id, Message};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::hash::Hash;
use tokio::{sync::broadcast, task::JoinHandle};

use self::awr::{Awr, AwrDescriptor};

pub trait Device: Send {
    fn channel(&mut self) -> (broadcast::Sender<Message>, broadcast::Receiver<Message>);

    fn configure(&mut self, config: DeviceConfig);

    fn destinations(&mut self) -> HashSet<Destination>;

    fn start(&mut self) -> JoinHandle<()>;
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

    pub fn init(&self) -> Box<dyn Device> {
        Box::new(match self.device_descriptor {
            DeviceDescriptor::AWR(_) => Awr::default(),
            // DeviceDescriptor::ZED(_) => todo!(),
            // DeviceDescriptor::Playback(_) => todo!(),
        })
    }
}

#[derive(Eq, Hash, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub enum DeviceDescriptor {
    AWR(AwrDescriptor),
    // ZED(ZedDescriptor),
    // Playback(PlaybackDescriptor),
}

impl DeviceDescriptor {
    pub fn title(&self) -> String {
        match &self {
            DeviceDescriptor::AWR(desc) => {
                format!("{}@{}", desc.model, desc.serial)
            } // DeviceDescriptor::ZED(_desc) => {
              //     format! {"ZED Camera"}
              // }
              // DeviceDescriptor::Playback(desc) => {
              //     format!("Playback {}", desc.path)
              // }
        }
    }
}
