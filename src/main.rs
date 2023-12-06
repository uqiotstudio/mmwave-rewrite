use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::{thread, time};

#[derive(Debug)]
pub enum RadarError {
    InvalidPath(PathBuf),             // In the event of a path not existing
    PortUnavailable(PathBuf, String), // In the event of a port being used/unavailable
    InvalidConfig(String),            // In the event of an invalid configuration file
}

#[derive(Debug, Clone)]
pub enum Model {
    AWR1843,
    AWR1843AOP,
}

#[derive(Debug, Clone)]
pub struct RadarDescriptor {
    pub model: Model,
    pub cli_port: PathBuf,
    pub data_port: PathBuf,
    pub config: PathBuf,
}

#[derive(Debug)]
pub struct RadarInstance<S: RadarState> {
    descriptor: RadarDescriptor,
    status: PhantomData<S>,
    cli_port: File,
    data_port: File,
}

pub trait RadarState {}
#[derive(Debug)]
pub struct ValidRadarInstance;
#[derive(Debug)]
pub struct InvalidRadarInstance;
impl RadarState for ValidRadarInstance {}
impl RadarState for InvalidRadarInstance {}

impl RadarDescriptor {
    pub fn initialize(&self) -> Result<RadarInstance<ValidRadarInstance>, RadarError> {
        let Ok(mut cli_port) = File::create(self.cli_port.clone()) else {
            return Err(RadarError::InvalidPath(self.cli_port.clone()));
        };
        let Ok(data_port) = File::open(self.data_port.clone()) else {
            return Err(RadarError::InvalidPath(self.data_port.clone()));
        };
        let Ok(mut config_file) = File::open(self.config.clone()) else {
            return Err(RadarError::InvalidPath(self.config.clone()));
        };

        let mut config_contents = String::new();
        if let Err(err) = config_file.read_to_string(&mut config_contents) {
            return Err(RadarError::PortUnavailable(
                self.config.clone(),
                err.to_string(),
            ));
        };

        dbg!(&config_contents.lines().collect::<Vec<&str>>());
        config_contents = config_contents.lines().map(|l| format!("{l}\n")).collect();
        for line in config_contents.lines().map(str::as_bytes) {
            if let Err(err) = cli_port.write(line) {
                return Err(RadarError::PortUnavailable(
                    self.cli_port.clone(),
                    err.to_string(),
                ));
            }
            println!("{}", String::from_utf8(line.into()).unwrap_or_default());
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
}

impl RadarInstance<ValidRadarInstance> {
    fn read(mut self) -> RadarReadResult {
        let mut contents = String::new();
        let read = self.data_port.read_to_string(&mut contents);
        RadarReadResult::Good(contents, self)
    }
}

fn main() {
    // Initialize the test radar descriptor
    let radar_descriptor = RadarDescriptor {
        model: Model::AWR1843,
        cli_port: Path::new("/dev/ttyACM0").to_owned(),
        data_port: Path::new("/dev/ttyACM1").to_owned(),
        config: Path::new("./profile_3d.cfg").to_owned(),
    };

    dbg!(&radar_descriptor);

    // Consumes radar_descriptor to produce a valid RadarInstance
    let mut radar_instance = radar_descriptor.initialize().unwrap();

    dbg!(&radar_instance);

    loop {
        // This is a tricky little ownership thing, just trust that it works
        radar_instance = {
            match radar_instance.read() {
                RadarReadResult::Good(contents, new_radar_instance) => {
                    // Do operations on the radar instance
                    new_radar_instance
                }
                RadarReadResult::Bad(_) => todo!(),
            }
        };
        thread::sleep(time::Duration::from_secs_f64(0.1));
    }
}
