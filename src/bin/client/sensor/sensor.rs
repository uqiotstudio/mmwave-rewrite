mod address;
mod args;

use chrono::{DateTime, Utc};
use clap::Parser;
use futures_util::{FutureExt, SinkExt, StreamExt};
use mmwave::{
    core::{
        config::Configuration,
        data::Data,
        message::{self, Destination, Id, Message},
        pointcloud::IntoPointCloud,
        transform::Transform,
    },
    sensors::{SensorConfig, SensorDescriptor},
};

use serde_json::{self};
use std::{
    collections::{HashMap, HashSet},
    ops::{AddAssign, MulAssign},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio::{select, sync::oneshot::Receiver};
use tokio_tungstenite::connect_async;
use tracing::{debug, error, info, instrument, level_filters::LevelFilter, warn};
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::{address::ServerAddress, args::Args};

#[derive(Debug)]
struct SensorClient {
    descriptor: SensorConfig,
    kill_signal: oneshot::Sender<()>,
}

struct AppState {
    server_address: ServerAddress,
    id: Id,
}

#[tokio::main]
async fn main() {
    // set up logging with tracing & indicatif
    let indicatif_layer = IndicatifLayer::new();

    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .expect("Should be impossible")
        .add_directive("mycrate=debug".parse().expect("Should be impossible"));

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(indicatif_layer.get_stderr_writer()))
        .with(indicatif_layer)
        .with(filter)
        .init();

    let args = Args::parse();
    let server_address = ServerAddress::new(args.clone()).await;
    let id = args.machine_id;

    info!(ip = ?server_address.address(), url = %server_address.url(), "server_address");

    let app_state = AppState {
        server_address: server_address.clone(),
        id,
    };

    // This should never end, ideally
    relay(app_state).await;
}

#[instrument(skip_all, fields(address=%server_address.address(), machine=?id))]
async fn relay(
    AppState {
        mut server_address,
        id,
    }: AppState,
) {
    let (inbound_tx, inbound_rx) = mpsc::channel::<Message>(100);
    let (outbound_tx, _) = broadcast::channel::<Message>(100);
    let (pointcloud_tx, pointcloud_rx) = mpsc::channel::<Data>(100);

    // Spawn the producer and inbound_handler
    tokio::task::spawn(inbound_handler(
        AppState { server_address, id },
        inbound_rx,
        pointcloud_tx,
    ));
    tokio::task::spawn(producer(
        AppState { server_address, id },
        outbound_tx.clone(),
        pointcloud_rx,
    ));

    // polling rate
    let mut interval = tokio::time::interval(Duration::from_millis(1000));
    loop {
        interval.tick().await;

        // Refresh the address if it is dynamic
        server_address.refresh();

        // Connect the WS
        let (mut ws_tx, mut ws_rx) = match connect_async(server_address.url_ws()).await {
            Ok((stream, _)) => stream,
            Err(e) => {
                warn!("Unable to connect to server");
                continue;
            }
        }
        .split();

        // Forward incoming signals to the manager
        let inbound_tx = inbound_tx.clone();
        let mut inbound_task = tokio::task::spawn(async move {
            loop {
                let Some(message) = ws_rx.next().await else {
                    continue;
                };

                let Ok(message) = message else {
                    break;
                };

                let Ok(message) = serde_json::from_str(&message.to_string()) else {
                    error!(text = message.to_string(), "Unable to parse message");
                    break;
                };

                inbound_tx.send(message).await;
            }
        });

        // Forward outgoing signals to the websocket
        let mut outbound_rx = outbound_tx.subscribe();
        let mut outbound_task = tokio::task::spawn(async move {
            loop {
                let Ok(message) = outbound_rx.recv().await else {
                    break;
                };

                let Ok(message) = serde_json::to_string(&message) else {
                    error!(message = ?message, "Unable to encode message");
                    break;
                };

                info!(message = message, "Sending Message");

                ws_tx
                    .send(tokio_tungstenite::tungstenite::Message::Text(message))
                    .await;
            }
        });

        let i = match id {
            Id::Machine(i) => i,
            Id::Device(_, _) => todo!(),
        };

        // Send any initialization messages
        let _ = outbound_tx.send(Message {
            content: message::MessageContent::RegisterId(
                HashSet::from([id, Id::Device(i, 1), Id::Device(i, 2)]),
                HashSet::from([
                    Destination::Id(id.clone()),
                    Destination::DataListener,
                    Destination::Sensor,
                ]),
            ),
            destination: HashSet::from([Destination::Server]),
            timestamp: Utc::now(),
        });

        let _ = outbound_tx.send(Message {
            content: message::MessageContent::ConfigRequest(Destination::Id(id)),
            destination: HashSet::from([Destination::Server]),
            timestamp: Utc::now(),
        });

        // This should go until one closes
        tokio::select! {
            _ = &mut inbound_task => {
                warn!("Inbound Task Closed");
                outbound_task.abort();
            }
            _ = &mut outbound_task => {
                warn!("Inbound Task Closed");
                inbound_task.abort();
            }
        }
    }
}

#[instrument(skip_all, fields(address=%server_address.address(), machine=?id))]
async fn inbound_handler(
    AppState {
        mut server_address,
        id,
    }: AppState,
    mut receiver: tokio::sync::mpsc::Receiver<Message>,
    mut pointcloud_sender: tokio::sync::mpsc::Sender<Data>,
) {
    let mut sensors = HashMap::<SensorDescriptor, SensorClient>::new();

    loop {
        let Some(message) = receiver.recv().await else {
            error!("receiver closed");
            continue;
        };

        match message.content {
            message::MessageContent::DataMessage(_) => {
                error!("Received Unsupported Message");
            }
            message::MessageContent::ConfigMessage(mut config) => {
                info!("Received config message");
                update_config(
                    AppState { server_address, id },
                    config,
                    &mut sensors,
                    &mut pointcloud_sender,
                )
                .await;
            }
            message::MessageContent::ConfigRequest(_) => {
                error!("Received Unsupported Message");
            }
            message::MessageContent::RegisterId(_, _) => {
                error!("Received Unsupported Message");
            }
            message::MessageContent::DeregisterId(_, _) => todo!(),
        }
    }
}

async fn update_config(
    AppState {
        mut server_address,
        id: machine_id,
    }: AppState,
    mut updated_config: Configuration,
    sensors: &mut HashMap<SensorDescriptor, SensorClient>,
    pointcloud_sender: &mut mpsc::Sender<Data>,
) {
    // We have a valid configuration. Filter it by our machine id
    updated_config
        .descriptors
        .retain(|cfg| cfg.machine_id == machine_id);

    let updated_sensors = updated_config.descriptors;
    let (mut updated_sensors_desc, mut updated_sensors_trans): (Vec<_>, Vec<_>) = updated_sensors
        .into_iter()
        .map(|sensor| (sensor.sensor_descriptor, sensor.transform))
        .unzip();

    // Anything in this set is flagged for removal at the end of loop
    // Because we index by a sensor descriptors hash, any changes to its internals will cause it to appear as a new device, thus keeping it in the removal flags *and* spawning up a new instance. This works well enough for reloading. As such, we only need to update the surroudning data. Because machine_id is filtered out already, it has the same effect as the hash. The only other remaining element to update without causing a removal is the transform.
    let mut removal_flags: HashSet<SensorDescriptor> = sensors.keys().cloned().collect();

    let mut changelog = Vec::new();

    // Here we remove any sensors that have the same hash from the removal set (mark them to be kept).
    // In addition, for sensors that are being kept, we update their transformation.
    // While doing this we remove any sensors from the updated_sensors list too, so that at the end we may iterate that list to spawn the new sensors.
    // All of this logic kind of depends on hash being the same as eq for the sensordescriptor
    for desc in sensors.keys().cloned().collect::<Vec<_>>() {
        if let Some(index) = updated_sensors_desc.iter().position(|n| *n == desc) {
            changelog.push(desc.title());
            removal_flags.remove(&desc);

            // Update the transform to match the new version
            let sensor_client = sensors
                .get_mut(&desc)
                .expect("This is an unreachable error");
            sensor_client.descriptor.transform = updated_sensors_trans[index].clone();

            // We saw this, so it is NOT new, remove it from the list of spawns
            updated_sensors_desc.swap_remove(index);
            updated_sensors_trans.swap_remove(index);
        }
    }

    info!("Config Maintenance Report:");
    for title in changelog {
        info!(descriptor = title, "Maintaining");
    }
    for desc in &removal_flags {
        info!(descriptor = desc.title(), "Killing");
    }
    for desc in &updated_sensors_desc {
        info!(descriptor = desc.title(), "Spawning");
    }

    // Kill any sensors that were removed or changed:
    for key in removal_flags {
        if let Some(sensorclient) = sensors.remove(&key) {
            info!(
                "Killed sensorclient {:?} with result: {:?}",
                sensorclient.descriptor.title(),
                sensorclient.kill_signal.send(())
            );
        }
    }

    // Spawn any sensors that were changed or new
    for (desc, trans) in updated_sensors_desc.iter().zip(updated_sensors_trans) {
        let (tx, rx) = oneshot::channel();
        tokio::task::spawn(desc_maintainer(
            desc.clone(),
            trans.clone(),
            rx,
            pointcloud_sender.clone(),
        ));
        sensors.insert(
            desc.clone(),
            SensorClient {
                descriptor: SensorConfig {
                    machine_id: machine_id.clone(),
                    sensor_descriptor: desc.clone(),
                    transform: trans.clone(),
                },
                kill_signal: tx,
            },
        );
    }
}

/// Maintains a sensor given a descriptor. This includes:
/// - making sure the sensor is running (restarts in event of failure automatically)
/// - Forwarding the sensor pointcloudlike results back to the client task
/// - shutting down the client properly in the event of a kill signal
async fn desc_maintainer(
    sensor_descriptor: SensorDescriptor,
    transform: Transform,
    mut kill_receiver: Receiver<()>,
    pointcloud_sender: mpsc::Sender<Data>,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    let mut sensor = None;
    loop {
        interval.tick().await;

        if kill_receiver.try_recv().is_ok() {
            info!("Killing Receiver");
            return;
        }

        // Attempt to connect
        let Some(sensor) = sensor.as_mut() else {
            info!(
                "Attempting to initialize sensor {}",
                sensor_descriptor.title()
            );
            let result = sensor_descriptor.try_initialize();
            if let Err(e) = &result {
                error!(
                    "Unable to initialize sensor {} with result {:?}",
                    sensor_descriptor.title(),
                    e
                );
            };
            info!(
                "Succesfully initialied sensor {}",
                sensor_descriptor.title()
            );
            sensor = result.ok();
            continue;
        };
        info!("Re/Connected to {}", sensor_descriptor.title());

        // read the sensor and forward
        loop {
            match sensor.try_read() {
                Ok(data) => {
                    // Apply the transformation, and convert into a typical pointcloud
                    let mut pointcloud = data.into_point_cloud();
                    pointcloud.points = pointcloud
                        .points
                        .iter_mut()
                        .map(|pt| {
                            let transformed = transform.apply([pt[0], pt[1], pt[2]]);
                            [transformed[0], transformed[1], transformed[2], pt[3]]
                        })
                        .collect();
                    if let Err(e) = pointcloud_sender.send(Data::PointCloud(pointcloud)).await {
                        error!("Channel closed");
                        return; // This is unrecoverable
                    }
                }
                Err(e) => {
                    error!(
                        "Error reading from sensor {:?}: {:?}",
                        sensor_descriptor.title(),
                        e
                    );
                    match e {
                        mmwave::sensors::SensorReadError::Critical => break,
                        mmwave::sensors::SensorReadError::Benign => {
                            continue;
                        }
                    }
                    // TODO handle this error, possibly reboot the device on certain kinds of failure
                    // TODO probably redefine how the error enum is defined to be 2 variants: requires_reboot(String) and negligible(String) or the like. Maybe critical/benign?
                }
            }
        }
    }
}

#[instrument(skip_all)]
async fn producer(
    AppState {
        server_address: _,
        id: _,
    }: AppState,
    sender: tokio::sync::broadcast::Sender<Message>,
    mut pointcloud_receiver: tokio::sync::mpsc::Receiver<Data>,
) {
    let frame_count = Arc::new(Mutex::new(0));
    let cloned_frame_count = frame_count.clone();
    tokio::task::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(10000));
        loop {
            interval.tick().await;
            info!(
                "Sent {} frames in the last 10 seconds",
                cloned_frame_count.lock().await
            );
            cloned_frame_count.lock().await.mul_assign(0);
        }
    });

    loop {
        match pointcloud_receiver.recv().await {
            Some(data) => {
                let message = Message {
                    content: message::MessageContent::DataMessage(data),
                    destination: HashSet::from([Destination::DataListener]),
                    timestamp: Utc::now(),
                };

                if let Err(e) = sender.send(message) {
                    error!("Error sending message: {:?}", e);
                    break; // Breaks inner loop, causes reconnection in outer loop
                }
                let mut fc = frame_count.lock().await;
                fc.add_assign(1);
            }
            None => {
                error!("sender channel closed, terminating");
                return;
            }
        }
    }
}
