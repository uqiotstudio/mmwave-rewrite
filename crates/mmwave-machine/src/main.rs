mod args;

use args::Args;
use async_nats::jetstream;
use clap::Parser;
use futures::StreamExt;
use mmwave_awr::{AwrDescriptor, Model};
use mmwave_core::{
    address::ServerAddress,
    config::Configuration,
    devices::{DeviceConfig, EmptyDeviceDescriptor},
    logging::enable_tracing,
    message::Id,
    nats::get_store,
};
use std::collections::HashMap;
use std::{error::Error, time::Duration};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, instrument, Instrument};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    if args.tracing {
        enable_tracing(args.debug, args.log_relay);
    }

    let config = Configuration {
        descriptors: vec![
            DeviceConfig {
                id: Id::Device(0, 0),
                device_descriptor: Box::new(EmptyDeviceDescriptor),
            },
            DeviceConfig {
                id: Id::Device(0, 1),
                device_descriptor: Box::new(AwrDescriptor {
                    serial: "00E23E8E".to_string(),
                    model: Model::AWR1843AOP,
                    config: "".to_string(),
                    transform: mmwave_core::transform::Transform {
                        translation: [0.0, 0.0, 0.0],
                        orientation: [0.0, 0.0],
                    },
                }),
            },
        ],
    };
    info!("{}", serde_json::to_string(&config)?);

    let mut address = ServerAddress::new(args.ip, args.port).await;

    loop {
        if let Err(e) = handle_nats(address.clone(), args.clone()).await {
            error!(error=%e, "nats exited with error")
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
        debug!(config=?config, "Updated config");
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

    let mut entries = store.watch("config").await?;

    info!("Watching for config updates");
    while let Some(config) = entries.next().await {
        match config {
            Err(e) => {
                error!(error=%e, "something went wrong watching for config updates");
                break;
            }
            Ok(entry) => {
                info!("Found updated config");
                debug!(entry=?entry, "Updated config");
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

    for deviceConfig in config.descriptors {
        if deviceConfig.id.to_machine() != args.machine_id {
            // we only care about configs for this machine
            continue;
        }

        // spawn tasks if they dont exist
        let handle = devices.entry(deviceConfig.clone()).or_insert_with(|| {
            info!("spawned new task");
            let future = deviceConfig.clone().init(address);
            tokio::task::spawn(future.instrument(tracing::Span::current()))
        });
    }
}
