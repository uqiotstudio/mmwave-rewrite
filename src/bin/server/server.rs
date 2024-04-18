use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use mmwave::core::{
    config::Configuration,
    message::{Destination, Message},
};
use tokio::sync::{mpsc, Mutex};

#[derive(Clone)]
struct AppState {
    relay_tx: mpsc::Sender<MessageWrapper>,
}

#[derive(Clone)]
struct ServerState {
    config: Configuration,
    destinations: HashMap<Destination, Vec<mpsc::Sender<Message>>>,
}

#[derive(Clone)]
struct MessageWrapper {
    message: Message,
    sender_tx: Option<mpsc::Sender<Message>>,
}

#[tokio::main]
async fn main() {
    let (relay_tx, relay_rx) = mpsc::channel(100);
    let mut app_state = AppState { relay_tx };

    // Spawn the relay task
    let mut relay = tokio::task::spawn(async move { relay(relay_rx) });

    // Set up the axum router
    let app = Router::new()
        .route("/ws", get(websocket_handler))
        .with_state(Arc::new(app_state));

    // Listen on port 3000
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn relay(mut relay_rx: mpsc::Receiver<MessageWrapper>) {
    // establish the config held by this relay
    let config = Configuration::default();

    // establish the mapping of destinations to sockets
    let destinations = HashMap::<Destination, Vec<mpsc::Sender<Message>>>::new();

    let server_state = Arc::new(Mutex::new(ServerState {
        config,
        destinations,
    }));

    async fn handle_server_message(
        MessageWrapper { message, sender_tx }: MessageWrapper,
        server_state: Arc<Mutex<ServerState>>,
    ) {
        match message.content {
            mmwave::core::message::MessageContent::DataMessage(_) => todo!(),
            mmwave::core::message::MessageContent::ConfigMessage(_) => todo!(),
            mmwave::core::message::MessageContent::ConfigRequest(_) => todo!(),
            // Register a destination to the destination map
            mmwave::core::message::MessageContent::EstablishDestination(destination) => {
                if let Some(sender_tx) = sender_tx {
                    server_state
                        .lock()
                        .await
                        .destinations
                        .entry(destination)
                        .or_insert(Vec::new())
                        .push(sender_tx)
                }
            }
        };
    }

    // Receive messages from relay_rx and forward them to destinations
    // if the destination is server, handle the message here!
    while let Some(MessageWrapper { message, sender_tx }) = relay_rx.recv().await {
        match message.destination.clone() {
            // Rewrap the message and forward it to the relay handler
            Destination::Server => {
                tokio::task::spawn(handle_server_message(
                    MessageWrapper { message, sender_tx },
                    server_state.clone(),
                ));
            }
            // Otherwise, forward the message to all appropriate destinations
            other => {
                for tx in server_state
                    .lock()
                    .await
                    .destinations
                    .entry(other)
                    .or_insert(Vec::new())
                    .iter()
                {
                    if let Err(e) = tx.send(message.clone()).await {
                        eprintln!("Unable to send message to socket handler, {:?}", e);
                    }
                }
            }
        };
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

/// Handles the provided websocket
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    // lets us forward messages to the relay
    let relay_tx = state.relay_tx.clone();

    // let us receive messages from the relay
    let (sender_tx, mut sender_rx) = mpsc::channel(100);

    // break the socket in half
    let (mut socket_tx, mut socket_rx) = socket.split();

    // listen for + deserialize messages on the socket and forward them to relay
    let mut t1 = tokio::task::spawn(async move {
        while let Some(Ok(axum::extract::ws::Message::Text(message))) = socket_rx.next().await {
            let message = match serde_json::from_str::<Message>(&message) {
                Ok(message) => message,
                Err(e) => {
                    eprintln!("Error parsing message, {:?}", e);
                    continue;
                }
            };

            let message_wrapped = MessageWrapper {
                message,
                sender_tx: Some(sender_tx.clone()),
            };

            // forward the message onto the relay
            relay_tx
                .send(message_wrapped)
                .await
                .expect("failed to forward message to relay");
        }
    });

    // Receive messages sent to any destinations this socket is registered to
    // and forward the messages onto the client
    let mut t2 = tokio::task::spawn(async move {
        while let Some(message) = sender_rx.recv().await {
            let message = serde_json::to_string(&message);
            match message {
                Ok(message) => {
                    socket_tx
                        .send(axum::extract::ws::Message::Text(message))
                        .await
                        .expect("Socket is closed or broken");
                }
                Err(e) => {
                    eprintln!("Failed to send message with error {:?}", e);
                }
            };
        }
    });

    tokio::select! {
        _ = (&mut t1) => t2.abort(),
        _ = (&mut t2) => t1.abort()
    };

    eprintln!("Socket Handler Stopped");
}
