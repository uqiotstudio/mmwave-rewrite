mod args;

use args::Args;
use async_nats::jetstream;
use clap::Parser;
use futures::StreamExt;
use mmwave_core::{address::ServerAddress, config::Configuration, logging::enable_tracing};
use std::{error::Error, time::Duration};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    if args.tracing {
        enable_tracing(args.debug, args.log_relay);
    }

    let mut address = ServerAddress::new(args.ip, args.port).await;

    loop {
        if let Err(e) = handle_nats(address.clone()).await {
            error!(error=%e, "nats exited with error")
        }
        info!("Attempting to re-establish connection");
        tokio::time::sleep(Duration::from_millis(1000)).await;
        address.refresh().await;
    }
}

async fn handle_nats(address: ServerAddress) -> Result<(), Box<dyn Error>> {
    // Connect to the NATS server
    let client = async_nats::connect(address.address().to_string()).await?;
    let jetstream = jetstream::new(client);

    let configs = match jetstream.get_key_value("config").await {
        Ok(configs) => configs,
        Err(_) => {
            info!("config bucket does not exist, creating a default config");
            jetstream
                .create_key_value(jetstream::kv::Config {
                    bucket: "config".to_string(),
                    history: 10,
                    ..Default::default()
                })
                .await?
        }
    };

    let default_config = Configuration::default();
    let serialized = serde_json::to_string(&default_config)?;
    configs.put("config", serialized.clone().into()).await?;

    info!(config = serialized, "put config into kv store");

    let mut entries = configs.watch("config").await?;
    info!("Watching for config updates");
    while let Some(config) = entries.next().await {
        match config {
            Err(e) => {
                error!(error=%e, "something went wrong watching for config updates");
                break;
            }
            Ok(config) => {
                info!(config=?config, "Found updated config");
            }
        }
    }

    Ok(())
}
