use std::{
    error::Error,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    pointcloud::{self, IntoPointCloud, PointCloud, PointMetaData},
    pointcloud_provider::PointCloudProvider,
};

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time;

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

        let item = self.recording.get(self.index)?;
        let duration = item.time - self.last_read;
        self.last_read = item.time;
        self.index += 1;

        // Track the real duration and bump it up
        let real_duration = time::Instant::now().duration_since(self.last_read);
        self.last_read = time::Instant::now();

        // Get the last item
        let start = self.recording.get(self.index)?;
        self.index += 1;
        while let Some(item) = self.recording.get(self.index) {
            let recording_passed = (item.time - start.time); // in ms
            if recording_passed > real_duration.as_millis() {
                // We hit something further forward than we passed, go back a step and break with this index
                self.index -= 1;
                let mut item = item.clone();
                item.time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                return Some(item);
            }
            self.index += 1;
        }

        if self.index > self.recording.len() {
            self.index = 0;
            self.last_read = time::Instant::now();
        }

        None
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub struct PlaybackDescriptor {
    pub path: String,
}

impl PlaybackDescriptor {
    pub fn try_initialize(self) -> Result<Playback, Box<dyn std::error::Error>> {
        Ok(Playback::new(self.path)?)
    }
}

impl PointCloudProvider for Playback {
    fn try_read(&mut self) -> Result<crate::pointcloud::PointCloudLike, Box<dyn Error + Send>> {
        match self.try_read() {
            Some(thing) => Ok(pointcloud::PointCloudLike::PointCloud(
                thing.into_point_cloud(),
            )),
            None => Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, ""))),
        }
    }
}
