use crate::sensors::{SensorConfig};

#[derive(PartialEq, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Configuration {
    pub descriptors: Vec<SensorConfig>,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            descriptors: Vec::new(),
        }
    }
}
