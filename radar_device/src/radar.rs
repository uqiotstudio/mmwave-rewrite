use crate::{connection, error::*};
use crate::{connection::Connection, message::Frame};
use rusb::{Context, UsbContext};
use serialport::SerialPort;
use std::error::Error;
use std::{fs::File, io::Read};

#[derive(Debug, Copy, Clone)]
pub enum Model {
    AWR1843Boost,
    AWR1843AOP,
}

#[derive(Debug, Clone)]
pub struct Transform {}

#[derive(Debug, Clone)]
pub struct RadarDescriptor {
    pub serial: String, // Serial id for the USB device (can be found with lsusb, etc)
    pub model: Model,   // Model of the USB device
    pub config: String, // Configuration path to initialize device
    pub transform: Transform, // The transform to apply to the radar
}

impl RadarDescriptor {
    pub fn try_initialize(self) -> Result<Radar, RadarInitError> {
        let connection = Connection::try_open(self.serial.to_owned(), self.model)?;

        let mut config_file = File::open(&self.config)
            .map_err(|_| RadarInitError::InaccessibleConfig(self.config.clone()))?;

        let mut config = String::new();
        config_file
            .read_to_string(&mut config)
            .map_err(|e| RadarInitError::InaccessibleConfig(e.to_string()))?;

        let connection = connection
            .send_command(config)
            .map_err(|_| RadarInitError::PortUnavailable("CLI_Port".to_owned()))?;

        Ok(Radar {
            descriptor: self,
            connection,
        })
    }
}

#[derive(Debug)]
pub struct Radar {
    descriptor: RadarDescriptor,
    connection: Connection,
}

impl Radar {
    fn reset_device(&mut self) -> Result<(), Box<dyn Error>> {
        eprintln!("Resetting Serial Device");

        for device in rusb::devices()?.iter() {
            let device_desc = match device.device_descriptor() {
                Ok(dd) => dd,
                Err(_) => continue,
            };

            let mut handle = match device.open() {
                Ok(h) => h,
                Err(_) => continue,
            };

            dbg!(&handle);

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
