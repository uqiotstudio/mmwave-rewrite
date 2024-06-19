use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::hash::Hash;

use crate::{address::ServerAddress, message::Id};

#[derive(Serialize, Deserialize)]
pub struct DeviceConfig {
    pub id: Id,
    pub device_descriptor: Box<dyn DeviceDescriptor>,
}

impl Clone for DeviceConfig {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            device_descriptor: self.device_descriptor.clone_boxed(),
        }
    }
}

impl std::fmt::Debug for DeviceConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.id)
    }
}

impl PartialEq for DeviceConfig {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl Hash for DeviceConfig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl DeviceConfig {
    pub fn title(&self) -> String {
        self.device_descriptor.title()
    }

    pub async fn init(self, address: ServerAddress) {
        self.device_descriptor.init(self.id, address).await
    }
}

#[typetag::serde(tag = "type")]
#[async_trait]
pub trait DeviceDescriptor {
    async fn init(self: Box<Self>, id: Id, address: ServerAddress);
    fn clone_boxed(&self) -> Box<dyn DeviceDescriptor>;
    fn title(&self) -> String {
        "Untitled Device".to_string()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
struct EmptyDeviceDescriptor;

#[typetag::serde]
#[async_trait]
impl DeviceDescriptor for EmptyDeviceDescriptor {
    async fn init(self: Box<Self>, _id: Id, _address: ServerAddress) {}
    fn clone_boxed(&self) -> Box<dyn DeviceDescriptor> {
        Box::new(self.clone())
    }
}
