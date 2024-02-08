use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::sensors::awr::message;
use crate::sensors::zed;

pub trait IntoPointCloud: Serialize + DeserializeOwned {
    fn into_point_cloud(self) -> PointCloud;
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PointCloudLike {
    PointCloud(PointCloud),
    AWRFrame(message::Frame),
    ZedCameraFrame(zed::ZedMessage),
}

impl IntoPointCloud for PointCloudLike {
    fn into_point_cloud(self) -> PointCloud {
        match self {
            PointCloudLike::PointCloud(pc) => pc.into_point_cloud(),
            PointCloudLike::AWRFrame(pc) => pc.into_point_cloud(),
            PointCloudLike::ZedCameraFrame(pc) => pc.into_point_cloud(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PointCloud {
    pub time: u128,
    pub points: Vec<[f32; 4]>, // x, y, z, v
    pub metadata: Vec<PointMetaData>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PointMetaData {
    pub label: Option<String>,
    pub device: Option<String>,
}

impl PointCloud {
    pub fn extend(&mut self, mut other: PointCloud) {
        // Extends this pointcloud with other, consuming it
        self.points.append(&mut other.points);
        self.metadata.append(&mut other.metadata);
    }
}

impl IntoPointCloud for PointCloud {
    fn into_point_cloud(self) -> PointCloud {
        // A pointcloud can in fact be turned into a pointcloud!
        self
    }
}

impl Default for PointCloud {
    fn default() -> Self {
        PointCloud {
            time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
                % 100,
            points: Vec::new(),
            metadata: Vec::new(),
        }
    }
}
