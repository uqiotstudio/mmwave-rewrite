use super::message::{Destination, Id, Message};
use std::collections::{HashMap, HashSet};
use tokio::sync::broadcast::{self};
use tracing::{debug, error, info, instrument, warn};

#[derive(Debug)]
pub struct Relay<T> {
    destination_to_address: HashMap<Destination, HashSet<Id>>,
    address_to_destination: HashMap<Id, HashSet<Destination>>,
    channels: HashMap<Id, broadcast::Sender<T>>,
}

impl<T: Clone + std::fmt::Debug + std::fmt::Display> Relay<T> {
    /// Creates a new relay
    pub fn new() -> Self {
        info!("Creating a new Relay");
        Relay {
            destination_to_address: HashMap::new(),
            address_to_destination: HashMap::new(),
            channels: HashMap::new(),
        }
    }

    /// Registers the given id to a destination in the relay
    /// This means messages intended for the destination will be
    /// forwarded to the identified device.
    pub fn register(&mut self, id: Id, destination: Destination) {
        debug!("Registering id to destination");
        self.destination_to_address
            .entry(destination.clone())
            .or_insert_with(HashSet::new)
            .insert(id.clone());

        self.address_to_destination
            .entry(id.clone())
            .or_insert_with(HashSet::new)
            .insert(destination.clone());

        if !self.channels.contains_key(&id) {
            let (tx, _rx) = broadcast::channel(100);
            self.channels.insert(id, tx);
        }
        info!("ID {:?} registered to destination {:?}", id, destination);
    }

    /// Deregisters the destination from the given id.
    pub fn deregister(&mut self, id: Id, destination: Destination) {
        debug!("Deregistering destination from id");

        if let Some(destinations) = self.destination_to_address.get_mut(&destination) {
            destinations.remove(&id);
            if destinations.is_empty() {
                self.destination_to_address.remove(&destination);
            }
        }

        if let Some(ids) = self.address_to_destination.get_mut(&id) {
            ids.remove(&destination);
            if ids.is_empty() {
                self.address_to_destination.remove(&id);
            }
        }

        info!(
            "Deregistered destination {:?} from ID {:?}",
            destination, id
        );
    }

    /// Forwards the given message to all destinations in the set.
    pub fn forward(&self, destinations: HashSet<Destination>, message: T) {
        debug!(
            "Forwarding message {:?} to destinations {:?}",
            message, destinations
        );

        for destination in &destinations {
            if let Some(ids) = self.destination_to_address.get(destination) {
                for id in ids {
                    if let Some(channel) = self.channels.get(id) {
                        if channel.send(message.clone()).is_err() {
                            error!("Failed to send message to ID {:?}", id);
                            debug!("Relay state: {:?}", self);
                            debug!("Channel state: {:?}", channel);
                            debug!("Channel recv count: {:?}", channel.receiver_count());
                        } else {
                            debug!("Sent message to ID {:?}", id);
                        }
                    } else {
                        warn!("No channel found for ID {:?}", id);
                    }
                }
            } else {
                warn!("No IDs found for destination {:?}", destination);
            }
        }

        info!(
            "Forwarded message {} to {} destinations: {:?}",
            message,
            destinations.len(),
            destinations
        );
    }

    /// Returns a receiver for the given id. Any messages forward to this device
    /// will be received here. None if there is no id registered.
    pub fn subscribe(&mut self, id: Id) -> broadcast::Receiver<T> {
        debug!("Subscribing to id {:?}", id);
        self.channels
            .entry(id)
            .or_insert(broadcast::channel(100).0)
            .subscribe()
    }

    pub fn subscribed(&mut self, id: Id) -> HashSet<Destination> {
        self.address_to_destination
            .entry(id)
            .or_insert(HashSet::new())
            .clone()
    }
}
