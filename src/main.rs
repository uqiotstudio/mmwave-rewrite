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

    loop {
        let now = Instant::now();
        match radar_instance.read_frame() {
            (Some(radar), Ok(frame)) => {
                // Got a frame and continue reading
                dbg!(now.elapsed().as_millis());
                radar_instance = radar;
            }
            (None, Ok(frame)) => {
                // The final frame before termination
                dbg!(now.elapsed().as_millis());
                break;
            }
            (Some(radar), Err(RadarReadError::ParseError(e))) => {
                dbg!(e);
                radar_instance = radar;
            }
            (None, Err(RadarReadError::ParseError(e))) => {
                dbg!(e);
                break;
            }
            (_, Err(RadarReadError::Disconnected))
            | (_, Err(RadarReadError::NotConnected))
            | (_, Err(RadarReadError::Timeout)) => {
                // In this event
                eprintln!("Connection to radar lost, attempting reconnection");
                // Attempt a reconnection!
                // dbg!(radar_instance.find_usb_device());
                // dbg!(radar_instance.reset_usb_device());
                // dbg!(radar_instance.connect());
                // radar_instance.write_config();
                radar_instance = radar_descriptor.clone().try_initialize().unwrap();
            }
        };
    }

    Ok(())
}
