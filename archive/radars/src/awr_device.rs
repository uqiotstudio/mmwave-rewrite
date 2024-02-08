use std::{
    error::Error,
    time::{SystemTime, UNIX_EPOCH},
};
use ti_device::{error::RadarReadError, message::Frame, message::TlvBody, radar::Awr};

use crate::{
    pointcloud::{IntoPointCloud, PointCloud, PointCloudLike, PointMetaData},
    pointcloud_provider::PointCloudProvider,
};

// Simply converts any AWR frame into a pointcloud, dropping all the extra info :(
impl IntoPointCloud for Frame {
    fn into_point_cloud(self) -> crate::pointcloud::PointCloud {
        // dbg!(self.frame_header.time);
        for tlv in self.frame_body.tlvs {
            if let TlvBody::PointCloud(pc) = tlv.tlv_body {
                return PointCloud {
                    time: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|t| t.as_millis())
                        .unwrap_or(0),
                    // time: self.frame_header.time as u128,
                    metadata: vec![
                        PointMetaData {
                            label: Some("mmwave".to_owned()),
                            device: Some(format!("{}", self.frame_header.version))
                        };
                        pc.len()
                    ],
                    points: pc,
                    ..Default::default()
                };
            }
        }
        PointCloud::default()
    }
}

impl PointCloudProvider for Awr {
    fn try_read(&mut self) -> Result<crate::pointcloud::PointCloudLike, Box<dyn Error + Send>> {
        match self.read_frame() {
            Ok(frame) => Ok(PointCloudLike::AWRFrame(frame)),
            Err(RadarReadError::ParseError(e)) => {
                eprintln!("Parse error reading frame, {:?}", e);
                // A parse error isnt serious enough to warrant a restart, so just let us continue with no points for a frame
                Ok(PointCloudLike::PointCloud(PointCloud::default()))
            }
            // Any other errors should never happen and will require reinitialization
            Err(e) => Err(Box::new(e)),
        }
    }
}
