mod message;
mod radar;
use radar::RadarDescriptor;
use std::time::Instant;

use crate::radar::{PortDescriptor, RadarReadResult};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the test radar descriptor
    let radar_descriptor = RadarDescriptor {
        cli_descriptor: PortDescriptor {
            path: "/dev/ttyACM0".to_owned(),
            baud_rate: 115200,
        },
        data_descriptor: PortDescriptor {
            path: "/dev/ttyACM1".to_owned(),
            baud_rate: 921600,
        },
        config_path: "./profile_AWR1843B.cfg".to_owned(),
    };

    dbg!(&radar_descriptor);

    // Consumes radar_descriptor to produce a valid RadarInstance
    let mut radar_instance = radar_descriptor.initialize().unwrap();

    dbg!(&radar_instance);

    radar_instance = radar_instance.write_config().unwrap();

    loop {
        let now = Instant::now();
        match radar_instance.read() {
            RadarReadResult::Success(radar, frame) => {
                radar_instance = radar;
                let elapsed = now.elapsed();
                dbg!(elapsed.as_millis());
                // dbg!(&frame);
            }
            RadarReadResult::Malformed(radar) => {
                radar_instance = radar;
                eprintln!("Hit Malformed Frame, Skipping!");
            }
            e => {
                dbg!(&e);
                break;
            }
        };
    }

    Ok(())
}
