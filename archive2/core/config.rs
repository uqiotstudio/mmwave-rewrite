use crate::devices::DeviceConfig;

#[derive(PartialEq, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Configuration {
    pub descriptors: Vec<DeviceConfig>,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            descriptors: Vec::new(),
        }
    }
}