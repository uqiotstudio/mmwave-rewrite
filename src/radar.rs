use serde::de::{DeserializeOwned, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serialport::SerialPort;
use std::collections::VecDeque;
use std::fmt::{self, Debug};
use std::marker::PhantomData;
use std::{error::Error, fs::File, io::Read, thread, time};

const MAGICWORD: [u16; 4] = [0x0102, 0x0304, 0x0506, 0x0708];

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
        while (self.data_port.bytes_to_read().unwrap_or(0) as usize) < n {} // Block until available
        self.data_port.read(&mut buffer)?;
        Ok(buffer)
    }

    /// Reads from the radar to get the next buffered frame. Frames are never skipped, so if called infrequently this can lead to a large backlog (however much the serial port can back up).
    ///
    /// # Errors
    ///
    /// In the event of a error regarding the serial ports, a [`RadarReadResult::Disconnected`] is returned and will require reconnection. In the event of a malformed message the radar is recoverable through the returned [`RadarReadResult::Malformed`].
    pub fn read(mut self) -> RadarReadResult {
        // Find magic number else block & grow buffer until buffer contains magic number
        dbg!("Beginning Read");

        let mut buffer = match self.read_n_bytes(std::mem::size_of_val(&MAGICWORD)) {
            Ok(buffer) => buffer,
            Err(e) => return RadarReadResult::Disconnected(self.radar_descriptor),
        };
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
            // TODO probably could optimize this using VecDequeue, seems unecessary
            buffer.extend(new_byte);
            buffer.remove(0);
        }

        // Grow the buffer from the magic number, until we can form a header
        let mut new_buffer = match self
            .read_n_bytes(std::mem::size_of::<FrameHeader>() - std::mem::size_of_val(&MAGICWORD))
        {
            Ok(mut new_buffer) => new_buffer,
            Err(e) => {
                eprintln!("{:?}:{:?}: {:?}", file!(), line!(), e);
                return RadarReadResult::Disconnected(self.radar_descriptor);
            }
        };
        buffer.extend(new_buffer);

        // Deserialize the header
        let header = match FrameHeader::from_bytes(&buffer) {
            Ok(header) => header,
            Err(e) => {
                eprintln!("{:?}:{:?}: {:?}", file!(), line!(), e);
                return RadarReadResult::Malformed(self);
            }
        };

        dbg!(&header);

        // Block until the size described by header is available.
        let body_length = header.packet_length as usize - std::mem::size_of::<FrameHeader>();
        let mut buffer = match self.read_n_bytes(body_length) {
            Ok(mut buffer) => buffer,
            Err(e) => {
                eprintln!("{:?}:{:?}: {:?}", file!(), line!(), e);
                return RadarReadResult::Disconnected(self.radar_descriptor);
            }
        };

        // Populate the body with tlvs!
        let mut body: Vec<TLV> = Vec::new();
        let mut tlvs_remaining = header.num_tlvs;
        while tlvs_remaining > 0 {
            dbg!(tlvs_remaining);
            let tlv = match TLV::from_bytes(&buffer) {
                Ok(tlv) => tlv,
                Err(e) => {
                    eprintln!("{:?}:{:?}: {:?}", file!(), line!(), e);
                    return RadarReadResult::Malformed(self);
                }
            };
            dbg!("complete");
            dbg!(std::mem::size_of_val(&tlv));
            dbg!(std::mem::size_of::<TLVHeader>());
            dbg!(std::mem::size_of::<TLVType>());
            dbg!(std::mem::size_of::<TLVHeader>() as u32 + tlv.header.length);
            buffer.drain(0..(std::mem::size_of::<TLVHeader>() + tlv.header.length as usize));
            body.push(tlv);
            tlvs_remaining -= 1;
        }

        RadarReadResult::Success(
            self,
            Frame {
                header: header,
                body: body,
            },
        )
    }
}

#[derive(Deserialize, Debug)]
struct FrameHeader {
    magic_word: [u16; 4],
    version: u32,
    packet_length: u32,
    platform: u32,
    frame_number: u32,
    time: u32,
    num_detected: u32,
    num_tlvs: u32,
    subframe_num: u32,
}

impl FrameHeader {
    fn from_bytes(bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        if bytes.len() < std::mem::size_of::<FrameHeader>() {
            return Err("Byte slice is too short to parse a FrameHeader".into());
        }

        Ok(bincode::deserialize(bytes)?)
    }
}

#[derive(Deserialize, Debug)]
struct TLVHeader {
    tlv_type: TLVType,
    length: u32,
}

#[derive(Debug)]
struct TLV {
    header: TLVHeader,
    body: TLVBody,
}

#[derive(Debug)]
pub struct Frame {
    header: FrameHeader,
    body: Vec<TLV>,
}

trait FromBytes: Sized {
    fn from_bytes(bytes: &[u8]) -> Option<Self>;
}

impl FromBytes for u8 {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 1 {
            Some(bytes[0])
        } else {
            None
        }
    }
}

impl<T: Default + Copy + FromBytes, const N: usize> FromBytes for [T; N] {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let item_size = std::mem::size_of::<T>();
        if bytes.len() < item_size * N {
            return None; // Not enough bytes to fill the array
        }

        let mut data: [T; N] = [T::default(); N]; // Initialize array with default values

        for (i, chunk) in bytes.chunks(item_size).enumerate().take(N) {
            if let Some(item) = T::from_bytes(chunk) {
                data[i] = item;
            } else {
                return None; // Conversion failed
            }
        }

        Some(data)
    }
}

#[derive(Debug)]
struct TLVPointCloud {
    points: Vec<[u32; 4]>, // Vector of points, (x, y, z, doppler)
}

impl FromBytes for TLVPointCloud {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        todo!()
    }
}

#[derive(Debug)]
struct TLVRangeProfile {
    points: Vec<[u8; 2]>,
}

impl FromBytes for TLVRangeProfile {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let item_size = std::mem::size_of::<[u8; 2]>();
        let mut items = Vec::new();
        for i in 0..(bytes.len() / item_size) {
            if let Some(ndvec) = <[u8; 2]>::from_bytes(&bytes[i * item_size..(i + 1) * item_size]) {
                items.push(ndvec);
            } else {
                return None;
            }
        }
        Some(Self { points: items })
    }
}

#[derive(Debug)]
struct TLVNoiseProfile {}

impl FromBytes for TLVNoiseProfile {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        todo!()
    }
}

#[derive(Debug)]
struct TLVStaticAzimuthHeatmap {}

impl FromBytes for TLVStaticAzimuthHeatmap {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        todo!()
    }
}

#[derive(Debug)]
struct TLVRangeDopplerHeatmap {}

impl FromBytes for TLVRangeDopplerHeatmap {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        todo!()
    }
}

#[derive(Debug)]
struct TLVStatistics {}

impl FromBytes for TLVStatistics {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        todo!()
    }
}

#[derive(Debug)]
struct TLVSideInfo {}

impl FromBytes for TLVSideInfo {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        todo!()
    }
}

#[derive(Debug)]
struct TLVAzimuthElevationStaticHeatmap {}

impl FromBytes for TLVAzimuthElevationStaticHeatmap {
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        todo!()
    }
}

#[derive(Debug)]
enum TLVBody {
    PointCloud(TLVPointCloud),
    RangeProfile(TLVRangeProfile),
    NoiseProfile(TLVNoiseProfile),
    StaticAzimuthHeatmap(TLVStaticAzimuthHeatmap),
    RangeDopplerHeatmap(TLVRangeDopplerHeatmap),
    Statistics(TLVStatistics),
    SideInfo(TLVSideInfo),
    AzimuthElevationStaticHeatmap(TLVAzimuthElevationStaticHeatmap),
}

#[repr(u32)]
#[derive(Deserialize, Debug)]
// The full list of TLVTypes can be found at https://dev.ti.com/tirex/explore/node?node=A__ADnbI7zK9bSRgZqeAxprvQ__radar_toolbox__1AslXXD__LATEST in case i need to implement more later on. Theres quite a few, in particular for *other* radar models that im skipping.
enum TLVType {
    PointCloud = 1,
    RangeProfile = 2,
    NoiseProfile = 3,
    StaticAzimuthHeatmap = 4,
    RangeDopplerHeatmap = 5,
    Statistics = 6,
    SideInfo = 7,
    AzimuthElevationStaticHeatmap = 8,
    Temperature = 9,
    // Unknown(u32), // This is for memory safety reasons but breaks the representation. Uh Oh!
}

impl TLV {
    fn from_bytes(bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        let start_index = std::mem::size_of::<TLVHeader>();
        let header: TLVHeader = bincode::deserialize(&bytes[..start_index])?;
        dbg!(&header);
        let body = match || -> Option<TLVBody> {
            Some(match header.tlv_type {
                TLVType::PointCloud => TLVBody::PointCloud(TLVPointCloud::from_bytes(
                    &bytes[start_index..start_index + header.length as usize],
                )?),
                TLVType::RangeProfile => TLVBody::RangeProfile(TLVRangeProfile::from_bytes(
                    &bytes[start_index..start_index + header.length as usize],
                )?),
                TLVType::NoiseProfile => TLVBody::NoiseProfile(TLVNoiseProfile::from_bytes(
                    &bytes[start_index..start_index + header.length as usize],
                )?),
                TLVType::StaticAzimuthHeatmap => {
                    TLVBody::StaticAzimuthHeatmap(TLVStaticAzimuthHeatmap::from_bytes(
                        &bytes[start_index..start_index + header.length as usize],
                    )?)
                }
                TLVType::RangeDopplerHeatmap => {
                    TLVBody::RangeDopplerHeatmap(TLVRangeDopplerHeatmap::from_bytes(
                        &bytes[start_index..start_index + header.length as usize],
                    )?)
                }
                TLVType::Statistics => TLVBody::Statistics(TLVStatistics::from_bytes(
                    &bytes[start_index..start_index + header.length as usize],
                )?),
                TLVType::SideInfo => TLVBody::SideInfo(TLVSideInfo::from_bytes(
                    &bytes[start_index..start_index + header.length as usize],
                )?),
                _ => panic!(), // This should never happen so just crash.
            })
        }() {
            Some(body) => body,
            _ => return Err("".into()),
        };
        Ok(Self { header, body })
    }
}
