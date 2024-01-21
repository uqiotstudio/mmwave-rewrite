pub mod buffer;
pub mod message;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use message::{ConfigMessage, PointCloudMessage, ServerMessage};
use radars::{config::Configuration, pointcloud::PointCloud};
use std::{
    fs::File,
    io::{BufReader, Read, Write},
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{
    mpsc::{self, Receiver, Sender},
    Mutex,
};
use tokio_stream::StreamExt;

use crate::buffer::Accumulator;

struct AppState {
    config: Configuration,
    tx: Sender<PointCloudMessage>,
    accumulator: Arc<Mutex<Accumulator>>,
}

#[tokio::main]
async fn main() {
    File::create("out.json").unwrap();

    let (tx, rx) = mpsc::channel::<PointCloudMessage>(100);

    // Get the initial configuration
    let file = File::open("./server/config.json").unwrap();
    let reader = BufReader::new(file);
    let config: Configuration = serde_json::from_reader(reader).unwrap();

    dbg!(&config);

    let accumulator = Arc::new(Mutex::new(Accumulator::new(1000)));
    let accumulator_clone = accumulator.clone();

    let app_state = Arc::new(AppState {
        config,
        tx,
        accumulator,
    });

    // Spawn the main loop task
    tokio::spawn(async move {
        accumulator_handler(accumulator_clone, rx, "out.json".to_string()).await
    });

    let app = Router::new()
        .route("/ws", get(websocket_handler))
        .route("/get_pointcloud", get(get_pointcloud_handler))
        .route("/get_config", get(get_config_handler))
        .with_state(app_state);

    // let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn accumulator_handler(
    accumulator: Arc<Mutex<Accumulator>>,
    mut rx: Receiver<PointCloudMessage>,
    file_path: String,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));

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
            _ = interval.tick() => {
                let popped_data = accumulator.lock().await.pop_finished();
                if !popped_data.is_empty() {
                    let mut file = std::fs::OpenOptions::new().read(true).open(&file_path).unwrap_or_else(|_| File::create(&file_path).unwrap());
                    let mut contents = String::new();
                    file.read_to_string(&mut contents).expect("Unable to read file");
                    let mut existing_data: Vec<PointCloud> = serde_json::from_str(&contents).unwrap_or_else(|_| Vec::new());

                    // Append new data
                    let popped_len = popped_data.len();
                    existing_data.extend(popped_data);

                    // Reserialize and write back
                    let serialized_data = serde_json::to_string(&existing_data).expect("Unable to serialize data");
                    let mut file = File::create(&file_path).expect("Unable to create file");
                    file.write_all(serialized_data.as_bytes()).expect("Unable to write data");

                    println!("Updated out.json with {} items", popped_len);
                }
            }
        }
    }
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // let tx = state.tx.clone();
    println!("Incoming Websocket Connection: {:#?}", &ws);
    let state = state.clone();
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let config = state.config.clone();
    let tx = state.tx.clone();

    // Send the config
    if socket
        .send(Message::Text(
            serde_json::to_string(&ServerMessage::ConfigMessage(ConfigMessage {
                changed: (0..config.descriptors.len()).into_iter().collect(),
                config,
            }))
            .unwrap(),
        ))
        .await
        .is_err()
    {
        dbg!("Error with socket");
        return;
    }

    while let Some(Ok(Message::Text(message))) = socket.next().await {
        let Ok(message) = serde_json::from_str::<ServerMessage>(&message) else {
            continue;
        };
        match message {
            ServerMessage::ConfigMessage(_) => break,
            ServerMessage::PointCloudMessage(pointcloud_message) => {
                // Forward the message onto the pointcloud handler
                let result = tx.send(pointcloud_message).await;
                if result.is_err() {
                    eprintln!("Error sending pointcloud to accumulator: {:#?}", result);
                    dbg!(result.err().unwrap().to_string());
                    break;
                }
            }
        };
    }

    eprintln!("Socket Handler Stopped");
}

async fn get_pointcloud_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let accumulator = state.accumulator.lock().await;

    // Get the top pointcloud from the accumulator
    let pointcloud = accumulator.peek(); // Implement `peek` method or similar in your `Accumulator` struct

    // Serialize and return the pointcloud data
    match serde_json::to_string(&pointcloud) {
        Ok(json) => (StatusCode::OK, json),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to serialize pointcloud".into(),
        ),
    }
}

async fn get_config_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match serde_json::to_string(&state.config) {
        Ok(json) => (StatusCode::OK, json),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to serialize config".into(),
        ),
    }
}
