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
struct BodyList {
    num_bodies: size_t,
    bodies: *mut Body,
}

#[link(name="zed_interface_lib")]
extern "C" {
    fn init_zed();
    fn poll_body_keypoints() -> BodyList;
    fn close_zed();
    fn free_body_list(body_list: BodyList);
}

fn main() {
    unsafe {
        init_zed();

        for _ in 0..100 {
            let body_list = poll_body_keypoints();

            println!("Detected {} bodies", body_list.num_bodies);

            let bodies_slice = std::slice::from_raw_parts(body_list.bodies, body_list.num_bodies as usize);
            for (i, body) in bodies_slice.iter().enumerate() {
                println!("  Body {}: ", i + 1);

                let keypoints_slice = std::slice::from_raw_parts(body.points, body.num_points as usize);
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
