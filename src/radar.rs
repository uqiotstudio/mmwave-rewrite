use crate::message::{Frame, FrameBody, FrameHeader, FromBytes};
use serialport::SerialPort;
use std::fmt::{self, Debug};
use std::{error::Error, fs::File, io::Read, thread, time};

#[derive(Debug)]
pub struct PortDescriptor {
    pub path: String,
    pub baud_rate: u32,
}

impl PortDescriptor {
    /// Returns a SerialPort based on the description of the [`PortDescriptor`].
    ///
    /// # Errors
    ///
    /// This function will return an error if if the serialport could not be opened for any reason.
    pub fn initialize(&self) -> Result<Box<dyn SerialPort>, Box<dyn Error>> {
        Ok(serialport::new(&self.path, self.baud_rate).open()?)
    }
}

#[derive(Debug)]
pub struct RadarDescriptor {
    pub cli_descriptor: PortDescriptor,
    pub data_descriptor: PortDescriptor,
    pub config_path: String,
}

#[derive(Debug)]
pub struct RadarInitError {
    blame: Box<dyn Error>,
    descriptor: RadarDescriptor,
}

impl RadarInitError {
    fn new(blame: Box<dyn Error>, descriptor: RadarDescriptor) -> RadarInitError {
        RadarInitError { blame, descriptor }
    }
}

impl RadarDescriptor {
    pub fn initialize(self) -> Result<Radar, RadarInitError> {
        // Any of these ports can cause failure
        let mut cli_port = match self.cli_descriptor.initialize() {
            Ok(cli_port) => cli_port,
            Err(e) => return Err(RadarInitError::new(e, self)),
        };
        let data_port = match self.data_descriptor.initialize() {
            Ok(data_port) => data_port,
            Err(e) => return Err(RadarInitError::new(e, self)),
        };
        let mut config_file = match File::open(&self.config_path) {
            Ok(config_file) => config_file,
            Err(e) => return Err(RadarInitError::new(Box::new(e), self)),
        };

        let mut config = String::new();
        match config_file.read_to_string(&mut config) {
            Err(e) => return Err(RadarInitError::new(Box::new(e), self)),
            _ => (),
        }

        let radar_descriptor = self;

        let radar = Radar {
            radar_descriptor,
            cli_port,
            data_port,
            config,
        };

        Ok(radar)
    }
}

pub struct Radar {
    radar_descriptor: RadarDescriptor,
    cli_port: Box<dyn SerialPort>,
    data_port: Box<dyn SerialPort>,
    config: String,
}

impl fmt::Debug for Radar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Radar")
            .field("radar_descriptor", &self.radar_descriptor)
            .field("cli_port", &self.cli_port)
            .field("data_port", &self.data_port)
            .field("config", &self.config)
            .finish()
    }
}

#[derive(Debug)]
pub enum RadarReadResult {
    Success(Radar, Frame),
    Malformed(Radar),
    Disconnected(RadarDescriptor),
}

#[derive(Debug)]
pub struct RadarWriteError {
    blame: Box<dyn Error>,
    descriptor: RadarDescriptor,
}

impl RadarWriteError {
    /// Creates a new [`RadarWriteError`].
    pub fn new(blame: Box<dyn Error>, descriptor: RadarDescriptor) -> Self {
        Self { blame, descriptor }
    }
}

impl Radar {
    /// Writes the config to the radars associated serialport.
    /// # Errors
    ///
    /// This function will return an error if the port cannot be written to successfully.
    /// This error carries the descriptor, which will require reinitialization as the port has failed.
    pub fn write_config(mut self) -> Result<Self, RadarWriteError> {
        // A little cheaty way to do a try/catch seeing as it's still in experimental
        match || -> Result<(), Box<dyn Error>> {
            for line in self.config.lines() {
                self.cli_port.write(line.as_bytes())?;
                self.cli_port.flush()?;
                self.cli_port.write(b"\n")?;
                self.cli_port.flush()?;
                println!("{}", line);
                thread::sleep(time::Duration::from_millis(20));
            }
            Ok(())
        }() {
            Ok(_) => Ok(self),
            Err(e) => Err(RadarWriteError::new(e, self.radar_descriptor)),
        }
    }

    /// Ensures there are at least n bytes in the buffer, reading new ones to fill empty space
    fn read_n_bytes(&mut self, n: usize) -> Result<Vec<u8>, std::io::Error> {
        let mut buffer = vec![0; n];
        dbg!("a");
        while (self.data_port.bytes_to_read().unwrap_or(0) as usize) < n {} // Block until available
        self.data_port.read(&mut buffer)?;
        dbg!("b");
        Ok(buffer)
    }

    pub fn read(mut self) -> RadarReadResult {
        const MAGICWORD: [u16; 4] = [0x0102, 0x0304, 0x0506, 0x0708];

        // Get a buffer of the size of the magic word
        let mut buffer = match self.read_n_bytes(std::mem::size_of_val(&MAGICWORD)) {
            Ok(buffer) => buffer,
            Err(e) => return RadarReadResult::Disconnected(self.radar_descriptor),
        };

        // Keep shifting by one byte untill we can find the magic word
        while buffer
            != MAGICWORD
                .iter()
                .flat_map(|&x| x.to_ne_bytes().to_vec())
                .collect::<Vec<u8>>()
        {
            let new_byte = match self.read_n_bytes(1) {
                Ok(buffer) => buffer,
                Err(e) => return RadarReadResult::Disconnected(self.radar_descriptor),
            };
            // Shift the bytes by one
            buffer.extend(new_byte);
            buffer.remove(0);
        }

        // Grow the buffer from the magic number, until we can form a header
        buffer.extend(
            match self.read_n_bytes(FrameHeader::size_of() - std::mem::size_of_val(&MAGICWORD)) {
                Ok(new_buffer) => new_buffer,
                Err(e) => {
                    eprintln!("{:?}:{:?}: {:?}", file!(), line!(), e);
                    return RadarReadResult::Disconnected(self.radar_descriptor);
                }
            },
        );

        // Deserialize the header
        let frame_header: FrameHeader = match FrameHeader::from_bytes(&buffer) {
            Ok(header) => header,
            Err(e) => {
                eprintln!("{:?}:{:?}: {:?}", file!(), line!(), e);
                return RadarReadResult::Malformed(self);
            }
        };

        buffer =
            match self.read_n_bytes(frame_header.packet_length as usize - FrameHeader::size_of()) {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("{:?}:{:?}: {:?}", file!(), line!(), e);
                    return RadarReadResult::Malformed(self);
                }
            };

        let frame_body = match FrameBody::from_bytes(&buffer, frame_header.num_tlvs as usize) {
            Ok(frame_body) => frame_body,
            Err(e) => {
                eprintln!("{:?}:{:?}: {:?}", file!(), line!(), e);
                return RadarReadResult::Malformed(self);
            }
        };

        let frame = Frame {
            frame_header,
            frame_body,
        };

        dbg!(&frame.frame_header);

        // dbg!(&frame);

        RadarReadResult::Success(self, frame)
    }
}
