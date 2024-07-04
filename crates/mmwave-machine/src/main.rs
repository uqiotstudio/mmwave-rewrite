mod args;

use args::Args;
use async_ctrlc::CtrlC;
use async_nats::jetstream;
use clap::Parser;
use futures::{future, task::noop_waker, Future, FutureExt, StreamExt};
use mmwave_awr::{AwrDescriptor, Model};
use mmwave_zed::{ZedDescriptor};
use mmwave_core::{
    address::ServerAddress,
    config::Configuration,
    devices::{DeviceConfig, EmptyDeviceDescriptor},
    logging::enable_tracing,
    message::Id,
    nats::get_store,
};
use mmwave_recorder::RecordingDescriptor;
use std::{
    collections::{HashMap, HashSet},
    task::Context,
};
use std::{error::Error, time::Duration};
use tokio::{signal, sync::watch, task::JoinHandle};
use tracing::{debug, error, info, instrument, Instrument};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    if args.tracing {
        enable_tracing(args.debug);
    }

    let mut address = ServerAddress::new(args.ip, args.port).await;

    loop {
        if let Err(e) = handle_nats(address.clone(), args.clone()).await {
            error!(error=%e, "nats exited with error")
        } else {
            // no error, we can safely quit
            return Ok(());
        }
        info!("Attempting to re-establish connection");
        tokio::time::sleep(Duration::from_millis(1000)).await;
        address.refresh().await;
    }
}

#[instrument(skip_all)]
async fn handle_nats(address: ServerAddress, args: Args) -> Result<(), Box<dyn Error>> {
    // Connect to the NATS server
    let client = async_nats::connect(address.address().to_string()).await?;
    let jetstream = jetstream::new(client);

    let store = get_store(jetstream).await?;

    // Create a hashset of devices
    let mut devices: HashMap<DeviceConfig, JoinHandle<()>> = HashMap::new();

    if let Some(config) = store.get("config").await? {
        info!("Found initial config");
        debug!(config=?config, "Initial config");
        match serde_json::from_slice(&config) {
            Ok(config) => {
                update_devices(&mut devices, config, address.clone(), args.clone());
            }
            Err(e) => {
                error!(error=?e, "Failed to parse config");
                debug!(config=?config, "The incorrect config");
            }
        };
    }

    let (shutdown_tx, mut shutdown_rx) = watch::channel(());
    let mut entries = store.watch("config").await?;

    let mut config_task = tokio::spawn(async move {
        info!("Watching for config updates");
        while let Some(config) = entries.next().await {
            match config {
                Err(e) => {
                    error!(error=%e, "something went wrong watching for config updates");
                    break;
                }
                Ok(entry) => {
                    info!("New config inbound");
                    debug!(entry=?entry, "Inbound config");
                    let config = match serde_json::from_slice(&entry.value) {
                        Ok(config) => config,
                        Err(e) => {
                            error!(error=?e, "Failed to parse config");
                            debug!(entry=?entry, "The incorrect entry");
                            continue;
                        }
                    };
                    update_devices(&mut devices, config, address.clone(), args.clone());
                }
            }
        }
    });

    let shutdown_task = tokio::spawn(async move {
        if signal::ctrl_c().await.is_ok() {
            info!("Received Ctrl-C, initiating shutdown");
            let _ = shutdown_tx.send(());
        }
    });

    tokio::select! {
        _ = &mut config_task => {},
        _ = shutdown_rx.changed() => {
            info!("Shutdown signal received, cleaning up");
            config_task.abort();
        },
    }

    info!("Shutting down gracefully");

    shutdown_task.await?;

    Ok(())
}

fn update_devices(
    devices: &mut HashMap<DeviceConfig, JoinHandle<()>>,
    config: Configuration,
    address: ServerAddress,
    args: Args,
) {
    // remove finished tasks
    devices.retain(|_, j| !j.is_finished());

    let mut removals: HashSet<_> = devices.keys().cloned().collect();

    for device_config in config.descriptors {
        if device_config.id.to_machine() != args.machine_id {
            // we only care about configs for this machine
            continue;
        }

        // spawn tasks if they dont exist
        removals.remove(&device_config);
        let handle = devices.entry(device_config.clone()).or_insert_with(|| {
            info!("spawned new task");
            let future = device_config.clone().init(address);
            tokio::task::spawn(future.instrument(tracing::Span::current()))
        });
    }

    for removal in removals {
        let Some(dev) = devices.remove(&removal) else {
            continue;
        };
        dev.abort();
    }
}
