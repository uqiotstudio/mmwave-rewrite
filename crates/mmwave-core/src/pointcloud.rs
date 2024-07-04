use crate::point::Point;
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

type Type = DateTime<Utc>;

#[derive(Debug, Clone)]
pub struct PointCloud {
    pub time: Type,
    pub points: Vec<Point>, // x, y, z, v
    pub labels: Vec<String>
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
            labels: Vec::new()
        }
    }

}

impl From<(Vec<Point>, Vec<String>)> for PointCloud {
    fn from((points, labels): (Vec<Point>, Vec<String>)) -> Self {
        Self {
            time: chrono::Utc::now(),
            points,
            labels
        }
    }
}

impl Default for PointCloud {
    fn default() -> Self {
        PointCloud {
            time: Utc::now(),
            points: Vec::new(),
            labels: Vec::new()
        }
    }
}

#[derive(Serialize, Deserialize)]
struct PointCloudHelper {
    time: DateTime<Utc>,
    x: Vec<f32>,
    y: Vec<f32>,
    z: Vec<f32>,
    v: Vec<f32>,
    l: Vec<String>
}

impl From<PointCloud> for PointCloudHelper {
    fn from(pc: PointCloud) -> Self {
        let (x, y, z, v): (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) =
            pc.points.into_iter().map(|p| (p.x, p.y, p.z, p.v)).unzip4(); // requires the itertools crate
        let l = pc.labels;
        PointCloudHelper {
            time: pc.time,
            x,
            y,
            z,
            v,
            l
        }
    }
}

impl From<PointCloudHelper> for PointCloud {
    fn from(helper: PointCloudHelper) -> Self {
        let points = helper
            .x
            .into_iter()
            .zip(helper.y.into_iter())
            .zip(helper.z.into_iter())
            .zip(helper.v.into_iter())
            .map(|(((x, y), z), v)| Point { x, y, z, v })
            .collect();
        let labels = helper.l;
        PointCloud {
            time: helper.time,
            points,
            labels,
        }
    }
}

impl serde::Serialize for PointCloud {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let helper: PointCloudHelper = self.clone().into();
        helper.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for PointCloud {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = PointCloudHelper::deserialize(deserializer)?;
        Ok(helper.into())
    }
}

// Helper function for unzipping tuples
trait Unzip4<A, B, C, D> {
    fn unzip4(self) -> (Vec<A>, Vec<B>, Vec<C>, Vec<D>);
}

impl<I, A, B, C, D> Unzip4<A, B, C, D> for I
where
    I: Iterator<Item = (A, B, C, D)>,
{
    fn unzip4(self) -> (Vec<A>, Vec<B>, Vec<C>, Vec<D>) {
        let mut x = Vec::new();
        let mut y = Vec::new();
        let mut z = Vec::new();
        let mut v = Vec::new();
        for (a, b, c, d) in self {
            x.push(a);
            y.push(b);
            z.push(c);
            v.push(d);
        }
        (x, y, z, v)
    }
}
