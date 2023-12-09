use serde::{Deserialize, Serialize};
use serde_binary::binary_stream::Endian;
use serialport::SerialPort;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::marker::PhantomData;
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::{thread, time};

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
struct TLVHeader {
    tlv_type: TLVType,
    length: u32,
}

#[derive(Deserialize, Debug)]
struct TLVPointCloud {
    points: Vec<[f32; 4]>, // Vector of points, (x, y, z, doppler)
}

#[derive(Deserialize, Debug)]
struct TLVRangeProfile {}

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

// Note: there aren't exactly unit tests for these and half I havent even used, they might just fail
#[derive(Debug)]
enum TLV {
    PointCloud(TLVPointCloud),
    RangeProfile(TLVRangeProfile),
    NoiseProfile(TLVNoiseProfile),
    StaticAzimuthHeatmap(TLVStaticAzimuthHeatmap),
    RangeDopplerHeatmap(TLVRangeDopplerHeatmap),
    Statistics(TLVStatistics),
    SideInfo(TLVSideInfo),
    AzimuthElevationStaticHeatmap(TLVAzimuthElevationStaticHeatmap),
}

impl<'de> Deserialize<'de> for TLV {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let header = TLVHeader::deserialize(deserializer)?;
        match header.tlv_type {
            TLVType::PointCloud => Ok(TLV::PointCloud(TLVPointCloud::deserialize(deserializer)?)),
            TLVType::RangeProfile => Ok(TLV::RangeProfile(TLVRangeProfile::deserialize(
                deserializer,
            )?)),
            TLVType::NoiseProfile => Ok(TLV::NoiseProfile(TLVNoiseProfile::deserialize(
                deserializer,
            )?)),
            TLVType::StaticAzimuthHeatmap => Ok(TLV::StaticAzimuthHeatmap(
                TLVStaticAzimuthHeatmap::deserialize(deserializer)?,
            )),
            TLVType::RangeDopplerHeatmap => Ok(TLV::RangeDopplerHeatmap(
                TLVRangeDopplerHeatmap::deserialize(deserializer)?,
            )),
            TLVType::Statistics => Ok(TLV::Statistics(TLVStatistics::deserialize(deserializer)?)),
            TLVType::SideInfo => Ok(TLV::SideInfo(TLVSideInfo::deserialize(deserializer)?)),
            TLVType::AzimuthElevationStaticHeatmap => Ok(TLV::AzimuthElevationStaticHeatmap(
                TLVAzimuthElevationStaticHeatmap::deserialize(deserializer)?,
            )),
            _ => panic!(), // This should never happen so just crash.
        }
    }
}

impl TLV {
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
pub struct ValidRadarInstance;
#[derive(Debug)]
pub struct InvalidRadarInstance;
impl RadarState for ValidRadarInstance {}
impl RadarState for InvalidRadarInstance {}

impl RadarDescriptor {
    pub fn initialize(
        &self,
    ) -> Result<RadarInstance<ValidRadarInstance>, Box<dyn std::error::Error>> {
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

enum RadarReadResult {
    Good(String, RadarInstance<ValidRadarInstance>),
    Bad(RadarInstance<InvalidRadarInstance>),
    Skipped(RadarInstance<ValidRadarInstance>),
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

        let Some(start_index) =
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
            &buffer[start_index..start_index + std::mem::size_of::<FrameHeader>() + 10],
        )
        .unwrap();
        dbg!(&header);

        // Parse all of the TLVs (type-length-value) using the magic of serde!
        for i in (0..header.num_tlvs) {
            // let tlv = tlv_from_bytes(&buffer[start_index..]);
            // start_index += std::mem::size_of_val(&tlv);
            // dbg!(tlv);
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
