pub mod zed;

extern crate libc;

use libc::{c_void, size_t};
use std::os::raw::c_float;

#[repr(C)]
struct Point3D {
    x: c_float,
    y: c_float,
    z: c_float,
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

    pub fn free_body_list(body_list: BodyList) {
        panic!("Zed camera feature is not enabled");
    }
}

// Re-export the functions so they can be used directly under the module's namespace.
pub use zed_camera_support::*;

fn main() {
    unsafe {
        init_zed();

        for _ in 0..100 {
            let body_list = poll_body_keypoints();

            println!("Detected {} bodies", body_list.num_bodies);

            let bodies_slice =
                std::slice::from_raw_parts(body_list.bodies, body_list.num_bodies as usize);
            for (i, body) in bodies_slice.iter().enumerate() {
                println!("  Body {}: ", i + 1);

                let keypoints_slice =
                    std::slice::from_raw_parts(body.points, body.num_points as usize);
                for kp in keypoints_slice.iter() {
                    println!("    Keypoint: ({}, {}, {})", kp.x, kp.y, kp.z);
                }
            }

            free_body_list(body_list);

            // Add a delay if necessary
        }

        close_zed();
    }
}
