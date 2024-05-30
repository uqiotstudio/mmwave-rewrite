use futures_util::{pin_mut, SinkExt, StreamExt, TryFutureExt};
use mmwave::{
    core::{
        config::Configuration,
        message::MachineId,
        pointcloud::{IntoPointCloud, PointCloudLike},
        transform::Transform,
    },
    sensors::{SensorConfig, SensorDescriptor},
};
use ndarray::AssignElem;
use reqwest::Url;
use searchlight::{
    discovery::{DiscoveryBuilder, DiscoveryEvent},
    dns::rr::RData,
    net::IpVersion,
};
use serde_json::{self, from_str};
use std::{
    collections::{HashMap, HashSet},
    env,
    net::{IpAddr, SocketAddr, SocketAddrV6, ToSocketAddrs},
    ops::{Add, AddAssign, Mul, MulAssign},
    str::FromStr,
    sync::{atomic::AtomicU32, Arc},
    thread,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::{select, sync::oneshot::Receiver};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug)]
struct SensorClient {
    descriptor: SensorConfig,
    kill_signal: oneshot::Sender<()>,
}

struct AppState {
    server_address: ServerAddress,
    machine_id: MachineId,
    sensors: HashMap<SensorDescriptor, SensorClient>,
    pointcloud_sender: mpsc::Sender<PointCloudLike>,
}

async fn discover_service() -> SocketAddr {
    let (found_tx, found_rx) = std::sync::mpsc::sync_channel(10);
    let discovery = DiscoveryBuilder::new()
        .loopback()
        .service("_http._tcp.local")
        .unwrap()
        .build(IpVersion::Both)
        .unwrap()
        .run_in_background(move |event| {
            if let DiscoveryEvent::ResponderFound(responder) = event {
                let (name, port) = responder
                    .last_response
                    .additionals()
                    .iter()
                    .find_map(|record| {
                        if let Some(RData::SRV(srv)) = record.data() {
                            let name = record.name().to_utf8();
                            let port = srv.port();
                            Some((name.to_string(), port))
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| ("Unknown".into(), 0));

                if name.contains("mmwaveserver") {
                    dbg!(&responder);
                    println!(
                        "Located dns record for server:\nName:{}\nIp Addr:{}\nPort:{}",
                        name,
                        responder.addr.ip(),
                        port
                    );
                    if responder.addr.is_ipv4() {
                        // Ipv6 Has issues setting up http request urls
                        let mut addr = responder.addr;
                        addr.set_port(port);
                        found_tx.send(addr);
                        return;
                    }
                }
            }
        });

    let server = found_rx.recv().expect("Lost mDNS discovery channel");
    dbg!(server);

    discovery.shutdown();
    return server;
}

#[derive(Clone, Copy, Debug)]
struct ServerAddress {
    fixed: Option<SocketAddr>,
    dynamic: SocketAddr,
}

impl ServerAddress {
    async fn new() -> Self {
        let args: Vec<String> = env::args().collect();
        let ip_override = args
            .get(1)
            .cloned()
            .map(|s| IpAddr::from_str(&s).expect("Arg 1 Is An Invalid Ip Address"));
        let port_override = args
            .get(2)
            .cloned()
            .map(|s| s.parse::<u16>().expect("Arg 2 should be a port"));

        let fixed = match (ip_override, port_override) {
            (Some(ip), Some(port)) => Some(SocketAddr::new(ip, port)),
            _ => None,
        };

        let dynamic = match &fixed {
            Some(address) => *address,
            None => {
                let address = discover_service().await;
                address
            }
        };

        Self { fixed, dynamic }
    }

    async fn refresh(&mut self) {
        self.dynamic = match self.fixed {
            Some(address) => {
                println!("Address is fixed, no action taken.");
                address
            }
            None => {
                println!("Attempting to locate a service on the local network.");
                let address = discover_service().await;
                address
            }
        };
        dbg!(self.dynamic);
    }

    fn address(&self) -> SocketAddr {
        self.fixed.unwrap_or_else(|| self.dynamic)
    }

    fn url(&self) -> Url {
        Url::parse(&format!("http://{}", self.address())).expect("Unable to parse url")
    }

    fn get_config_url(&self) -> Url {
        Url::parse(&format!("{}get_config", self.url())).expect("Unable to parse get_config url")
    }

    fn set_config_url(&self) -> Url {
        Url::parse(&format!("{}set_config", self.url())).expect("Unable to parse sent_config url")
    }

    fn ws_url(&self) -> Url {
        Url::parse(&format!("ws:///{}/ws", self.address())).expect("Unable to parse websocket url")
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    // If these variables are set, we NEVER look for a dns service

    let server_address = ServerAddress::new().await;

    let machine_id = MachineId(
        args.get(3)
            .cloned()
            .unwrap_or("0".to_owned())
            .parse()
            .expect("Requires Positive Integer for machine_id"),
    );

    dbg!(server_address.address());
    println!("{}", server_address.address());
    println!("{}", format!("http://{}", server_address.address()));

    let sensors = HashMap::<SensorDescriptor, SensorClient>::new();

    let (mpsc_tx, mpsc_rx) = mpsc::channel::<PointCloudLike>(100);

    let app_state = AppState {
        server_address: server_address.clone(),
        machine_id,
        sensors,
        pointcloud_sender: mpsc_tx,
    };

    // Periodically check for config updates
    // Filter out sensors with different machine id. Of those that remain:
    // - If a sensor is no longer listed, kill it
    // - If a sensor is added, spawn it
    // - If a sensors descriptor (not transform) is changed, reboot it
    let mut maintenance_task = tokio::task::spawn(config_maintainer(app_state));
    let mut forwarding_task =
        tokio::task::spawn(pointcloud_forwarding(mpsc_rx, server_address.ws_url()));

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
        mut server_address,
        machine_id,
        mut sensors,
        mut pointcloud_sender,
    }: AppState,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(1000));

    loop {
        interval.tick().await;

        // Get a response from the url
        let Ok(resp) = reqwest::get(server_address.get_config_url()).await else {
            eprintln!("Unable to send get request, is the server not running?");
            server_address.refresh().await;
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
                        machine_id: machine_id.clone(),
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
