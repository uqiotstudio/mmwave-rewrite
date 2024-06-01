mod args;

use args::Args;
use axum::{
    extract::{ws::WebSocket, ConnectInfo, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use chrono::Utc;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use mmwave::core::{
    config::Configuration,
    message::{Destination, Message},
};
use mmwave::core::{message::Id, relay::Relay};
use searchlight::broadcast::{BroadcasterBuilder, ServiceBuilder};
use std::{collections::HashMap, sync::Arc};
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};
use tokio::{
    stream,
    sync::{
        broadcast,
        mpsc::{self},
        Mutex,
    },
    task::JoinHandle,
};
use tracing::{error, info, info_span, instrument, level_filters::LevelFilter, warn};
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Clone)]
struct AppState {
    relay_tx: mpsc::Sender<MessageWrapper>,
}

struct RelayState {
    config: Configuration,
    relay_joins: HashMap<Id, JoinHandle<()>>,
    relay: Relay<MessageWrapper>,
}

impl Default for RelayState {
    fn default() -> Self {
        RelayState {
            config: Configuration::default(),
            relay_joins: HashMap::new(),
            relay: Relay::<MessageWrapper>::new(),
        }
    }
}

#[derive(Clone, Debug)]
struct MessageWrapper {
    message: Message,
    sender_tx: Option<mpsc::Sender<Message>>,
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

    let Args { port } = Args::parse();

    let (relay_tx, relay_rx) = mpsc::channel(100);
    let mut app_state = AppState { relay_tx };

    // Spawn the relay task
    let mut relay = tokio::task::spawn(relay(relay_rx));

    // Broadcast a mdns service
    tokio::task::spawn(async move {
        let broadcaster = BroadcasterBuilder::new()
            .loopback()
            .add_service(
                ServiceBuilder::new("_http._tcp.local", "mmwaveserver", port)
                    .unwrap()
                    .add_ip_address(IpAddr::V4(Ipv4Addr::LOCALHOST))
                    .add_ip_address(IpAddr::V6(Ipv6Addr::LOCALHOST))
                    .build()
                    .unwrap(),
            )
            .build(searchlight::net::IpVersion::Both)
            .unwrap()
            .run_in_background();

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1));
        }
    });

    // Set up the axum router
    let app = Router::new()
        .route("/ws", get(websocket_handler))
        .with_state(Arc::new(app_state));

    let addr = &SocketAddr::new(IpAddr::from(Ipv6Addr::UNSPECIFIED), port);
    info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

#[instrument(skip_all)]
async fn relay(mut relay_rx: mpsc::Receiver<MessageWrapper>) {
    let relay_state_orig = Arc::new(Mutex::new(RelayState::default()));

    // Set up the realy, with a server_rx for server messages
    let relay_state = relay_state_orig.clone();
    relay_state
        .lock()
        .await
        .relay
        .register(Id::Machine(0), Destination::Server);
    let mut server_rx = relay_state
        .lock()
        .await
        .relay
        .subscribe(Id::Machine(0))
        .expect("Failed to subscribe server to relay");

    #[instrument(skip_all, fields(id=?id))]
    async fn join(id: Id, mut rx: broadcast::Receiver<MessageWrapper>, tx: mpsc::Sender<Message>) {
        loop {
            tokio::select! {
                Ok(MessageWrapper {
                  message,
                  sender_tx: _,
                }) = rx.recv() => {
                    let _ = tx.send(message).await;
                }
                _ = tx.closed() => {
                    warn!("Join Closed");
                    break;
                }
            }
        }
    }

    // Receive server messages, handle them
    tokio::task::spawn(async move {
        while let Ok(MessageWrapper { message, sender_tx }) = server_rx.recv().await {
            match message.content {
                mmwave::core::message::MessageContent::DataMessage(_) => todo!(),
                mmwave::core::message::MessageContent::ConfigMessage(_) => todo!(),
                mmwave::core::message::MessageContent::ConfigRequest(destination) => {
                    info!("Received ConfigRequest message");
                    // Create and send a config message to the destination specified:
                    let message = Message {
                        content: mmwave::core::message::MessageContent::ConfigMessage(
                            relay_state.lock().await.config.clone(),
                        ),
                        destination: HashSet::from([destination.clone()]),
                        timestamp: Utc::now(),
                    };
                    relay_state.lock().await.relay.forward(
                        message.destination.clone(),
                        MessageWrapper {
                            message,
                            sender_tx: None,
                        },
                    );
                }
                mmwave::core::message::MessageContent::RegisterId(id, destinations) => {
                    let mut relay_state = relay_state.lock().await;
                    info!(
                        "Connecting {:?} to {} destinations {:?}",
                        id,
                        destinations.len(),
                        destinations
                    );
                    for destination in destinations {
                        relay_state.relay.register(id, destination);
                    }
                    if let Some(join_handle) = relay_state.relay_joins.get(&id) {
                        if !join_handle.is_finished() {
                            continue;
                        }
                    }

                    let Some(sender_tx) = sender_tx else {
                        error!("Unable to unwrap sender_tx");
                        continue;
                    };
                    let Some(mut rx) = relay_state.relay.subscribe(id) else {
                        error!("Unable to subscribe to id {:?}", id);
                        continue;
                    };

                    // this is a new task, OR the previous one ended
                    relay_state
                        .relay_joins
                        .insert(id, tokio::task::spawn(join(id, rx, sender_tx)));
                }
                mmwave::core::message::MessageContent::DeregisterId(_, _) => todo!(),
            }
        }
    });

    // Receive messages from relay_rx and forward them to destinations
    let relay_state = relay_state_orig.clone();
    while let Some(MessageWrapper { message, sender_tx }) = relay_rx.recv().await {
        info!(message=?message, "Message received by relay");
        relay_state.lock().await.relay.forward(
            message.destination.clone(),
            MessageWrapper { message, sender_tx },
        );
        info!("Message forwarded along relay");
    }
}

#[instrument(skip_all)]
async fn websocket_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    info!(addr = %addr, "Client Connecting");
    let state = state.clone();
    ws.on_upgrade(move |socket| handle_socket(socket, addr.clone(), state))
}

/// Handles the provided websocket
#[instrument(skip_all, fields(addr=%addr))]
async fn handle_socket(socket: WebSocket, addr: SocketAddr, state: Arc<AppState>) {
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
                    error!("Error parsing message, {:?}", e);
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
                    error!("Failed to send message with error {:?}", e);
                }
            };
        }
    });

    info!("Socket handler starter");
    tokio::select! {
        _ = (&mut t1) => t2.abort(),
        _ = (&mut t2) => t1.abort()
    };

    warn!("Socket Handler Stopped");
}
