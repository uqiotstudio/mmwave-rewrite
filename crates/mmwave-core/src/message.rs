use async_nats::subject::ToSubject;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fmt::{self, Display, Formatter},
    num::ParseIntError,
    str::FromStr,
};
use thiserror::Error;

use crate::pointcloud::PointCloud;

#[derive(Serialize, Deserialize, Debug, Hash, Clone, Eq, PartialEq)]
pub enum Tag {
    Pointcloud,
    Id(Id),
}

#[derive(Hash, Eq, PartialEq, Serialize, Deserialize, Debug, Clone, Copy)]
pub enum Id {
    Machine(usize),
    Device(usize, usize),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MessageContent {
    PointCloud(PointCloud),
    Empty,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub content: MessageContent,
    pub tags: HashSet<Tag>,
    pub timestamp: DateTime<Utc>,
}

impl Default for Message {
    fn default() -> Self {
        Self {
            content: MessageContent::Empty,
            tags: HashSet::new(),
            timestamp: chrono::Utc::now(),
        }
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tag::Pointcloud => write!(f, "pointcloud"),
            Tag::Id(id) => write!(f, "id({})", id),
        }
    }
}

impl Id {
    pub fn to_machine(self) -> Self {
        match self {
            Id::Machine(m) => Id::Machine(m),
            Id::Device(m, d) => Id::Machine(m),
        }
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Id::Machine(id) => write!(f, "{}", id),
            Id::Device(m, d) => write!(f, "{}:{}", m, d),
        }
    }
}

impl Display for MessageContent {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            MessageContent::PointCloud(pointcloud) => write!(f, "pointcloud"),
            MessageContent::Empty => write!(f, "empty"),
        }
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Message {{ content: {}, tags: {:?}, timestamp: {} }}",
            self.content, self.tags, self.timestamp
        )
    }
}

#[derive(Debug, Error)]
pub enum IdParseError {
    #[error("Invalid format")]
    InvalidFormat,
    #[error("Parse int error: {0}")]
    ParseIntError(#[from] ParseIntError),
}

impl FromStr for Id {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s.parse::<usize>()?;
        Ok(Id::Machine(id))
    }
}

impl ToSubject for Tag {
    fn to_subject(&self) -> async_nats::Subject {
        self.to_string().into()
    }
}
