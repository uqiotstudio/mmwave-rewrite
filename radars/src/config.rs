use crate::pointcloud_provider::PcPDescriptor;

#[derive(PartialEq, Eq, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Configuration {
    pub descriptors: Vec<PcPDescriptor>,
}
