use super::{config::Configuration, data::Data};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fmt::{self, Display, Formatter},
    num::ParseIntError,
    str::FromStr,
};

#[derive(Serialize, Deserialize, Debug, Hash, Clone, Eq, PartialEq)]
pub enum Destination {
    /// Message for all clients (not server)
    Global,
    /// Message for a specific machine
    Id(Id),
    /// Messages intended for a relay host
    Manager,
    /// Message for all sensors
    Sensor,
    /// Message for server
    Server,
    /// Message for visualisers
    Visualiser,
    /// Message for writer
    Writer,
    /// Message for anything interested in data
    DataListener,
}

impl Display for Destination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Destination::Global => write!(f, "global"),
            Destination::Id(id) => write!(f, "id({})", id),
            Destination::Manager => write!(f, "manager"),
            Destination::Sensor => write!(f, "sensor"),
            Destination::Server => write!(f, "server"),
            Destination::Visualiser => write!(f, "visualiser"),
            Destination::Writer => write!(f, "writer"),
            Destination::DataListener => write!(f, "datalistener"),
        }
    }
}

#[derive(Hash, Eq, PartialEq, Serialize, Deserialize, Debug, Clone, Copy)]
pub enum Id {
    Machine(usize),
    Device(usize, usize),
}

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Id::Machine(id) => write!(f, "{}", id),
            Id::Device(m, d) => write!(f, "{}:{}", m, d),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MessageContent {
    /// A message containing data
    DataMessage(Data),
    /// A message containing a config
    ConfigMessage(Configuration),
    /// Requests the config be sent to the provided destination
    ConfigRequest(Destination),
    /// Register all given ids to listen for messages going to the listed destinations
    RegisterId(HashSet<Id>, HashSet<Destination>),
    /// Deregisters a registered ID
    DeregisterId(Id, HashSet<Destination>),
    /// Tells the receiving machine/device to reboot
    Reboot,
}

impl Display for MessageContent {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            MessageContent::DataMessage(_) => write!(f, "DataMessage"),
            MessageContent::ConfigMessage(_) => write!(f, "ConfigMessage"),
            MessageContent::ConfigRequest(dest) => write!(f, "ConfigRequest({:?})", dest),
            MessageContent::RegisterId(id, dests) => {
                write!(f, "RegisterId({:?}, {:?})", id, dests)
            }
            MessageContent::DeregisterId(id, dests) => {
                write!(f, "DeregisterId({:?}, {:?})", id, dests)
            }
            MessageContent::Reboot => write!(f, "Reboot"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub content: MessageContent,
    pub destination: HashSet<Destination>,
    pub timestamp: DateTime<Utc>,
}

impl Display for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Message {{ content: {}, destination: {:?}, timestamp: {} }}",
            self.content, self.destination, self.timestamp
        )
    }
}

pub enum IdParseError {
    InvalidFormat,
    ParseIntError(ParseIntError),
}

impl FromStr for Id {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<usize>().map(Id::Machine)
    }
}
