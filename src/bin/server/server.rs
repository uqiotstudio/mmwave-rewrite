use axum::{
    extract::{ws::WebSocket, ConnectInfo, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use chrono::Utc;
use clap::Parser;
use futures_util::{stream::SplitStream, SinkExt, StreamExt};
use indicatif::ProgressStyle;
use mmwave::core::{
    config::Configuration,
    message::{Destination, Message},
};
use mmwave::core::{message::Id, relay::Relay};
use searchlight::broadcast::{BroadcasterBuilder, ServiceBuilder};
use std::{
    collections::HashSet,
    fmt::Display,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    panic,
    sync::Arc,
};
use tokio::sync::{broadcast, Mutex};
use tracing::{error, info, instrument, span, warn, Instrument};
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Port for server
    #[arg(short, long, default_value_t = 3000)]
    pub port: u16,

    #[arg(short, long, default_value_t = false)]
    pub debug: bool,
}

#[derive(Clone)]
struct AppState {
    relay: Arc<Mutex<Relay<TraceableMessage>>>,
}

#[derive(Debug, Clone)]
struct TraceableMessage {
    message: Message,
    tx: broadcast::Sender<Id>,
}

impl Display for TraceableMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

fn set_panic_hook() {
    panic::set_hook(Box::new(|panic_info| {
        error!("Panic occurred: {:?}", panic_info);
    }));
}

#[tokio::main]
async fn main() {
    let Args { port, debug } = Args::parse();

    setup_logging(debug);
    set_panic_hook();

    let relay = Arc::new(Mutex::new(Relay::<TraceableMessage>::new()));
    let app_state = AppState {
        relay: relay.clone(),
    };

    tokio::spawn(register_server(relay.clone()));

    tokio::spawn(broadcast_mdns_service(port));

    let app = Router::new()
        .route("/ws", get(websocket_handler))
        .with_state(app_state);

    let addr = &SocketAddr::new(IpAddr::from(Ipv6Addr::UNSPECIFIED), port);
    info!(%addr, "Listening");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

fn setup_logging(debug: bool) {
    let indicatif_layer =
        IndicatifLayer::new().with_max_progress_bars(100, Some(ProgressStyle::default_bar()));
    let mut filter = EnvFilter::builder()
        .with_default_directive(tracing::Level::INFO.into())
        .from_env()
        .expect("Failed to parse environment filter");

    if debug {
        filter = filter.add_directive("mmwave=debug".parse().expect("Failed to parse directive"));
    }

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(indicatif_layer.get_stderr_writer()))
        .with(indicatif_layer)
        .with(filter)
        .init();
}

/// Registers the server and listens for incoming messages to process them.
///
/// Handles different types of messages such as ConfigRequest and forwards them accordingly.
///
/// # Arguments
///
/// * `relay` - An `Arc<Mutex<Relay<Message>>>` containing the relay for message forwarding.
#[instrument(skip(relay))]
async fn register_server(relay: Arc<Mutex<Relay<TraceableMessage>>>) {
    let mut server_rx = {
        let mut relay = relay.lock().await;
        relay.register(Id::Machine(0), Destination::Server);
        relay.subscribe(Id::Machine(0))
    };

    tokio::spawn(async move {
        let span = tracing::info_span!("registration service");
        let _guard = span.enter();

        let (traceback_tx, traceback_rx) = broadcast::channel(100);

        while let Ok(TraceableMessage { message, tx }) = server_rx.recv().await {
            match message.content {
                mmwave::core::message::MessageContent::DataMessage(_) => todo!(),
                mmwave::core::message::MessageContent::ConfigMessage(_) => todo!(),
                mmwave::core::message::MessageContent::ConfigRequest(ref destination) => {
                    let config_message = Message {
                        content: mmwave::core::message::MessageContent::ConfigMessage(
                            Configuration::default(),
                        ),
                        destination: HashSet::from([destination.clone()]),
                        timestamp: Utc::now(),
                    };
                    relay.lock().await.forward(
                        config_message.destination.clone(),
                        TraceableMessage {
                            message: config_message,
                            tx: traceback_tx.clone(),
                        },
                    );
                }
                mmwave::core::message::MessageContent::RegisterId(ids, destinations) => {
                    for id in ids {
                        let mut relay = relay.lock().await;
                        for destination in destinations.clone() {
                            relay.register(id, destination);
                        }

                        if let Err(e) = tx.send(id) {
                            error!(error=?e, "Unable to send traceback.");
                        }
                    }
                }
                mmwave::core::message::MessageContent::DeregisterId(_, _) => todo!(),
            }
        }
    });
}

/// Broadcasts the mDNS service for service discovery on the local network.
///
/// # Arguments
///
/// * `port` - The port on which the service is running.
#[instrument(name = "broadcasting mdns service")]
async fn broadcast_mdns_service(port: u16) {
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
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    info!(%addr, "Client connecting");
    let state = state.clone();
    ws.on_upgrade(move |socket| handle_socket(socket, addr, state))
}

/// Handles an upgraded WebSocket connection, forwarding messages between the WebSocket and the relay.
///
/// # Arguments
///
/// * `socket` - The upgraded WebSocket connection.
/// * `addr` - The client's socket address.
/// * `state` - The shared application state.
#[instrument(skip_all, fields(addr=%addr))]
async fn handle_socket(socket: WebSocket, addr: SocketAddr, state: AppState) {
    let (mut socket_tx, mut socket_rx) = socket.split();
    let relay = state.relay.clone();
    let (traceback_tx, mut traceback_rx) = broadcast::channel(100);
    let (outbound_tx, mut outbound_rx) = broadcast::channel(100);

    tokio::task::spawn(async move {
        while let Ok(out) = outbound_rx.recv().await {
            socket_tx.send(out).await;
        }
    });

    // Forward WebSocket messages to relay
    let relay_tx_span = tracing::info_span!("inbound");
    let mut relay_tx_task = tokio::spawn(
        forward_websocket_to_relay(relay.clone(), socket_rx, traceback_tx)
            .instrument(relay_tx_span),
    );

    // Forward relay messages to WebSocket
    let relay_rx_span = tracing::info_span!("outbound");
    let tasks = Arc::new(Mutex::new(Vec::new()));
    let mut relay_rx_task = tokio::spawn(
        forward_relay_to_websocket(relay.clone(), traceback_rx, outbound_tx, tasks.clone())
            .instrument(relay_rx_span),
    );

    tokio::select! {
        _ = (&mut relay_tx_task) => {
            relay_rx_task.abort();
        },
        _ = (&mut relay_rx_task) => {
            relay_tx_task.abort();
        }
    }

    tasks.lock().await.iter().for_each(|t| t.abort());
    info!(%addr, "Connection closed");
}

async fn forward_websocket_to_relay(
    relay: Arc<Mutex<Relay<TraceableMessage>>>,
    mut socket_rx: SplitStream<WebSocket>,
    traceback_tx: broadcast::Sender<Id>,
) {
    while let Some(Ok(axum::extract::ws::Message::Text(message))) = socket_rx.next().await {
        let message = match serde_json::from_str::<Message>(&message) {
            Ok(msg) => msg,
            Err(e) => {
                error!(error = %e, "Error parsing message");
                continue;
            }
        };

        relay.lock().await.forward(
            message.destination.clone(),
            TraceableMessage {
                message,
                tx: traceback_tx.clone(),
            },
        );
    }
}

async fn forward_relay_to_websocket(
    relay: Arc<Mutex<Relay<TraceableMessage>>>,
    mut traceback_rx: broadcast::Receiver<Id>,
    outbound_tx: broadcast::Sender<axum::extract::ws::Message>,
    tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
) {
    while let Ok(id) = traceback_rx.recv().await {
        let id_span = tracing::info_span!("subscription", subscriber=%id);
        let task = tokio::task::spawn(
            subscribe_to_relay(relay.clone(), id, outbound_tx.clone()).instrument(id_span),
        );

        tasks.lock().await.push(task);
    }
}

async fn subscribe_to_relay(
    relay: Arc<Mutex<Relay<TraceableMessage>>>,
    id: Id,
    outbound_tx: broadcast::Sender<axum::extract::ws::Message>,
) {
    let mut rx = relay.lock().await.subscribe(id.clone());

    while let Ok(TraceableMessage { message, tx: _ }) = rx.recv().await {
        let message = match serde_json::to_string(&message) {
            Ok(msg) => msg,
            Err(e) => {
                error!(error = %e, "Error serializing message");
                continue;
            }
        };

        outbound_tx
            .send(axum::extract::ws::Message::Text(message))
            .expect("Unable to send message to outbound_tx");
    }
}
