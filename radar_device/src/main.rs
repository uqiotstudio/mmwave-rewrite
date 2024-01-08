pub mod config;
pub mod connection;
pub mod error;
pub mod manager;
pub mod message;
pub mod pointcloud;
pub mod radar;
use std::{
    thread,
    time::{Duration, Instant},
};

use crate::{message::FrameHeader, radar::Transform};
use config::RadarConfiguration;
use manager::Manager;
use radar::RadarDescriptor;

#[tokio::main]
async fn main() {
    // Initialize the test radar descriptor
    let radar_descriptor = RadarDescriptor {
        serial: "R2091049".to_owned(),
        model: radar::Model::AWR1843Boost,
        config: "./profile_AWR1843B.cfg".to_owned(),
        transform: Transform {},
    };

    let radar_descriptor2 = RadarDescriptor {
        serial: "00E23FD7".to_owned(),
        model: radar::Model::AWR1843AOP,
        config: "./profile_AWR1843_AOP.cfg".to_owned(),
        transform: Transform {},
    };

    let radar_config = RadarConfiguration {
        descriptors: vec![radar_descriptor, radar_descriptor2],
    };

    let mut manager = Manager::new();

    manager.set_config(radar_config);

    thread::sleep(Duration::from_secs(1));

    let mut i = 0;
    loop {
        i += 1;
        // thread::sleep(Duration::from_secs(1));
        let frames = manager.receive().await;
        let headers: Vec<&FrameHeader> = frames.iter().map(|f| &f.frame_header).collect();
        dbg!(headers);
        if i > 100 {
            i = 0;
            manager.reload();
        }
    }
}
