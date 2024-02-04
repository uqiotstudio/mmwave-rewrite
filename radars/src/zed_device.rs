use crate::{
    pointcloud::{IntoPointCloud, PointCloud, PointCloudLike, PointMetaData},
    pointcloud_provider::PointCloudProvider,
};
use std::{
    error::Error,
    time::{SystemTime, UNIX_EPOCH},
};
use zed_device::zed::{Message, Zed};

impl IntoPointCloud for Message {
    fn into_point_cloud(self) -> crate::pointcloud::PointCloud {
        let mut points: Vec<[f32; 4]> = Vec::new();
        let mut metadata = Vec::new();

        for body_info in self.bodies {
            for (i, keypoint) in body_info.keypoints.iter().enumerate() {
                // Convert the zed::Point3D to your PointMetaData struct
                let point_metadata = PointMetaData {
                    label: Some(format!("{}", i)), // Set this to appropriate label if needed
                    device: Some("zed".to_owned()), // Set this to appropriate device info if needed
                };
                points.push([keypoint.x, keypoint.y, keypoint.z, 0.0]);
                metadata.push(point_metadata);
            }
        }

        PointCloud {
            time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|t| t.as_millis())
                .unwrap_or(0),
            metadata,
            points,
            ..Default::default()
        }
    }
}

impl PointCloudProvider for Zed {
    fn try_read(&mut self) -> Result<crate::pointcloud::PointCloudLike, Box<dyn Error + Send>> {
        self.try_read()
            .map(|m| PointCloudLike::ZedCameraFrame(m))
            .ok_or(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "")))
    }
}
