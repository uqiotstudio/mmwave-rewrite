use crate::config::RadarConfiguration;
use crate::error::RadarReadError;
use crate::message::Frame;
use crate::radar::Radar;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task;
use tokio::time::{timeout, Duration};
use tokio_stream::StreamExt;

pub struct Manager {
    config: RadarConfiguration,
    frame_receivers: Vec<mpsc::Receiver<Frame>>,
    kill_sender: watch::Sender<bool>,
    kill_receiver: watch::Receiver<bool>,
}

impl Manager {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Manager {
            config: RadarConfiguration {
                descriptors: Vec::new(),
            },
            frame_receivers: Vec::new(),
            kill_sender: tx,
            kill_receiver: rx,
        }
    }
    pub fn set_config(&mut self, config: RadarConfiguration) {
        self.config = config;
        // Whenever we update the config we need to reload all existing radars
        self.reload();
    }

    pub fn reload(&mut self) {
        // Send the kill signal to all radar nodes and then restart them
        // This could be improved in future to only modify nodes that have changed
        println!("Reloading the radar manager");
        self.kill_all().unwrap(); // TODO if this *somehow* fails, probably should auto restart it
        self.start();
        println!("Reload complete");
    }

    fn kill_all(&mut self) -> Result<(), watch::error::SendError<bool>> {
        println!("Sending kill signal to all radar instances");
        self.frame_receivers.drain(..);
        self.kill_sender.send(true)?;
        let (tx, rx) = watch::channel(false);
        self.kill_sender = tx;
        self.kill_receiver = rx;
        Ok(())
    }

    fn start(&mut self) {
        println!("Starting up all radar instances");
        for descriptor in self.config.descriptors.iter() {
            match descriptor.clone().try_initialize() {
                Ok(radar_instance) => {
                    let (tx, rx) = mpsc::channel(1);
                    println!("Radar instance {:?} spawned", descriptor);
                    task::spawn(radar_loop(radar_instance, self.kill_receiver.clone(), tx));
                    self.frame_receivers.push(rx);
                }
                Err(e) => {
                    println!(
                        "Unable to spawn instance {:?} with error {:?}",
                        descriptor, e
                    );
                }
            }
        }
    }

    pub async fn receive(&mut self) -> Vec<Frame> {
        // Receives a frame from each receiver, with a 50ms window for all radars to send before they are abandoned.
        let futures: Vec<_> = self
            .frame_receivers
            .iter_mut()
            .map(|rx| {
                let duration = Duration::from_millis(50);
                async move { timeout(duration, rx.recv()).await.ok().flatten() }
            })
            .collect();

        tokio_stream::iter(futures)
            .then(|future| future)
            .filter_map(|future| future)
            .collect::<Vec<_>>()
            .await
    }
}

async fn radar_loop(
    mut radar_instance: Radar,
    kill_receiver: watch::Receiver<bool>,
    sender: mpsc::Sender<Frame>,
) {
    while !*kill_receiver.borrow() {
        match radar_instance.read_frame() {
            Ok(frame) => {
                // Got a frame and continue reading
                match sender.send(frame).await {
                    Ok(_) => {}
                    Err(_) => {
                        eprintln!("Error sending frame to manager, disconnecting");
                        break;
                    }
                }
            }
            Err(RadarReadError::ParseError(e)) => {
                eprintln!("Parse error reading frame, {:?}", e);
            }
            Err(RadarReadError::Disconnected)
            | Err(RadarReadError::NotConnected)
            | Err(RadarReadError::Timeout) => {
                eprintln!("Connection to radar lost, attempting reconnection");
                radar_instance = radar_instance.reconnect();
            }
        }
    }
    println!(
        "Radar instance {:?} was killed",
        radar_instance.get_descriptor()
    )
}
