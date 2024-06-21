pub mod config;
pub mod connection;
pub mod error;
pub mod manager;
pub mod message;
pub mod radar;
use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use crate::{message::FrameHeader, radar::Transform};
use config::RadarConfiguration;
use manager::Manager;
use radar::AwrDescriptor;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    // Initialize the test radar descriptor
    let radar_descriptor = AwrDescriptor {
        serial: "R2091049".to_owned(),
        model: radar::Model::AWR1843Boost,
        config: include_str!("../../profile_AWR1843B.cfg").to_owned(),
    };

    let radar_descriptor2 = AwrDescriptor {
        serial: "00E23FD7".to_owned(),
        model: radar::Model::AWR1843AOP,
        config: include_str!("../../profile_AWR1843_AOP.cfg").to_owned(),
    };

    let radar_config = RadarConfiguration {
        descriptors: vec![radar_descriptor, radar_descriptor2],
    };

    let mut manager_orig = Arc::new(Mutex::new(Manager::new()));

    let manager = manager_orig.clone();
    tokio::task::spawn(async move {
        manager.lock().await.set_config(radar_config);
    })
    .await;

    assert_send::<Manager>();

    let manager = manager_orig.clone();
    // tokio::task::spawn(async move {
    //     manager.lock().await.receive().await;
    // })
    // .await;

    loop {
        dbg!(manager.lock().await.receive().await);
    }
    // let mut i = 0;
    // loop {
    //     i += 1;
    //     // thread::sleep(Duration::from_secs(1));
    //     let frames = manager.receive().await.len();
    //     if i > 100 {
    //         i = 0;
    //         manager.reload();
    //     }
    // }
}

fn assert_send<T: Send>() {}
