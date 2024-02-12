use std::{
    error::Error,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time;

use crate::core::pointcloud::{self, IntoPointCloud, PointCloud, PointCloudLike};

use super::{Sensor, SensorInitError, SensorReadError};

pub struct Playback {
    last_read: u128,
    index: usize,
    recording: Vec<PointCloud>,
}

impl Playback {
    pub fn new(file_path: String) -> Result<Self, Box<dyn Error>> {
        let mut file = std::fs::OpenOptions::new().read(true).open(&file_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let recording: Vec<PointCloud> = serde_json::from_str(&contents)?;
        Ok(Self {
            last_read: recording
                .get(0)
                .map(|pc| pc.time)
                .ok_or("Empty recording!")?,
            index: 0,
            recording,
        })
    }

    pub fn try_read(&mut self) -> Option<PointCloud> {
        // Gets the time since last read
        // keep bumping up index until we hit the first time after indexed time + time passed, and then return the pointcloud *before* index.
        if self.index > self.recording.len() {
            self.index = 0;
        }

        let item = self.recording.get(self.index)?;
        if self.last_read > item.time {}
        let duration = if self.last_read > item.time {
            0
        } else {
            (item.time - self.last_read).min(1000) // no more than 1 second duration between frames
        };
        self.last_read = item.time;
        self.index += 1;
        dbg!(duration as f64 / 1000.0);
        dbg!(&self.index);

        std::thread::sleep(time::Duration::from_millis(duration as u64));

        let mut result = item.clone();
        result.time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|m| m.as_millis())
            .unwrap_or(0);
        Some(result)
    }
}

#[derive(Debug, Hash, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct PlaybackDescriptor {
    pub path: String,
}

impl PlaybackDescriptor {
    pub fn try_initialize(self) -> Result<Playback, SensorInitError> {
        Ok(Playback::new(self.path).map_err(|_e| SensorInitError::DeviceFailure)?)
    }
}

impl Sensor for Playback {
    fn try_read(&mut self) -> Result<PointCloudLike, SensorReadError> {
        match self.try_read() {
            Some(thing) => Ok(pointcloud::PointCloudLike::PointCloud(
                thing.into_point_cloud(),
            )),
            None => Err(SensorReadError::Critical),
        }
    }
}
