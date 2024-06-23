use std::{default, ops::Deref};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default, Copy, PartialEq, PartialOrd)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub v: f32,
}

impl From<[f32; 4]> for Point {
    fn from(value: [f32; 4]) -> Self {
        Self {
            x: value[0],
            y: value[1],
            z: value[2],
            v: value[3],
        }
    }
}

impl From<[f32; 3]> for Point {
    fn from(value: [f32; 3]) -> Self {
        Self {
            x: value[0],
            y: value[1],
            z: value[2],
            ..Default::default()
        }
    }
}

impl Into<[f32; 3]> for Point {
    fn into(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

impl Into<[f32; 4]> for Point {
    fn into(self) -> [f32; 4] {
        [self.x, self.y, self.z, self.v]
    }
}
