use crate::devices::{awr::message::Frame, zed::ZedMessage};

use super::pointcloud::PointCloud;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Data {
    AWRFrame(Frame),
    PointCloud(PointCloud),
    ZedCameraFrame(ZedMessage),
}
