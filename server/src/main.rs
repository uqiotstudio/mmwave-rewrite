pub mod buffer;
pub mod message;

use crate::buffer::FrameBuffer;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use message::{ConfigMessage, PointCloudMessage, ServerMessage};
use radars::config::Configuration;
use std::{fs::File, io::BufReader, net::SocketAddr, sync::Arc};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_stream::StreamExt;

struct AppState {
    config: Configuration,
    tx: Sender<PointCloudMessage>,
}

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::channel::<PointCloudMessage>(100);

    // Get the initial configuration
    let file = File::open("./server/config.json").unwrap();
    let reader = BufReader::new(file);
    let config: Configuration = serde_json::from_reader(reader).unwrap();

    dbg!(&config);

    // Spawn the main loop task
    tokio::spawn(async move { accumulator(rx) });

    let app_state = Arc::new(AppState { config, tx });

    let app = Router::new()
        .route("/ws", get(websocket_handler))
        .with_state(app_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn accumulator(mut rx: Receiver<PointCloudMessage>) {
    let mut frame_buffer = FrameBuffer::new(100);
    while let Some(point_cloud_message) = rx.recv().await {
        let mut point_cloud = point_cloud_message.pointcloud;
        frame_buffer.push_frame(&mut point_cloud);
        dbg!(&frame_buffer);
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
    let config2 = state.config.clone();
    let tx = state.tx.clone();

    let json = serde_json::to_string(&ServerMessage::ConfigMessage(ConfigMessage {
        changed: (0..config2.descriptors.len()).into_iter().collect(),
        config: config2,
    }))
    .unwrap();

    dbg!(&json);

    let deserialized: ServerMessage = serde_json::from_str(&json).unwrap();
    dbg!(deserialized);

    // Send the config
    if socket
        .send(Message::Binary(
            bincode::serialize(&ServerMessage::ConfigMessage(ConfigMessage {
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

    while let Some(Ok(Message::Binary(message))) = socket.next().await {
        let Ok(message) = bincode::deserialize::<ServerMessage>(&message) else {
            continue;
        };
        match message {
            ServerMessage::ConfigMessage(_) => break,
            ServerMessage::PointCloudMessage(pointcloud_message) => {
                // Forward the message onto the pointcloud handler
                if tx.send(pointcloud_message).await.is_err() {
                    break;
                }
            }
        };
    }
}
