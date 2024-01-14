use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub trait IntoPointCloud: Serialize + DeserializeOwned {
    fn into_point_cloud(self) -> PointCloud;
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PointCloudLike {
    PointCloud(PointCloud),
    AWRFrame(ti_device::message::Frame),
    ZedCameraFrame,
}

// Merges the clouds into a single, extended pointcloud for each 100ms bucket
pub fn merge_pointclouds(point_clouds: &mut Vec<PointCloud>) -> Vec<PointCloud> {
    const BUCKET_SIZE: u128 = 100; // 100ms in milliseconds
    let mut buckets = std::collections::HashMap::new();

    for mut point_cloud in point_clouds.iter_mut() {
        let bucket_key = point_cloud.time / BUCKET_SIZE;
        buckets
            .entry(bucket_key)
            .and_modify(|bucket: &mut PointCloud| bucket.extend(&mut point_cloud))
            .or_insert_with(|| {
                let mut new_cloud = PointCloud::default();
                new_cloud.time = bucket_key * BUCKET_SIZE;
                new_cloud.extend(&mut point_cloud);
                new_cloud
            });
    }

    buckets.into_iter().map(|(_, cloud)| cloud).collect()
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
    pub time: u128,
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
