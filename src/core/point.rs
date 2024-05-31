use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Point {
    pub(crate) x: u64,
    pub y: u64,
    pub z: u64,
    pub v: u64,
}
