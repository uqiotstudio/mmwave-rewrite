use futures_util::{SinkExt, TryFutureExt};
use mmwave::{
    core::{
        config::Configuration,
        pointcloud::{IntoPointCloud, PointCloudLike},
        transform::Transform,
    },
    sensors::{SensorConfig, SensorDescriptor},
};
use ndarray::AssignElem;
use serde_json;
use std::{
    collections::{HashMap, HashSet},
    env,
    ops::{Add, AddAssign, Mul, MulAssign},
    sync::{atomic::AtomicU32, Arc},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::{select, sync::oneshot::Receiver};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use reqwest::Url;

#[derive(Debug)]
struct SensorClient {
    descriptor: SensorConfig,
    kill_signal: oneshot::Sender<()>,
}

struct AppState {
    ip_address: String,
    machine_id: usize,
    url: Url,
    ws_url: Url,
    get_config_url: Url,
    set_config_url: Url,
    sensors: HashMap<SensorDescriptor, SensorClient>,
    pointcloud_sender: mpsc::Sender<PointCloudLike>,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    let ip_address = args.get(1).cloned().unwrap_or("localhost".to_owned());
    let machine_id: usize = args
        .get(2)
        .cloned()
        .unwrap_or("0".to_owned())
        .parse()
        .expect("Requires Positive Integer for machine_id");

    let url = Url::parse(&format!("http://{}:3000", ip_address)).expect("Unable to parse url");

    let ws_url = Url::parse(&format!("ws:///{}:3000/ws", ip_address))
        .expect("Unable to parse websocket url");
    let get_config_url =
        Url::parse(&format!("{}get_config", url)).expect("Unable to parse get_config url");
    let set_config_url =
        Url::parse(&format!("{}set_config", url)).expect("Unable to parse sent_config url");

    let sensors = HashMap::<SensorDescriptor, SensorClient>::new();

    let (mpsc_tx, mpsc_rx) = mpsc::channel::<PointCloudLike>(100);

    let app_state = AppState {
        ip_address,
        machine_id,
        url,
        ws_url: ws_url.clone(),
        get_config_url,
        set_config_url,
        sensors,
        pointcloud_sender: mpsc_tx,
    };

    // Periodically check for config updates
    // Filter out sensors with different machine id. Of those that remain:
    // - If a sensor is no longer listed, kill it
    // - If a sensor is added, spawn it
    // - If a sensors descriptor (not transform) is changed, reboot it
    let mut maintenance_task = tokio::task::spawn(config_maintainer(app_state));
    let mut forwarding_task = tokio::task::spawn(pointcloud_forwarding(mpsc_rx, ws_url));

    // This should go on forever!
    select! {
        _ = &mut maintenance_task => {
            eprintln!("config maintainer failed");
            forwarding_task.abort();
        }
        _ = &mut forwarding_task => {
            eprintln!("Forwarding task failed");
            maintenance_task.abort();
        }
    };
}

async fn config_maintainer(
    AppState {
        ip_address,
        machine_id,
        url,
        ws_url,
        get_config_url,
        set_config_url,
        mut sensors,
        mut pointcloud_sender,
    }: AppState,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(1000));

    loop {
        interval.tick().await;

        // Get a response from the url
        let Ok(resp) = reqwest::get(get_config_url.clone()).await else {
            eprintln!("Unable to send get request, is the server not running?");
            continue;
        };

        // Convert it into text
        let Ok(text) = resp.text().await else {
            continue;
        };

        // Parse a config
        let Ok(mut updated_config) = serde_json::from_str::<Configuration>(&text) else {
            eprintln!("Unable to parse config from server response");
            continue;
        };

        // We have a valid configuration. Filter it by our machine id
        updated_config
            .descriptors
            .retain(|cfg| cfg.machine_id == machine_id);

        let updated_sensors = updated_config.descriptors;
        let (mut updated_sensors_desc, mut updated_sensors_trans): (Vec<_>, Vec<_>) =
            updated_sensors
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

        println!("Config Maintenance Report:");
        for title in changelog {
            println!("\tMaintaining {}", title);
        }
        for desc in &removal_flags {
            println!("\tKilling {}", desc.title());
        }
        for desc in &updated_sensors_desc {
            println!("\tSpawning {}", desc.title());
        }

        // Kill any sensors that were removed or changed:
        for key in removal_flags {
            if let Some(sensorclient) = sensors.remove(&key) {
                println!(
                    "Killed sensorclient {:?} with result: {:?}",
                    sensorclient.descriptor.title(),
                    sensorclient.kill_signal.send(())
                );
            }
        }

        // Spawn any sensors that were changed or new
        for (desc, trans) in updated_sensors_desc.iter().zip(updated_sensors_trans) {
            let (tx, rx) = oneshot::channel();
            tokio::task::spawn(maintainer(
                desc.clone(),
                trans.clone(),
                rx,
                pointcloud_sender.clone(),
            ));
            sensors.insert(
                desc.clone(),
                SensorClient {
                    descriptor: SensorConfig {
                        machine_id: machine_id,
                        sensor_descriptor: desc.clone(),
                        transform: trans.clone(),
                    },
                    kill_signal: tx,
                },
            );
        }
    }
}

/// Maintains a sensor given a descriptor. This includes:
/// - making sure the sensor is running (restarts in event of failure automatically)
/// - Forwarding the sensor pointcloudlike results back to the client task
/// - shutting down the client properly in the event of a kill signal
async fn maintainer(
    sensor_descriptor: SensorDescriptor,
    transform: Transform,
    mut kill_receiver: Receiver<()>,
    pointcloud_sender: mpsc::Sender<PointCloudLike>,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    let mut sensor = None;
    loop {
        interval.tick().await;

        if kill_receiver.try_recv().is_ok() {
            println!("Killing Receiver");
            return;
        }

        // Attempt to connect
        let Some(sensor) = sensor.as_mut() else {
            println!(
                "Attempting to initialize sensor {}",
                sensor_descriptor.title()
            );
            let result = sensor_descriptor.try_initialize();
            if let Err(e) = &result {
                eprintln!(
                    "Unable to initialize sensor {} with result {:?}",
                    sensor_descriptor.title(),
                    e
                );
            };
            println!(
                "Succesfully initialied sensor {}",
                sensor_descriptor.title()
            );
            sensor = result.ok();
            continue;
        };
        println!("Re/Connected to {}", sensor_descriptor.title());

        // We have a mutable reference to the sensor. Attempt to read.
        loop {
            match sensor.try_read() {
                Ok(pointcloud_like) => {
                    // Apply the transformation, and convert into a typical pointcloud
                    let mut pointcloud = pointcloud_like.into_point_cloud();
                    pointcloud.points = pointcloud
                        .points
                        .iter_mut()
                        .map(|pt| {
                            let transformed = transform.apply([pt[0], pt[1], pt[2]]);
                            [transformed[0], transformed[1], transformed[2], pt[3]]
                        })
                        .collect();
                    if let Err(e) = pointcloud_sender
                        .send(PointCloudLike::PointCloud(pointcloud))
                        .await
                    {
                        return; // This is unrecoverable
                    }
                }
                Err(e) => {
                    eprintln!(
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

async fn pointcloud_forwarding(mut mpsc_rx: mpsc::Receiver<PointCloudLike>, ws_url: Url) {
    let frame_count = Arc::new(Mutex::new(0));
    let cloned_frame_count = frame_count.clone();
    tokio::task::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(10000));
        loop {
            interval.tick().await;
            println!(
                "Sent {} frames in the last 10 seconds",
                cloned_frame_count.lock().await
            );
            cloned_frame_count.lock().await.mul_assign(0);
        }
    });
    loop {
        let websocket = match connect_async(ws_url.clone()).await {
            Ok((stream, _)) => Some(stream),
            Err(e) => {
                eprintln!(
                    "Failed to connect to WebSocket with request \"{}\". Error was: {:?}",
                    ws_url, e
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }
        };

        if let Some(mut ws_stream) = websocket {
            loop {
                match mpsc_rx.recv().await {
                    Some(pointcloud_like) => {
                        match serde_json::to_string(&pointcloud_like) {
                            Ok(message) => {
                                if let Err(e) = ws_stream.send(Message::Text(message)).await {
                                    eprintln!("Error sending message: {:?}", e);
                                    break; // Breaks inner loop, causes reconnection in outer loop
                                }
                                let mut fc = frame_count.lock().await;
                                fc.add_assign(1);
                            }
                            Err(e) => eprintln!("Serialization error: {:?}", e),
                        }
                    }
                    None => {
                        eprintln!("Channel closed, terminating.");
                        return; // This is unrecoverable from in this scope
                    }
                }
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
