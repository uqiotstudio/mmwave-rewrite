use serde::{Deserialize, Serialize};

pub trait IntoPointCloud {
    fn intoPointCloud(&mut self) -> PointCloud;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PointCloud {}

impl PointCloud {
    pub fn extend(&mut self, other: PointCloud) {
        todo!()
    }
}

impl Default for PointCloud {
    fn default() -> Self {
        PointCloud {}
    }
}
