use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub trait IntoPointCloud: Serialize + DeserializeOwned {
    fn into_point_cloud(self) -> PointCloud;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PointCloud {
    pub time: DateTime<Utc>,
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
            time: Utc::now(),
            points: Vec::new(),
            metadata: Vec::new(),
        }
    }
}