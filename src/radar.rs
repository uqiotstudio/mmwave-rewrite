use crate::{connection, error::*};
use crate::{connection::Connection, message::Frame};
use rusb::{Context, UsbContext};
use serialport::SerialPort;
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
    // pub fn find_usb_device(&mut self) -> Option<(u16, u16)> {
    //     let context = Context::new().unwrap();
    //     let mut enumerator = Enumerator::new(&context).unwrap();

    //     enumerator.match_subsystem("tty").unwrap();

    //     for device in enumerator.scan_devices().unwrap() {
    //         let syspath = device.syspath();
    //         let devnode = device.devnode();

    //         if let Some(devnode_path) = devnode {
    //             if devnode_path == Path::new(&self.descriptor.cli_descriptor.path) {
    //                 if let Some(vendor_id) = device.property_value("ID_VENDOR_ID") {
    //                     if let Some(product_id) = device.property_value("ID_MODEL_ID") {
    //                         let Ok(vendor_id) =
    //                             u16::from_str_radix(&vendor_id.to_string_lossy(), 16)
    //                         else {
    //                             return None;
    //                         };
    //                         let Ok(product_id) =
    //                             u16::from_str_radix(&product_id.to_string_lossy(), 16)
    //                         else {
    //                             return None;
    //                         };

    //                         return Some((vendor_id, product_id));
    //                     }
    //                 }
    //             }
    //         }
    //     }

    //     return None;
    // }

    // pub fn reset_usb_device(&mut self) -> Result<(), Box<dyn Error>> {
    //     let Some((vendor_id, product_id)) = self.find_usb_device() else {
    //         return Err("usb device not found".into());
    //     };

    //     for device in rusb::devices()?.iter() {
    //         let device_desc = device.device_descriptor()?;

    //         if device_desc.vendor_id() == vendor_id && device_desc.product_id() == product_id {
    //             eprintln!(
    //                 "Found Device: {} - Vendor ID: {}, Product ID: {}, Resetting...",
    //                 &self.descriptor.cli_descriptor.path, vendor_id, product_id
    //             );
    //             let mut handle = device.open()?;
    //             let res = handle.reset()?;
    //             std::thread::sleep(std::time::Duration::from_millis(100));
    //             return Ok(());
    //         }
    //     }

    //     Err(Box::new(rusb::Error::NoDevice))
    // }

    pub fn read_frame(mut self) -> (Option<Self>, Result<Frame, RadarReadError>) {
        let (connection, frame) = match self.connection.read_frame() {
            Ok((connection, frame)) => (connection, frame),
            Err(e) => return (None, Err(e)),
        };
        self.connection = connection;
        (Some(self), Ok(frame))
    }
}
