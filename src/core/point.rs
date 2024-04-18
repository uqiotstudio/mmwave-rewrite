use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Point {
    x: u64,
    y: u64,
    z: u64,
    v: u64,
}
