use super::{config::Configuration, data::Data};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{num::ParseIntError, str::FromStr};

#[derive(Serialize, Deserialize, Debug, Hash, Clone, Eq, PartialEq)]
pub enum Destination {
    /// Message for all clients (not server)
    Global,
    /// Message for a specific machine
    Machine(MachineId),
    /// Message for all sensors
    Sensor,
    /// Message for server
    Server,
    /// Message for visualiser
    Visualiser,
    /// Message for writer
    Writer,
}

#[derive(Hash, Eq, PartialEq, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct MachineId(pub usize);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MessageContent {
    /// A message containing data
    DataMessage(Data),
    /// A message containing a config
    ConfigMessage(Configuration),
    /// Requests the config be sent to the provided destination
    ConfigRequest(Destination),
    /// The producing client will be registered to listen for messages sent to the provided destination
    EstablishDestination(Destination),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub content: MessageContent,
    pub destination: Destination,
    pub timestamp: DateTime<Utc>,
}

impl FromStr for MachineId {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<usize>().map(MachineId)
    }
}
