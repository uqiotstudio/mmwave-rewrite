use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use futures_util::{SinkExt, StreamExt};
use mmwave::core::{accumulator::Accumulator, config::Configuration, pointcloud::PointCloud};
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Write},
    net::SocketAddr,
    sync::Arc,
};
use tokio::sync::{
    mpsc::{self, Receiver as MpscReceiver, Sender as MpscSender},
    watch::{self, Receiver as WatchReceiver, Sender as WatchSender},
    Mutex,
};

struct AppState {
    config: Arc<Mutex<Configuration>>,
    tx: MpscSender<PointCloud>,
    rx: WatchReceiver<PointCloud>,
}

#[tokio::main]
async fn main() {
    // Config is set up properly by a user, in the dashboard
    let config = Arc::new(Mutex::new(Configuration::default()));

    let (mpsc_tx, mpsc_rx) = mpsc::channel::<PointCloud>(100);
    let (watch_tx, watch_rx) = watch::channel::<PointCloud>(PointCloud::default());

    // Initialize the app state
    let app_state = Arc::new(AppState {
        config,
        tx: mpsc_tx,
        rx: watch_rx,
    });

    // Spawn a task to start handling the accumulator
    let accumulator = Accumulator::new(1000);
    tokio::task::spawn(handle_accumulator(accumulator, mpsc_rx, watch_tx));

    // Set up the axum router
    let app = Router::new()
        .route("/ws", get(websocket_handler))
        .route("/get_config", get(get_config_handler))
        .route("/set_config", post(set_config_handler))
        .with_state(app_state);

    // Listen on port 3000
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn handle_accumulator(
    mut accumulator: Accumulator,
    mut mpsc_rx: MpscReceiver<PointCloud>,
    watch_tx: WatchSender<PointCloud>,
) {
    loop {
        tokio::select! {
            recieved = mpsc_rx.recv() => {
                if let Some(point_cloud) = recieved {
                    accumulator.push(point_cloud);
                    accumulator.reorganize();
                } else {
                    eprintln!("Accumulator Stopped");
                    return;
                }
            },
            Some(pointcloud) = accumulator.peek() => {
                watch_tx.send(pointcloud);
            }
        }
    }
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    println!("Incoming Websocket Connection: {:#?}", &ws);
    let state = state.clone();
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let tx = state.tx.clone();
    let mut rx = state.rx.clone();
    let (mut socket_tx, mut socket_rx) = socket.split();

    // We are effectively just creating two processes which connect the channels up with those used in the accumulator handler.

    // Recieve on the socket and forward that to the accumulator
    let mut t1 = tokio::spawn(async move {
        while let Some(Ok(Message::Text(message))) = socket_rx.next().await {
            let Ok(message) = serde_json::from_str::<PointCloud>(&message) else {
                continue;
            };
            // Forward the message onto the pointcloud handler
            let result = tx.send(message).await;
            if result.is_err() {
                eprintln!("Error sending pointcloud to accumulator: {:#?}", result);
                dbg!(result.err().unwrap().to_string());
                break;
            }
        }
    });

    // Receive the top frame from the accumulator each 100ms and forward it
    let mut t2 = tokio::spawn(async move {
        loop {
            let pointcloud = rx.borrow_and_update().clone();
            if let Err(e) = socket_tx
                .send(Message::Text(
                    serde_json::to_string(&pointcloud).unwrap_or("".to_owned()),
                ))
                .await
            {
                eprintln!("Error receiving pointcloud from accumulator: {:#?}", e);
                break;
            }
            if let Err(e) = rx.changed().await {
                eprintln!("Error receiving pointcloud from accumulator: {:#?}", e);
                break;
            }
        }
    });

    tokio::select! {
        _ = (&mut t1) => t2.abort(),
        _ = (&mut t2) => t1.abort()
    };

    eprintln!("Socket Handler Stopped");
}

async fn get_config_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match serde_json::to_string(&state.config.lock().await.clone()) {
        Ok(json) => (StatusCode::OK, json),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to serialize config".into(),
        ),
    }
}

async fn set_config_handler(State(state): State<Arc<AppState>>, message: String) {
    let Ok(config) = serde_json::from_str::<Configuration>(&message) else {
        return;
    };

    *state.config.lock().await = config;
}
