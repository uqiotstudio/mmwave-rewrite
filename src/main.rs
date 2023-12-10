use serde::de::{DeserializeOwned, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_binary::binary_stream::Endian;
use serialport::SerialPort;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::marker::PhantomData;
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::{fmt, thread, time};

const MAGICWORD: [u16; 4] = [0x0102, 0x0304, 0x0506, 0x0708];

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

#[derive(Deserialize, Debug)]
struct NdVec<T: Serialize + DeserializeOwned, const N: usize> {
    #[serde(with = "serde_arrays")]
    data: [T; N],
}

#[derive(Deserialize, Debug)]
struct TLVHeader {
    tlv_type: TLVType,
    length: u32,
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

// Note: there aren't exactly unit tests for these and half I havent even used, they might just fail
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

#[derive(Debug)]
struct TLV {
    header: TLVHeader,
    body: TLVBody,
}

impl TLV {
    fn from_bytes(bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        let start_index = std::mem::size_of::<TLVHeader>();
        let header: TLVHeader = bincode::deserialize(&bytes[..start_index])?;
        dbg!(&header);
        let body = match header.tlv_type {
            TLVType::PointCloud => TLVBody::PointCloud(bincode::deserialize(
                &bytes[start_index..start_index + header.length as usize],
            )?),
            TLVType::RangeProfile => TLVBody::RangeProfile(bincode::deserialize(
                &bytes[start_index..start_index + header.length as usize],
            )?),
            TLVType::NoiseProfile => TLVBody::NoiseProfile(bincode::deserialize(
                &bytes[start_index..start_index + header.length as usize],
            )?),
            TLVType::StaticAzimuthHeatmap => TLVBody::StaticAzimuthHeatmap(bincode::deserialize(
                &bytes[start_index..start_index + header.length as usize],
            )?),
            TLVType::RangeDopplerHeatmap => TLVBody::RangeDopplerHeatmap(bincode::deserialize(
                &bytes[start_index..start_index + header.length as usize],
            )?),
            TLVType::Statistics => TLVBody::Statistics(bincode::deserialize(
                &bytes[start_index..start_index + header.length as usize],
            )?),
            TLVType::SideInfo => TLVBody::SideInfo(bincode::deserialize(
                &bytes[start_index..start_index + header.length as usize],
            )?),
            _ => panic!(), // This should never happen so just crash.
        };
        Ok(Self { header, body })
    }
}

impl TLVBody {
    fn from_bytes(bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        todo!()
    }
}

impl FrameHeader {
    fn from_bytes(bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        if bytes.len() < std::mem::size_of::<FrameHeader>() {
            return Err("Byte slice is too short to parse a FrameHeader".into());
        }

        Ok(bincode::deserialize(bytes)?)
    }
}

#[derive(Debug)]
pub enum RadarError {
    InvalidPath(PathBuf),             // In the event of a path not existing
    PortUnavailable(PathBuf, String), // In the event of a port being used/unavailable
    InvalidConfig(String),            // In the event of an invalid configuration file
}

#[derive(Debug, Clone)]
pub struct PortDescriptor(String, u32);

#[derive(Debug, Clone)]
pub struct RadarDescriptor {
    pub cli_descriptor: PortDescriptor,
    pub data_descriptor: PortDescriptor,
    pub config_path: PathBuf,
}

pub struct RadarInstance<S: RadarState> {
    descriptor: RadarDescriptor,
    status: PhantomData<S>,
    cli_port: Box<dyn SerialPort>,
    data_port: Box<dyn SerialPort>,
}

impl<S: RadarState> std::fmt::Debug for RadarInstance<S>
where
    S: std::fmt::Debug, // Assuming S and RadarDescriptor implement Debug
    RadarDescriptor: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RadarInstance")
            .field("descriptor", &self.descriptor)
            .field("status", &self.status)
            .finish()
    }
}

pub trait RadarState {}
#[derive(Debug)]
pub struct RadarReadyState; // The radar has found both magic words
#[derive(Debug)]
pub struct RadarReadingState; // The radar has not found the second magicword
#[derive(Debug)]
pub struct RadarFreshState; // The radar has not found the first magicword
#[derive(Debug)]
pub struct RadarDisconnectedState; // The radar has disconnected and requires some fixing
impl RadarState for RadarReadyState {}
impl RadarState for RadarReadingState {}
impl RadarState for RadarFreshState {}
impl RadarState for RadarDisconnectedState {}

impl RadarDescriptor {
    pub fn initialize(&self) -> Result<RadarInstance<RadarFreshState>, Box<dyn std::error::Error>> {
        let mut cli_port = serialport::new(&self.cli_descriptor.0, self.cli_descriptor.1).open()?;
        let data_port = serialport::new(&self.data_descriptor.0, self.data_descriptor.1).open()?;

        let mut config_file = File::open(&self.config_path)?;

        let mut config_contents = String::new();
        config_file.read_to_string(&mut config_contents)?;

        dbg!(&config_contents.lines().collect::<Vec<&str>>());
        config_contents = config_contents.lines().map(|l| format!("{l}\n")).collect();

        for line in config_contents.lines() {
            cli_port.write(line.as_bytes())?;

            cli_port.flush()?;

            cli_port.write(b"\n")?;

            cli_port.flush()?;

            println!("{}", line);

            thread::sleep(time::Duration::from_millis(20));
        }

        Ok(RadarInstance {
            descriptor: self.clone(),
            status: PhantomData::default(),
            cli_port,
            data_port,
        })
    }
}

struct DataFrame {}

struct RadarReadResult(RadarInstance<RadarFreshState>, DataFrame);

impl RadarInstance<RadarReadyState> {
    fn read(mut self) -> RadarReadResult {
        (
            RadarInstance {
                descriptor: todo!(),
                status: PhantomData::default(),
                cli_port: todo!(),
                data_port: todo!(),
            },
            DataFrame {},
        )
    }
}

enum RadarPollResult {
    Finished(RadarInstance<RadarReadyState>),
    Unfinished(RadarInstance<RadarReadingState>),
    Disconnected(RadarInstance<RadarDisconnectedState>),
}

impl RadarInstance<RadarReadingState> {
    fn try_poll(mut self) -> RadarPollResult {
        todo!()
    }
}

enum RadarStartupResult {
    Begin(RadarInstance<RadarReadingState>),
    Fail(RadarInstance<RadarFreshState>),
    Disconnect(RadarInstance<RadarDisconnectedState>),
}

impl RadarInstance<RadarFreshState> {
    fn try_start(mut self) -> RadarStartupResult {}
}

impl RadarInstance<ValidRadarInstance> {
    fn read(mut self) -> RadarReadResult {
        // We need to return this result because there is a chance when we read that something has failed, giving us an InvalidRadarInstance that needs to reconnect.
        let bytes_available = self.data_port.bytes_to_read().unwrap_or(0);
        let mut buffer = vec![0; bytes_available as usize];
        match self.data_port.read(&mut buffer) {
            Err(e) => {
                dbg!(e);
            }
            _ => {}
        }

        let Some(mut start_index) =
            buffer
                .windows(std::mem::size_of_val(&MAGICWORD))
                .position(|window| {
                    window
                        == MAGICWORD
                            .iter()
                            .flat_map(|&x| x.to_ne_bytes().to_vec())
                            .collect::<Vec<u8>>()
                })
        else {
            return RadarReadResult::Skipped(self);
        };

        let header = FrameHeader::from_bytes(
            &buffer[start_index..start_index + std::mem::size_of::<FrameHeader>()],
        )
        .unwrap();
        dbg!(&header);

        // Bump the start index up to after the frame header
        dbg!(start_index);
        start_index = std::mem::size_of::<FrameHeader>() + start_index;
        dbg!(start_index);
        dbg!(buffer.len());

        // Parse all of the TLVs (type-length-value) using the magic of serde!
        for i in (0..header.num_tlvs) {
            let tlv = match TLV::from_bytes(&buffer[start_index..]) {
                Ok(tlv) => tlv,
                Err(e) => {
                    eprintln!("Malformed TLV with error {:?}", e);
                    break;
                }
            };
            start_index += tlv.header.length as usize + std::mem::size_of::<TLVHeader>();
            dbg!(tlv);
        }

        RadarReadResult::Good("".to_owned(), self)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the test radar descriptor
    let radar_descriptor = RadarDescriptor {
        cli_descriptor: PortDescriptor("/dev/ttyACM0".to_owned(), 115200),
        data_descriptor: PortDescriptor("/dev/ttyACM1".to_owned(), 921600),
        config_path: Path::new("./profile_AWR1843B.cfg").to_owned(),
    };

    dbg!(&radar_descriptor);

    // Consumes radar_descriptor to produce a valid RadarInstance
    let mut radar_instance = radar_descriptor.initialize()?;

    dbg!(&radar_instance);

    loop {
        // This is a tricky little ownership thing, just trust that it works
        radar_instance = {
            match radar_instance.read() {
                RadarReadResult::Good(contents, new_radar_instance) => {
                    // Do operations on the radar instance
                    println!(":{}", contents);
                    new_radar_instance
                }
                RadarReadResult::Bad(_) => todo!(),
                RadarReadResult::Skipped(new_radar_instance) => new_radar_instance,
            }
        };
        thread::sleep(time::Duration::from_secs_f64(0.1));
    }
}
