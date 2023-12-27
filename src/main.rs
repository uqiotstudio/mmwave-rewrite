mod connection;
mod error;
mod message;
mod radar;
use radar::RadarDescriptor;
use std::time::Instant;

use crate::{error::RadarReadError, radar::Transform};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the test radar descriptor
    let radar_descriptor = RadarDescriptor {
        serial: "R2091049".to_owned(),
        model: radar::Model::AWR1843Boost,
        config: "./profile_AWR1843B.cfg".to_owned(),
        transform: Transform {},
    };

    dbg!(&radar_descriptor);

    // Consumes radar_descriptor to produce a valid RadarInstance
    let mut radar_instance = radar_descriptor.clone().try_initialize().unwrap();

    dbg!(&radar_instance);

    let mut i = 0;
    loop {
        dbg!(i);
        i += 1;
        let now = Instant::now();
        match radar_instance.read_frame() {
            Ok(frame) => {
                // Got a frame and continue reading
                dbg!(now.elapsed().as_millis());
            }
            Err(RadarReadError::ParseError(e)) => {
                dbg!(e);
            }
            Err(RadarReadError::Disconnected)
            | Err(RadarReadError::NotConnected)
            | Err(RadarReadError::Timeout) => {
                eprintln!("Connection to radar lost, attempting reconnection");
                radar_instance = radar_instance.reconnect();
            }
        };
    }
}
