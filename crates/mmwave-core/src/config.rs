use crate::devices::DeviceConfig;

#[derive(PartialEq, Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Configuration {
    pub descriptors: Vec<DeviceConfig>,
}
