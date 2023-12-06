use std::fs::File;
use std::io::prelude::*;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

pub enum RadarError {
    InvalidPath(PathBuf),     // In the event of a path not existing
    PortUnavailable(PathBuf), // In the event of a port being used/unavailable
    InvalidConfig(String),    // In the event of an invalid configuration file
}

#[derive(Debug)]
pub enum Model {
    AWR1843,
    AWR1843AOP,
}

#[derive(Debug)]
pub struct RadarDescriptor {
    pub model: Model,
    pub cli_port: Box<PathBuf>,
    pub data_port: Box<PathBuf>,
    pub config: Box<PathBuf>,
}

pub struct RadarInstance<S: RadarState> {
    descriptor: RadarDescriptor,
    status: PhantomData<S>,
}

pub struct ValidRadarInstance;
pub struct InvalidRadarInstance;
impl RadarState for ValidRadarInstance {}
impl RadarState for InvalidRadarInstance {}
pub trait RadarState {}

impl RadarDescriptor {
    pub fn initialize(self) -> Result<RadarInstance<ValidRadarInstance>, RadarError> {
        // TODO connect the radar so that its fully reading, or error out!
        Ok(RadarInstance {
            descriptor: self,
            status: PhantomData::default(),
        })
    }
}

fn main() {
    // Initialize the test radar descriptor
    let radar_descriptor = RadarDescriptor {
        model: Model::AWR1843,
        cli_port: Box::new(Path::new("/devttyACM0").to_owned()),
        data_port: Box::new(Path::new("/dev/ttyACM1").to_owned()),
        config: Box::new(Path::new("./profile_3d.cfg").to_owned()),
    };

    dbg!(&radar_descriptor);

    // Consumes radar_descriptor to produce a valid RadarInstance
    let radar_instance = radar_descriptor.initialize();
}
