extern crate libc;

use serde::{Deserialize, Serialize};

use libc::size_t;
use std::{
    collections::HashSet,
    os::raw::c_float,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    select,
    sync::{broadcast, Mutex},
    task::JoinHandle,
};
use tracing::{error, info};

#[repr(C)]
#[derive(Serialize, Deserialize, Debug, Clone)]
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

use crate::{
    core::{
        data::Data,
        message::{Destination, Id, Message, MessageContent},
    },
    devices::DeviceConfig,
};
use crate::{
    core::{
        pointcloud::{IntoPointCloud, PointCloud, PointMetaData},
        transform::Transform,
    },
    devices::DeviceDescriptor,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ZedMessage {
    pub bodies: Vec<BodyInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BodyInfo {
    pub keypoints: Vec<Point3D>,
}

pub struct Zed {
    id: Id,
    zed_descriptor: ZedDescriptor,
    inbound: broadcast::Sender<Message>,
    outbound: broadcast::Sender<Message>,
}

impl Zed {
    pub fn new(id: Id, zed_descriptor: ZedDescriptor) -> Zed {
        #[cfg(not(feature = "zed_camera"))]
        {
            error!("zed_camera must be enabled to use zed camera sensor");
            panic!("zed_camera must be enabled");
        }
        unsafe {
            init_zed();
        }
        Zed {
            id,
            zed_descriptor,
            inbound: broadcast::channel(100).0,
            outbound: broadcast::channel(100).0,
        }
    }

    pub fn try_read() -> Option<ZedMessage> {
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

    pub fn channel(&mut self) -> (broadcast::Sender<Message>, broadcast::Receiver<Message>) {
        (self.inbound.clone(), self.outbound.subscribe())
    }

    pub fn start(mut self) -> JoinHandle<()> {
        let mut inbound = self.inbound.clone();
        let mut outbound = self.outbound.clone();
        let id = self.id;
        let descriptor = Arc::new(Mutex::new(self.zed_descriptor.clone()));

        let mut t1 = tokio::task::spawn({
            let descriptor = descriptor.clone();
            async move {
                let mut inbound_rx = inbound.subscribe();
                while let Ok(message) = inbound_rx.recv().await {
                    match message.content {
                        MessageContent::ConfigMessage(config) => {
                            info!("zed received config update");
                            for DeviceConfig {
                                id: new_id,
                                device_descriptor,
                            } in config.descriptors
                            {
                                if id != new_id {
                                    continue;
                                }
                                let DeviceDescriptor::ZED(zed_desc) = device_descriptor else {
                                    continue;
                                };
                                let mut descriptor = descriptor.lock().await;
                                *descriptor = zed_desc;
                                info!("updated transform");
                            }
                        }
                        _other => {
                            error!("Received unsupported message");
                        }
                    }
                }
            }
        });

        let mut t2 = tokio::task::spawn({
            let descriptor = descriptor.clone();
            async move {
                loop {
                    tokio::task::yield_now().await;
                    let Ok(frame) = Zed::try_read()
                        .map(|m| Data::ZedCameraFrame(m))
                        .ok_or(SensorReadError::Benign)
                    else {
                        error!("unable to read from zed");
                        continue;
                    };

                    let descriptor = descriptor.lock().await;
                    let mut pointcloud = frame.into_point_cloud();
                    pointcloud.points = pointcloud
                        .points
                        .iter_mut()
                        .map(|pt| {
                            let transformed = descriptor.transform.apply([pt[0], pt[1], pt[2]]);
                            [transformed[0], transformed[1], transformed[2], pt[3]]
                        })
                        .collect();

                    let message = Message {
                        content: MessageContent::DataMessage(Data::PointCloud(pointcloud)),
                        destination: HashSet::from([
                            Destination::DataListener,
                            Destination::Visualiser,
                        ]),
                        timestamp: chrono::Utc::now(),
                    };

                    outbound.send(message);
                }
            }
        });

        let mut t3 = tokio::task::spawn(async move {
            self;
            select!(
                _ = &mut t1 => {
                    t2.abort();
                },
                _ = &mut t2 => {
                    t1.abort();
                }
            );
        });

        t3
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ZedDescriptor {
    transform: Transform,
}

impl Eq for ZedDescriptor {}

impl std::hash::Hash for ZedDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {}
}

#[derive(Debug)]
pub enum SensorReadError {
    Critical,
    Benign,
}

#[derive(Debug)]
pub enum SensorInitError {
    InvalidTransform,
    DeviceFailure,
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
