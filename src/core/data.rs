use super::pointcloud::PointCloud;
use crate::sensors::{awr::message, zed};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Data {
    AWRFrame(message::Frame),
    PointCloud(PointCloud),
    ZedCameraFrame(zed::ZedMessage),
}
