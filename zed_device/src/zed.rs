
extern crate libc;

use serde::{Deserialize, Serialize};

use libc::{c_void, size_t};
use std::os::raw::c_float;

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
struct BodyList {
    num_bodies: size_t,
    bodies: *mut Body,
}

#[link(name = "zed_interface_lib")]
extern "C" {
    fn init_zed();
    fn poll_body_keypoints() -> BodyList;
    fn close_zed();
    fn free_body_list(body_list: BodyList);
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    pub bodies: Vec<BodyInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BodyInfo {
    pub keypoints: Vec<Point3D>,
}

pub struct Zed {
    // You can add any necessary fields here.
}

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

    pub fn try_read(&mut self) -> Option<Message> {
        let body_list = unsafe { poll_body_keypoints() };
        let mut bodies = Vec::new();

        let bodies_slice =
            unsafe { std::slice::from_raw_parts(body_list.bodies, body_list.num_bodies as usize) };
        for body in bodies_slice.iter() {
            let keypoints_slice =
                unsafe { std::slice::from_raw_parts(body.points, body.num_points as usize) };
            let keypoints = keypoints_slice
                .iter()
                .map(|kp| Point3D {
                    x: kp.x,
                    y: kp.y,
                    z: kp.z,
                })
                .collect();
            bodies.push(BodyInfo { keypoints });
        }

        unsafe {
            free_body_list(body_list);
        }

        Some(Message { bodies })
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


#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ZedDescriptor {
    
}

impl ZedDescriptor {
    pub fn try_initialize(self) -> Result<Zed, Box<dyn std::error::Error>> {
        Ok(Zed {})
    }
}
