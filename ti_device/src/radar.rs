use crate::error::RadarInitError;
use crate::error::RadarReadError;
use crate::{connection::Connection, message::Frame};
use std::error::Error;
use std::{fs::File, io::Read};

#[derive(PartialEq, Eq, Debug, Copy, Clone, serde::Serialize, serde::Deserialize)]
pub enum Model {
    AWR1843Boost,
    AWR1843AOP,
}

#[derive(PartialEq, Eq, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Transform {}

#[derive(PartialEq, Eq, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AwrDescriptor {
    pub serial: String, // Serial id for the USB device (can be found with lsusb, etc)
    pub model: Model,   // Model of the USB device
    pub config: String, // Configuration string to initialize device
}

impl AwrDescriptor {
    pub fn try_initialize(self) -> Result<Awr, RadarInitError> {
        let connection = Connection::try_open(self.serial.to_owned(), self.model)?;

        let config = self.config.clone();

        let connection = connection
            .send_command(config)
            .map_err(|_| RadarInitError::PortUnavailable("CLI_Port".to_owned()))?;

        Ok(Awr {
            descriptor: self,
            connection,
        })
    }
}

#[derive(Debug)]
pub struct Awr {
    descriptor: AwrDescriptor,
    connection: Connection,
}

impl Awr {
    pub fn get_descriptor(&self) -> AwrDescriptor {
        self.descriptor.clone()
    }

    fn reset_device(&mut self) -> Result<(), Box<dyn Error>> {
        // eprintln!("Resetting Serial Device");

        for device in rusb::devices()?.iter() {
            let device_desc = match device.device_descriptor() {
                Ok(dd) => dd,
                Err(_) => continue,
            };

            let mut handle = match device.open() {
                Ok(h) => h,
                Err(_) => continue,
            };

            // dbg!(&handle);

            let serial_number = match handle.read_serial_number_string_ascii(&device_desc) {
                Ok(sn) => sn,
                Err(_) => continue,
            };

            if serial_number == self.descriptor.serial {
                handle.reset();
            }
        }

        Ok(())
    }

    pub fn reconnect(mut self) -> Self {
        // Restart the device
        let Ok(()) = self.reset_device() else {
            return self;
        };

        // Attempt to craete a new instance, else give self back, still broken
        self.descriptor.clone().try_initialize().unwrap_or(self)
    }

    // Err(Box::new(rusb::Error::NoDevice))
    // }

    pub fn read_frame(&mut self) -> Result<Frame, RadarReadError> {
        let frame = match self.connection.read_frame() {
            Ok(frame) => frame,
            Err(e) => return Err(e),
        };
        Ok(frame)
    }
}
