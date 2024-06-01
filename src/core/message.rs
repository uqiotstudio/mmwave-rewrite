use super::{config::Configuration, data::Data};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, num::ParseIntError, str::FromStr};

#[derive(Serialize, Deserialize, Debug, Hash, Clone, Eq, PartialEq)]
pub enum Destination {
    /// Message for all clients (not server)
    Global,
    /// Message for a specific machine
    Id(Id),
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

#[derive(Hash, Eq, PartialEq, Serialize, Deserialize, Debug, Clone, Copy)]
pub enum Id {
    Machine(usize),
    Device(usize, usize),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MessageContent {
    /// A message containing data
    DataMessage(Data),
    /// A message containing a config
    ConfigMessage(Configuration),
    /// Requests the config be sent to the provided destination
    ConfigRequest(Destination),
    /// The producing client will be registered to listen for messages sent to the provided destination
    RegisterId(Id, HashSet<Destination>),
    /// Deregisters a registered ID
    DeregisterId(Id, HashSet<Destination>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub content: MessageContent,
    pub destination: HashSet<Destination>,
    pub timestamp: DateTime<Utc>,
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
