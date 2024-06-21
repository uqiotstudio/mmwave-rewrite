mod address;
mod args;

use crate::{address::ServerAddress, args::Args};
use clap::Parser;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use indicatif::ProgressStyle;
use mmwave::{
    core::{
        config::Configuration,
        message::{Destination, Id, Message, MessageContent},
        relay::Relay,
    },
    devices::DeviceConfig,
};
use serde_json::{self};
use std::{
    collections::{HashMap, HashSet},
    panic,
    sync::Arc,
    time::Duration,
};
use tokio::{net::TcpStream, select};
use tokio::{
    sync::{broadcast, Mutex},
    task::JoinHandle,
};
use tokio_tungstenite::{connect_async, MaybeTlsStream};
use tracing::{debug, error, info, warn, Instrument};
use tracing::{info_span, instrument};
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[derive(Clone)]
struct AppState {
    server_address: ServerAddress,
    id: Id,
    relay: Arc<Mutex<Relay<Message>>>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if args.tracing {
        setup_logging(args.debug, args.log_relay);
    }
    set_panic_hook();

    let server_address = ServerAddress::new(args.clone()).await;
    let id = args.machine_id;

    info!(ip = ?server_address.address(), url = %server_address.url(), "server_address");

    let app_state = AppState {
        server_address,
        id,
        relay: Arc::new(Mutex::new(Relay::new())),
    };

    let (outbound_tx, outbound_rx) = broadcast::channel::<Message>(100);

    let mut t1 = tokio::task::spawn(manage_devices(app_state.clone(), outbound_tx));
    let mut t2 = tokio::task::spawn(manage_connection(app_state.clone(), outbound_rx));

    select! {
        _ = &mut t1 => { t2.abort(); }
        _ = &mut t2 => { t1.abort(); }
    };
    error!("task aborted, this is unrecoverable");
}


fn set_panic_hook() {
    panic::set_hook(Box::new(|panic_info| {
        error!("Panic occurred: {:?}", panic_info);
    }));
}

#[instrument(skip_all, fields(address=%app_state.server_address.address(), machine=?app_state.id))]
async fn manage_connection(app_state: AppState, outbound_rx: broadcast::Receiver<Message>) {
    let AppState {
        mut server_address,
        id,
        relay,
    } = app_state;
    let outbound_rx = Arc::new(Mutex::new(outbound_rx));
    let mut interval = tokio::time::interval(Duration::from_millis(1000));

    loop {
        interval.tick().await;
        server_address.refresh().await;

        let (mut ws_tx, mut ws_rx) = match connect_async(server_address.url_ws()).await {
            Ok((stream, _)) => stream,
            Err(e) => {
                warn!("Unable to connect to server");
                debug!("Unable to connect to server with error {:?}", e);
                continue;
            }
        }
        .split();
        info!(addr=%server_address.address(), "Connected to server");

        if let Err(e) = ws_tx
            .send(tokio_tungstenite::tungstenite::Message::Text(
                serde_json::to_string(&Message {
                    content: MessageContent::RegisterId(
                        HashSet::from([id]),
                        HashSet::from([Destination::Id(id), Destination::Manager]),
                    ),
                    destination: HashSet::from([Destination::Server]),
                    timestamp: chrono::Utc::now(),
                })
                .expect("this should serialize fine"),
            ))
            .await
        {
            error!("Failed to send register message: {:?}", e);
            continue;
        }

        let mut outbound_task = tokio::task::spawn(
            outbound_loop(outbound_rx.clone(), ws_tx).instrument(tracing::Span::current()),
        );
        let mut inbound_task = tokio::task::spawn(
            inbound_loop(ws_rx, relay.clone()).instrument(tracing::Span::current()),
        );

        select! {
            _ = (&mut outbound_task) => { inbound_task.abort(); }
            _ = (&mut inbound_task) => { outbound_task.abort(); }
        }
        warn!("websocket connection was closed, is this intentional?");
    }
}

#[instrument(skip_all)]
async fn outbound_loop(
    outbound_rx: Arc<Mutex<broadcast::Receiver<Message>>>,
    mut ws_tx: SplitSink<
        tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>,
        tokio_tungstenite::tungstenite::Message,
    >,
) {
    info!("outbound loop started");
    while let Ok(message) = outbound_rx.lock().await.recv().await {
        if let Ok(message) = serde_json::to_string(&message) {
            if let Err(e) = ws_tx
                .send(tokio_tungstenite::tungstenite::Message::Text(message))
                .await
            {
                error!("Unable to send message on websocket: {:?}", e);
                break;
            }
        } else {
            error!("Unable to encode message: {:?}", message);
            break;
        }
    }
    error!("outbound_rx closed");
}

#[instrument(skip_all)]
async fn inbound_loop(
    mut ws_rx: SplitStream<tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>>,
    relay: Arc<Mutex<Relay<Message>>>,
) {
    info!("inbound loop started");
    while let Some(message) = ws_rx.next().await {
        match message {
            Ok(message) => {
                if let Ok(message) = serde_json::from_str::<Message>(&message.to_string()) {
                    relay
                        .lock()
                        .await
                        .forward(message.destination.clone(), message);
                } else {
                    error!("Unable to parse message: {:?}", message);
                    break;
                }
            }
            Err(e) => {
                error!("Unable to receive from websocket: {:?}", e);
                break;
            }
        }
    }
}

#[instrument(skip_all, fields(machine=?app_state.id))]
async fn manage_devices(app_state: AppState, outbound_tx: broadcast::Sender<Message>) {
    let AppState {
        server_address: _,
        id,
        relay,
    } = app_state;
    let mut tasks: HashMap<Id, JoinHandle<()>> = HashMap::new();
    let mut rx = relay.lock().await.subscribe(id);

    relay.lock().await.register(id, Destination::Id(id));
    relay.lock().await.register(id, Destination::Manager);

    let interval = tokio::time::interval(Duration::from_secs(10));
    tokio::task::spawn(request_config_loop(outbound_tx.clone(), id, interval));

    while let Ok(message) = rx.recv().await {
        info!("Received message");
        debug!(message=?message, "Received message");
        match message.content {
            MessageContent::ConfigMessage(config) => {
                handle_config_message(config, id, relay.clone(), &mut tasks, &outbound_tx).await
            }
            MessageContent::RegisterId(ids, destinations) => {
                handle_registration_message(ids, destinations, relay.clone()).await
            }
            MessageContent::Reboot => todo!(),
            message => error!(message = %message, "Received unsupported message"),
        }
    }
    error!("relay subscription closed unexpectedly");
}

#[instrument(skip_all)]
async fn handle_registration_message(
    ids: HashSet<Id>,
    destinations: HashSet<Destination>,
    relay: Arc<Mutex<Relay<Message>>>,
) {
    for id in ids {
        let mut relay = relay.lock().await;
        for destination in destinations.clone() {
            relay.register(id, destination);
        }
    }
}

#[instrument(skip_all)]
async fn request_config_loop(
    outbound_tx: broadcast::Sender<Message>,
    id: Id,
    mut interval: tokio::time::Interval,
) {
    interval.reset();
    loop {
        info!("requesting config refresh from server");
        if let Err(e) = outbound_tx.send(Message {
            content: MessageContent::ConfigRequest(Destination::Id(id)),
            destination: HashSet::from([Destination::Server]),
            timestamp: chrono::Utc::now(),
        }) {
            error!("failed to send config message: {:?}", e);
            panic!("failed to send config message");
        }
        interval.tick().await;
    }
}

#[instrument(skip_all)]
async fn handle_config_message(
    config: Configuration,
    id: Id,
    relay: Arc<Mutex<Relay<Message>>>,
    tasks: &mut HashMap<Id, JoinHandle<()>>,
    outbound_tx: &broadcast::Sender<Message>,
) {
    let mut keep = HashSet::new();
    for desc in config.clone().descriptors {
        let id = match desc.id {
            Id::Device(m, d) if Id::Machine(m) == id => Id::Device(m, d),
            _ => continue,
        };
        keep.insert(id);
        if !tasks.contains_key(&id) {
            tasks.insert(
                id,
                tokio::task::spawn(handle_device(desc, id, relay.clone(), outbound_tx.clone())),
            );
        } else {
            // send an update config through to said device
            relay.lock().await.forward(
                HashSet::from([Destination::Id(id)]),
                Message {
                    content: MessageContent::ConfigMessage(config.clone()),
                    destination: HashSet::from([Destination::Id(id)]),
                    timestamp: chrono::Utc::now(),
                },
            )
        }
    }

    for id in tasks.keys().cloned().collect::<Vec<_>>() {
        if !keep.contains(&id) {
            tasks.remove(&id).map(|popped| popped.abort());
            info!(id=?id, "removed device");
        }
    }
}

#[instrument(skip_all)]
async fn handle_device(
    desc: DeviceConfig,
    id: Id,
    relay: Arc<Mutex<Relay<Message>>>,
    outbound_tx: broadcast::Sender<Message>,
) {
    let mut dev = desc.init();
    let (dev_tx, dev_rx) = dev.channel();

    // We want to register the ID of this device to listen for its own ID on both the server, and on this device manager.
    let _ = outbound_tx.send(Message {
        content: MessageContent::RegisterId(
            HashSet::from([id]),
            HashSet::from([Destination::Id(id)]),
        ),
        destination: HashSet::from([Destination::Server, Destination::Id(id.to_machine())]),
        timestamp: chrono::Utc::now(),
    });

    let rx = relay.lock().await.subscribe(id);
    let mut dev = dev.start();

    let mut t1 = tokio::task::spawn(
        forward_messages_to_device(rx, dev_tx).instrument(tracing::Span::current()),
    );
    let mut t2 = tokio::task::spawn(
        forward_messages_to_ws(dev_rx, outbound_tx.clone()).instrument(tracing::Span::current()),
    );

    select! {
        _ = &mut t1 => { t2.abort(); dev.abort(); }
        _ = &mut t2 => { t1.abort(); dev.abort(); }
        _ = &mut dev => { t1.abort(); t2.abort(); }
    }
}

#[instrument(skip_all)]
async fn forward_messages_to_device(
    mut rx: broadcast::Receiver<Message>,
    dev_tx: broadcast::Sender<Message>,
) {
    info!("started forwarding from relay to device");
    while let Ok(message) = rx.recv().await {
        dev_tx
            .send(message)
            .expect("unable to send message to dev_tx, was it closed?");
    }
    error!("forwarding from relay to device failed");
}

#[instrument(skip_all)]
async fn forward_messages_to_ws(
    mut dev_rx: broadcast::Receiver<Message>,
    outbound_tx: broadcast::Sender<Message>,
) {
    info!("started forwarding from device to ws");
    while let Ok(message) = dev_rx.recv().await {
        outbound_tx.send(message).unwrap();
    }
    error!("forwarding from device to ws failed");
}
