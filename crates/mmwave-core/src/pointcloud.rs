use crate::point::Point;
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

type Type = DateTime<Utc>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PointCloud {
    pub time: Type,
    pub points: Vec<Point>, // x, y, z, v
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PointMetaData {}

impl PointCloud {
    pub fn extend(&mut self, mut other: PointCloud) {
        // Extends this pointcloud with other, consuming it
        self.points.append(&mut other.points);
    }
}

impl From<Vec<Point>> for PointCloud {
    fn from(value: Vec<Point>) -> Self {
        Self {
            time: chrono::Utc::now(),
            points: value,
        }
    }
}

impl Default for PointCloud {
    fn default() -> Self {
        PointCloud {
            time: Utc::now(),
            points: Vec::new(),
        }
    }
}
