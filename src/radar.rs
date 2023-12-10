use serde::de::{DeserializeOwned, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serialport::SerialPort;
use std::collections::VecDeque;
use std::fmt;
use std::marker::PhantomData;
use std::{error::Error, fs::File, io::Read, thread, time};

const MAGICWORD: [u16; 4] = [0x0102, 0x0304, 0x0506, 0x0708];

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

pub struct RadarDescriptor {
    pub cli_descriptor: PortDescriptor,
    pub data_descriptor: PortDescriptor,
    pub config_path: String,
}

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
        let buffer = Vec::new();

        let radar = Radar {
            radar_descriptor,
            cli_port,
            data_port,
            config,
            buffer,
        };

        Ok(radar)
    }
}

pub struct Radar {
    radar_descriptor: RadarDescriptor,
    cli_port: Box<dyn SerialPort>,
    data_port: Box<dyn SerialPort>,
    config: String,
    buffer: Vec<u8>,
}

pub enum RadarReadResult {
    Success(Radar, Frame),
    Malformed(Radar),
    Disconnected(RadarDescriptor),
}

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
                thread::sleep(time::Duration::from_millis(10));
            }
            Ok(())
        }() {
            Ok(_) => Ok(self),
            Err(e) => Err(RadarWriteError::new(e, self.radar_descriptor)),
        }
    }

    /// Reads from the radar to get the next buffered frame. Frames are never skipped, so if called infrequently this can lead to a large backlog (however much the serial port can back up).
    ///
    /// # Errors
    ///
    /// In the event of a error regarding the serial ports, a [`RadarReadResult::Disconnected`] is returned and will require reconnection. In the event of a malformed message the radar is recoverable through the returned [`RadarReadResult::Malformed`].
    pub fn read(mut self) -> RadarReadResult {
        // Find magic number else block & grow buffer until buffer contains magic number

        let mut index: usize = loop {
            let bytes_available = self.data_port.bytes_to_read().unwrap_or(0);
            let mut temp_buffer = vec![0; bytes_available as usize];
            if let Err(e) = self.data_port.read(&mut temp_buffer) {
                return RadarReadResult::Disconnected(self.radar_descriptor);
            }

            self.buffer.extend(temp_buffer);

            match self
                .buffer
                .windows(std::mem::size_of_val(&MAGICWORD))
                .position(|window| {
                    window
                        == MAGICWORD
                            .iter()
                            .flat_map(|&x| x.to_ne_bytes().to_vec())
                            .collect::<Vec<u8>>()
                }) {
                Some(index) => break index,
                // In the event of None, drain all but the last MAGICWORD::memsize worth of elements
                // as they are irrelevant going forwards
                None => self
                    .buffer
                    .drain(0..(self.buffer.len() - std::mem::size_of_val(&MAGICWORD)).max(0)),
            };
        };

        self.buffer.drain(0..index); // The buffer now starts at the magic word

        // Block, growing the buffer from the magic number, until we can form a header
        while (self.data_port.bytes_to_read().unwrap_or(0) as usize)
            < std::mem::size_of::<FrameHeader>()
        {}

        // Load those bytes into the buffer
        let bytes_available = self.data_port.bytes_to_read().unwrap_or(0);
        let mut temp_buffer = vec![0; bytes_available as usize];
        if let Err(e) = self.data_port.read(&mut temp_buffer) {
            return RadarReadResult::Disconnected(self.radar_descriptor);
        };
        self.buffer.extend(temp_buffer);

        // Deserialize the header
        let Ok(header) = FrameHeader::from_bytes(&self.buffer) else {
            return RadarReadResult::Malformed(self);
        };

        // Trim the header out of the buffer
        self.buffer.drain(..std::mem::size_of::<FrameHeader>());

        // Block until the size described by header is available.

        while self.buffer.len() < header.packet_length as usize - std::mem::size_of::<FrameHeader>()
        {
            let bytes_available = self.data_port.bytes_to_read().unwrap_or(0);
            let mut temp_buffer = vec![0; bytes_available as usize];
            if let Err(e) = self.data_port.read(&mut temp_buffer) {
                return RadarReadResult::Disconnected(self.radar_descriptor);
            };
            self.buffer.extend(temp_buffer);
        }

        // Deserialize into a frame. Reset buffer to the remainder.

        todo!()
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

struct TLV {
    header: TLVHeader,
    body: TLVBody,
}

pub struct Frame {
    header: FrameHeader,
    body: Vec<TLV>,
}

#[derive(Deserialize, Debug)]
struct NdVec<T: Serialize + DeserializeOwned, const N: usize> {
    #[serde(with = "serde_arrays")]
    data: [T; N],
}

#[derive(Deserialize, Debug)]
struct TLVPointCloud {
    points: Vec<NdVec<u32, 4>>, // Vector of points, (x, y, z, doppler)
}

#[derive(Deserialize, Debug)]
struct TLVRangeProfile {
    #[serde(deserialize_with = "custom_vec_deserializer::<_, u16, 2>")]
    points: Vec<NdVec<u16, 2>>,
}

#[derive(Deserialize, Debug)]
struct TLVNoiseProfile {}

#[derive(Deserialize, Debug)]
struct TLVStaticAzimuthHeatmap {}

#[derive(Deserialize, Debug)]
struct TLVRangeDopplerHeatmap {}

#[derive(Deserialize, Debug)]
struct TLVStatistics {}

#[derive(Deserialize, Debug)]
struct TLVSideInfo {}

#[derive(Deserialize, Debug)]
struct TLVAzimuthElevationStaticHeatmap {}

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
    Unknown(u32), // This is for memory safety reasons, and conveniently forces us to handle invalid state
}

fn custom_vec_deserializer<'de, D, T, const N: usize>(
    deserializer: D,
) -> Result<Vec<NdVec<T, N>>, D::Error>
where
    D: Deserializer<'de>,
    T: Serialize + DeserializeOwned,
{
    struct VecVisitor<T, const N: usize>
    where
        T: Serialize + DeserializeOwned,
    {
        marker: PhantomData<NdVec<T, N>>,
    }

    impl<'de, T: Serialize + DeserializeOwned, const N: usize> Visitor<'de> for VecVisitor<T, N> {
        type Value = Vec<NdVec<T, N>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a sequence of arrays")
        }

        fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
        where
            S: SeqAccess<'de>,
        {
            let mut vec = Vec::new();

            while let Some(array) = seq.next_element::<NdVec<T, N>>()? {
                vec.push(array);
            }

            Ok(vec)
        }
    }

    let visitor = VecVisitor {
        marker: PhantomData,
    };
    deserializer.deserialize_seq(visitor)
}
