use super::error::{RadarInitError, RadarReadError, RadarWriteError};
use super::message::{Frame, FrameBody, FrameHeader, FromBytes};
use super::Model;
use regex::Regex;
use serialport::SerialPort;
use std::{thread, time};
use tracing::{info, instrument, warn};

#[derive(Debug)]
pub struct PortDescriptor {
    pub path: String,
    pub baud_rate: u32,
}

impl PortDescriptor {
    pub fn initialize(&self) -> Result<Box<dyn SerialPort>, RadarInitError> {
        serialport::new(&self.path, self.baud_rate)
            .open()
            .map_err(|e| {
                RadarInitError::PortUnavailable(format!("{}, {}", self.path.clone(), e.description))
            })
    }
}

#[derive(Debug)]
pub struct Connection {
    cli_port: Box<dyn SerialPort>,
    data_port: Box<dyn SerialPort>,
}

impl Connection {
    pub fn try_open(serial: String, model: Model) -> Result<Self, RadarInitError> {
        // Attempt to open the two serial devices

        let mut enumerator = udev::Enumerator::new().unwrap();
        let mut cli_port = None;
        let mut data_port = None;

        for device in enumerator.scan_devices().unwrap() {
            if Some(Some(serial.as_str()))
                == device.property_value("ID_SERIAL_SHORT").map(|x| x.to_str())
            {
                match model {
                    Model::AWR1843Boost => {
                        // For AWR1843Boost, the first ttyACMX is cli, second is data
                        let regex = Regex::new(r"^/dev/ttyACM\d+$").unwrap();
                        let Some(Some(devname)) =
                            device.property_value("DEVNAME").map(|x| x.to_str())
                        else {
                            continue;
                        };
                        info!(devname=%devname, "found AWR1843Boost matching serial");
                        if !regex.is_match(devname) {
                            // Irelevant
                            continue;
                        }
                        // The cli_port comes first, so this should be fine (I THINK)
                        // TODO possibly find a better way to do this, there are some distinguishing
                        // features in the attributes/properties to utilize
                        if cli_port.is_none() {
                            cli_port = Some(PortDescriptor {
                                path: devname.to_owned(),
                                baud_rate: 115200,
                            });
                        } else if data_port.is_none() {
                            data_port = Some(PortDescriptor {
                                path: devname.to_owned(),
                                baud_rate: 921600,
                            });
                        }
                    }
                    Model::AWR1843AOP => {
                        // For AWR1843Boost, the first ttyACMX is cli, second is data
                        let regex = Regex::new(r"^/dev/ttyUSB\d+$").unwrap();
                        warn!(dname=?device.property_value("DEVNAME"), "device found");
                        let Some(Some(devname)) =
                            device.property_value("DEVNAME").map(|x| x.to_str())
                        else {
                            continue;
                        };
                        if !regex.is_match(devname) {
                            // Irelevant
                            continue;
                        }

                        info!(devname=%devname, "found AWR1843Aop matching serial");
                        // The cli_port comes first, so this should be fine (I THINK)
                        // TODO possibly find a better way to do this, there are some distinguishing
                        // features in the attributes/properties to utilize
                        if cli_port.is_none() {
                            cli_port = Some(PortDescriptor {
                                path: devname.to_owned(),
                                baud_rate: 115200,
                            });
                        } else if data_port.is_none() {
                            data_port = Some(PortDescriptor {
                                path: devname.to_owned(),
                                baud_rate: 115200,
                            });
                        }
                    }
                }
            }
        }

        info!(cli_port=?cli_port, data_port=?data_port);
        let (cli_port, data_port) = (
            cli_port.ok_or(RadarInitError::PortNotFound("CLI Port".to_owned()))?,
            data_port.ok_or(RadarInitError::PortNotFound("Data Port".to_owned()))?,
        );

        Ok(Self {
            cli_port: cli_port.initialize()?,
            data_port: data_port.initialize()?,
        })
    }

    fn read_n_bytes(&mut self, n: usize) -> Result<Vec<u8>, RadarReadError> {
        let mut buffer = vec![0; n];

        let time = std::time::Instant::now();
        while (self.data_port.bytes_to_read().unwrap_or(0) as usize) < n {
            if time.elapsed().as_millis() > 1000 {
                return Err(RadarReadError::Disconnected);
            }
        } // Block until available, with timeout of 1000ms!

        match self.data_port.read(&mut buffer) {
            Ok(_) => Ok(buffer),
            Err(_) => Err(RadarReadError::Disconnected),
        }
    }

    pub fn read_frame(&mut self) -> Result<Frame, RadarReadError> {
        const MAGICWORD: [u16; 4] = [0x0102, 0x0304, 0x0506, 0x0708];

        // Get a buffer of the size of the magic word
        let mut buffer;
        buffer = self.read_n_bytes(std::mem::size_of_val(&MAGICWORD))?;

        // Keep shifting by one byte untill we can find the magic word
        while buffer
            != MAGICWORD
                .iter()
                .flat_map(|&x| x.to_ne_bytes().to_vec())
                .collect::<Vec<u8>>()
        {
            let new_byte;
            new_byte = self.read_n_bytes(1)?;
            buffer.extend(new_byte);
            buffer.remove(0);
        }

        // Grow the buffer from the magic number, until we can form a header
        let extension;
        extension =
            self.read_n_bytes(FrameHeader::size_of() - std::mem::size_of_val(&MAGICWORD))?;
        buffer.extend(extension);

        // Deserialize the header
        let frame_header =
            FrameHeader::from_bytes(&buffer).map_err(|e| RadarReadError::ParseError(e))?;

        buffer = self.read_n_bytes(frame_header.packet_length as usize - FrameHeader::size_of())?;

        let frame_body = FrameBody::from_bytes(&buffer, frame_header.num_tlvs as usize)
            .map_err(|e| RadarReadError::ParseError(e))?;

        let frame = Frame {
            frame_header,
            frame_body,
        };

        // dbg!(&frame.frame_header);

        Ok(frame)
    }

    pub fn send_command(mut self, command: String) -> Result<Self, RadarWriteError> {
        // A little cheaty way to do a try/catch seeing as it's still in experimental
        // Dont think too hard about it
        for line in command.lines() {
            if let Err(_) = self
                .cli_port
                .write(line.as_bytes())
                .and_then(|_| self.cli_port.flush())
                .and_then(|_| self.cli_port.write(b"\n"))
                .and_then(|_| self.cli_port.flush())
            {
                return Err(RadarWriteError::Disconnected);
            }
            // println!("{}", line);
            thread::sleep(time::Duration::from_millis(20));
        }
        Ok(self)
    }
}
