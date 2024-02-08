use std::{fs::OpenOptions, io::BufReader, net::SocketAddr, sync::Arc, time::Duration};

use axum::{routing::get, Router};
use mmwave::core::{accumulator::Accumulator, config::Configuration, pointcloud::PointCloud};
use serde::{Deserialize, Serialize};
use tokio::sync::{
    mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender},
    watch::{self, Receiver as WatchReceiver, Sender as WatchSender},
    Mutex,
};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct PointCloudMessage {
    pub pointclouds: Vec<PointCloud>,
}

struct AppState {
    config: Arc<Mutex<Configuration>>,
    accumulator: Arc<Mutex<Accumulator>>,
}

#[tokio::main]
async fn main() {
    // Get the saved configuration
    let config_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true) // This will create the file if it doesn't exist.
        .open("./config.json");

    let config_file = config_file.expect("Failed to open or create './config.json'");
    let reader = BufReader::new(config_file);
    let config: Configuration = serde_json::from_reader(reader).expect("Unable to read config");
    dbg!(&config);
    let config = Arc::new(Mutex::new(config));

    let accumulator = Arc::new(Mutex::new(Accumulator::new(1000)));

    // Initialize the app state
    let app_state = Arc::new(AppState {
        config,
        accumulator,
    });

    // Spawn a task to start handling the accumulator
    let (mpsc_tx, mpsc_rx) = mpsc::channel::<PointCloudMessage>(100);
    let (watch_tx, watch_rx) = watch::channel::<PointCloudMessage>(PointCloudMessage::default());
    let accumulator = app_state.accumulator.clone();
    tokio::task::spawn(handle_accumulator(accumulator, mpsc_rx, watch_tx));

    // Set up the axum router
    let app = Router::new()
        .route("/ws", get(websocket_handler))
        .route("/get_config", get(get_config_handler))
        .route("/set_config", axum::routing::post(set_config_handler))
        .with_state(app_state);

    // Listen on port 3000
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn handle_accumulator(
    accumulator: Arc<Mutex<Accumulator>>,
    mut rx: MpscReceiver<PointCloudMessage>,
    tx: WatchSender<PointCloudMessage>,
) {
    let mut clean_interval = tokio::time::interval(Duration::from_secs(15));
    let mut peek_interval = tokio::time::interval(Duration::from_millis(100));

    loop {
        tokio::select! {
            recieved = rx.recv() => {
                if let Some(point_cloud_message) = recieved {
                    let point_clouds = point_cloud_message.pointclouds;
                    accumulator.lock().await.push_multiple(point_clouds);
                    accumulator.lock().await.reorganize();
                } else {
                    eprintln!("Accumulator Stopped");
                    return;
                }
            },
            _ = clean_interval.tick() => {
                let popped_data = accumulator.lock().await.pop_finished();
                // TODO Replace this with something that writes to a database
                // let file_path = file_path.clone();
                // tokio::task::spawn(async move {
                //     if !popped_data.is_empty() {
                //         let mut file = std::fs::OpenOptions::new().read(true).open(&file_path).unwrap_or_else(|_| File::create(&file_path).unwrap());
                //         let mut contents = String::new();
                //         file.read_to_string(&mut contents).expect("Unable to read file");
                //         let mut existing_data: Vec<PointCloud> = serde_json::from_str(&contents).unwrap_or_else(|_| Vec::new());

                //         // Append new data
                //         let popped_len = popped_data.len();
                //         existing_data.extend(popped_data);

                //         // Reserialize and write back
                //         let serialized_data = serde_json::to_string(&existing_data).expect("Unable to serialize data");
                //         let mut file = File::create(&file_path).expect("Unable to create file");
                //         file.write_all(serialized_data.as_bytes()).expect("Unable to write data");

                //         println!("Updated out.json with {} items", popped_len);
                //     }
                // });
            },
            _ = peek_interval.tick() => {
                // Send the most recently complete pointcloud to all clients on a regular interval
                let mut accumulator = accumulator.lock().await;
                if accumulator.peekable() {
                    if let Some(pointcloud) = accumulator.peek() {
                        tx.send(PointCloudMessage {
                            pointclouds: vec![pointcloud]
                        });
                    }
                }
            }
        }
    }
}
