extern crate libc;

use serde::{Deserialize, Serialize};

use libc::size_t;
use std::{
    os::raw::c_float,
    time::{SystemTime, UNIX_EPOCH},
};

#[repr(C)]
#[derive(Serialize, Deserialize, Debug)]
pub struct Point3D {
    pub x: c_float,
    pub y: c_float,
    pub z: c_float,
}

#[repr(C)]
struct Body {
    num_points: size_t,
    points: *mut Point3D,
}

#[repr(C)]
pub struct BodyList {
    num_bodies: size_t,
    bodies: *mut Body,
}

#[cfg(feature = "zed_camera")]
mod zed_camera_support {
    use super::*;
    #[link(name = "zed_interface_lib")]
    extern "C" {
        pub fn init_zed();
        pub fn poll_body_keypoints() -> BodyList;
        pub fn close_zed();
        pub fn free_body_list(body_list: BodyList);
    }
}

#[cfg(not(feature = "zed_camera"))]
mod zed_camera_support {
    use super::*;
    pub fn init_zed() {
        panic!("Zed camera feature is not enabled");
    }

    pub fn poll_body_keypoints() -> BodyList {
        panic!("Zed camera feature is not enabled");
    }

    pub fn close_zed() {
        panic!("Zed camera feature is not enabled");
    }

    pub fn free_body_list(_body_list: BodyList) {
        panic!("Zed camera feature is not enabled");
    }
}

// Re-export the functions so they can be used directly under the module's namespace.
pub use zed_camera_support::*;

use crate::core::pointcloud::{IntoPointCloud, PointCloud, PointCloudLike, PointMetaData};

use super::{Sensor, SensorInitError, SensorReadError};

#[derive(Serialize, Deserialize, Debug)]
pub struct ZedMessage {
    pub bodies: Vec<BodyInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BodyInfo {
    pub keypoints: Vec<Point3D>,
}

pub struct Zed {}

impl Zed {
    pub fn new() -> Zed {
        // Initialize any necessary resources here.
        unsafe {
            init_zed();
        }
        Zed {
            // Initialize your struct fields here.
        }
    }

    pub fn try_read(&mut self) -> Option<ZedMessage> {
        let body_list = unsafe { poll_body_keypoints() };
        let mut bodies = Vec::new();

        let bodies_slice =
            unsafe { std::slice::from_raw_parts(body_list.bodies, body_list.num_bodies as usize) };
        for body in bodies_slice.iter() {
            let keypoints_slice =
                unsafe { std::slice::from_raw_parts(body.points, body.num_points as usize) };
            let Some(keypoints) = keypoints_slice.iter().nth(1).map(|kp| {
                vec![Point3D {
                    x: kp.x,
                    y: kp.z,
                    z: kp.y,
                }]
            }) else {
                continue;
            };

            bodies.push(BodyInfo { keypoints });
        }

        unsafe {
            free_body_list(body_list);
        }

        Some(ZedMessage { bodies })
    }
}

impl Drop for Zed {
    fn drop(&mut self) {
        // Clean up any resources when the struct is dropped.
        unsafe {
            close_zed();
        }
    }
}

#[derive(Debug, Hash, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct ZedDescriptor {}

impl ZedDescriptor {
    pub fn try_initialize(self) -> Result<Zed, SensorInitError> {
        #[cfg(not(feature = "zed_camera"))]
        {
            eprintln!("zed_camera must be enabled to use zed camera sensor");
            return Err(SensorInitError::DeviceFailure);
        }
        Ok(Zed::new())
    }
}

impl IntoPointCloud for ZedMessage {
    fn into_point_cloud(self) -> crate::core::pointcloud::PointCloud {
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

impl Sensor for Zed {
    fn try_read(&mut self) -> Result<PointCloudLike, SensorReadError> {
        self.try_read()
            .map(|m| PointCloudLike::ZedCameraFrame(m))
            .ok_or(SensorReadError::Benign)
    }
}
