use async_nats::subject::ToSubject;
use chrono::{DateTime, Utc};
use egui::{DragValue, Ui, Widget};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display, Formatter},
    num::ParseIntError,
    str::FromStr,
};
use thiserror::Error;

use crate::pointcloud::PointCloud;

#[derive(Serialize, PartialOrd, Ord, Deserialize, Debug, Hash, Clone, Eq, PartialEq)]
pub enum Tag {
    Pointcloud,
    FromId(Id),
}

#[derive(Hash, Eq, PartialOrd, Ord, PartialEq, Serialize, Deserialize, Debug, Clone, Copy)]
pub enum Id {
    Device(usize, usize),
    Machine(usize),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MessageContent {
    PointCloud(PointCloud),
    Empty,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub content: MessageContent,
    pub tags: Vec<Tag>,
    pub timestamp: DateTime<Utc>,
}

impl Default for Message {
    fn default() -> Self {
        Self {
            content: MessageContent::Empty,
            tags: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }
}

pub trait TagsToSubject {
    fn to_subject(self) -> String;
}

impl TagsToSubject for Vec<Tag> {
    fn to_subject(mut self) -> String {
        self.sort();
        self.iter()
            .map(|tag| tag.to_string())
            .collect::<Vec<String>>()
            .join(".")
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tag::Pointcloud => write!(f, "Pointcloud"),
            Tag::FromId(id) => write!(f, "FromId({})", id),
        }
    }
}

impl Id {
    pub fn to_machine(self) -> Self {
        match self {
            Id::Machine(m) => Id::Machine(m),
            Id::Device(m, _d) => Id::Machine(m),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let Id::Device(m, d) = self else {
            ui.label("Machine ID editing not supported");
            return;
        };

        ui.horizontal(|ui| {
            ui.label("id:");
            ui.group(|ui| {
                ui.label("M:");
                DragValue::new(m).update_while_editing(true).ui(ui);
                ui.label("D:");
                DragValue::new(d).update_while_editing(true).ui(ui);
            });
        });
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
            MessageContent::PointCloud(_pointcloud) => write!(f, "pointcloud"),
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
