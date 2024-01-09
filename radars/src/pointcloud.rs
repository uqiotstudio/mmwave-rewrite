use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub trait IntoPointCloud: Serialize + DeserializeOwned {
    fn into_point_cloud(self) -> PointCloud;
}

#[derive(Serialize, Deserialize)]
pub enum PointCloudLike {
    PointCloud(PointCloud),
    AWRFrame(ti_device::message::Frame),
    ZedCameraFrame,
}

impl IntoPointCloud for PointCloudLike {
    fn into_point_cloud(self) -> PointCloud {
        match self {
            PointCloudLike::PointCloud(pc) => pc.into_point_cloud(),
            PointCloudLike::AWRFrame(pc) => pc.into_point_cloud(),
            PointCloudLike::ZedCameraFrame => PointCloud::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PointCloud {
    pub points: Vec<[f32; 4]>, // x, y, z, v
    pub metadata: Vec<PointMetaData>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PointMetaData {
    label: Option<String>,
}

impl PointCloud {
    pub fn extend(&mut self, other: &mut PointCloud) {
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
            points: Vec::new(),
            metadata: Vec::new(),
        }
    }
}
