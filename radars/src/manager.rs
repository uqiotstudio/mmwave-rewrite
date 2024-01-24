use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task;
use tokio::time;
use tokio::time::{timeout, Duration};
use tokio_stream::StreamExt;

use crate::config::Configuration;
use crate::pointcloud::IntoPointCloud;
use crate::pointcloud::PointCloud;
use crate::pointcloud::PointCloudLike;
use crate::pointcloud_provider::PcPDescriptor;
use crate::pointcloud_provider::PointCloudProvider;

pub struct Manager {
    machine_id: usize,
    config: Configuration,
    pointcloud_sender: mpsc::Sender<PointCloudLike>,
    pointcloud_receiver: mpsc::Receiver<PointCloudLike>,
    read_window: u64,
    kill_sender: watch::Sender<bool>,
    kill_receiver: watch::Receiver<bool>,
}

impl Manager {
    pub fn new(machine_id: usize) -> Self {
        let (tx, rx) = watch::channel(false);
        let (tx2, rx2) = mpsc::channel(100);
        Manager {
            machine_id,
            config: Configuration {
                descriptors: Vec::new(),
            },
            pointcloud_sender: tx2,
            pointcloud_receiver: rx2,
            read_window: 100,
            kill_sender: tx,
            kill_receiver: rx,
        }
    }
    pub fn set_config(&mut self, config: Configuration) {
        if config != self.config {
            println!("Loading new config into manager");
            self.config = config;
            // Whenever we update the config we need to reload all existing radars
            self.reload();
        }
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
        self.kill_sender.send(true)?;
        let (tx, rx) = watch::channel(false);
        let (tx2, rx2) = mpsc::channel(100);
        self.pointcloud_sender = tx2;
        self.pointcloud_receiver = rx2;
        self.kill_sender = tx;
        self.kill_receiver = rx;
        Ok(())
    }

    fn start(&mut self) {
        println!("Starting up all radar instances");
        for descriptor in self.config.descriptors.iter() {
            if descriptor.machine_id != self.machine_id {
                continue;
            }
            match descriptor.clone().try_initialize() {
                Ok(radar_instance) => {
                    println!("Radar instance {:#?} spawned", descriptor);
                    task::spawn(radar_loop(
                        radar_instance,
                        self.kill_receiver.clone(),
                        self.pointcloud_sender.clone(),
                        descriptor.clone(),
                    ));
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

    pub async fn receive(&mut self) -> Vec<PointCloudLike> {
        // // Receives a frame from each receiver, with a timeout window for all radars to send before they are abandoned.
        let mut point_clouds = Vec::new();
        let deadline = time::Instant::now() + Duration::from_millis(self.read_window);

        loop {
            if time::Instant::now() > deadline {
                break;
            }
            let msg = match self.pointcloud_receiver.try_recv() {
                Ok(m) => m,
                Err(_) => continue,
            };
            point_clouds.push(msg);
        }
        point_clouds
    }
}

async fn radar_loop(
    mut provider: Box<dyn PointCloudProvider>,
    kill_receiver: watch::Receiver<bool>,
    sender: mpsc::Sender<PointCloudLike>,
    descriptor: PcPDescriptor,
) {
    while !*kill_receiver.borrow() {
        match provider.try_read() {
            Ok(frame) => {
                // Got a frame and continue reading
                let mut frame = frame.into_point_cloud();
                frame.points = frame
                    .points
                    .iter()
                    .map(|p| {
                        let p2 = descriptor.transform.unapply([p[0], p[1], p[2]]);
                        [p2[0], p2[1], p2[2], p[3]]
                    })
                    .collect();
                match sender.send(PointCloudLike::PointCloud(frame)).await {
                    Ok(_) => {}
                    Err(_) => {
                        eprintln!("Error sending frame to manager, disconnecting");
                        break;
                    }
                }
            }
            Err(e) => {
                // In this event, the pointcloud provider has tried to recover and failed, so we need to kill this process
                eprintln!("Provider failed with error {:?}, reinitializing", e);
                // TODO this requires the provider to be dropped to really work in SOME cases, which can cause infinite reinitialization. We need to promise the compilerl that provider can be dropped, and then reassign it.
                provider = match descriptor.clone().try_initialize() {
                    Ok(p) => {
                        println!("Successfully reinitialized provider");
                        p
                    }
                    Err(e) => {
                        eprintln!(
                            "Failed to reinitialize with error {:?}, trying again later",
                            e
                        );
                        continue;
                    }
                }
            }
        }
    }
    println!("A radar instance was killed")
}
