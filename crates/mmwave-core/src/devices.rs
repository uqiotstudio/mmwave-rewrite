use async_trait::async_trait;
use egui::Ui;
use serde::{Deserialize, Serialize};
use std::{any::Any, hash::Hash, time::Duration};
use tracing::{info, instrument, warn};

use crate::{address::ServerAddress, message::Id, point::Point, transform::Transform};

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

impl Eq for DeviceConfig {}

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

    pub fn ui(&mut self, ui: &mut Ui) {
        self.id.ui(ui);
        self.device_descriptor.ui(ui);
    }
}

#[typetag::serde(tag = "type", content = "value")]
#[async_trait]
pub trait DeviceDescriptor: Send + Sync + Any {
    async fn init(self: Box<Self>, id: Id, address: ServerAddress);
    fn clone_boxed(&self) -> Box<dyn DeviceDescriptor>;
    fn as_any(&self) -> &dyn Any;
    fn title(&self) -> String {
        "Untitled Device".to_string()
    }
    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Unimplemented config for this descriptor");
        });
    }

    fn transform(&self) -> Option<Transform> {
        None
    }

    /// if the descriptor has a spatial position, return it
    fn position(&self) -> Option<Point> {
        None
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct EmptyDeviceDescriptor;

#[typetag::serde]
#[async_trait]
impl DeviceDescriptor for EmptyDeviceDescriptor {
    #[instrument(skip_all)]
    async fn init(self: Box<Self>, _id: Id, _address: ServerAddress) {
        warn!("Opened Empty Device Descriptor");
        tokio::time::sleep(Duration::from_millis(2000)).await;
        warn!("Closing Empty Device Descriptor");
        return;
    }
    fn clone_boxed(&self) -> Box<dyn DeviceDescriptor> {
        Box::new(self.clone())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
