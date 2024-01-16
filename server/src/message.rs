use radars::config::Configuration;
use radars::pointcloud::PointCloud;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    ConfigMessage(ConfigMessage),
    PointCloudMessage(PointCloudMessage),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ConfigMessage {
    pub config: Configuration,
    pub changed: Vec<usize>, // Indices for the changed elements!
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PointCloudMessage {
    pub pointclouds: Vec<PointCloud>,
}
