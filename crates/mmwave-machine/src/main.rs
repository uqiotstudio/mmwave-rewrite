mod args;

use args::Args;
use async_nats::{jetstream, rustls::pki_types::IpAddr};
use bincode::deserialize;
use clap::Parser;
use futures::StreamExt;
use mmwave_core::{
    address::ServerAddress,
    config::Configuration,
    logging::enable_tracing,
    message::{Id, Message, Tag},
};
use std::error::Error;
use tokio::task::JoinHandle;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    if args.tracing {
        enable_tracing(args.debug, args.log_relay);
    }

    let address = ServerAddress::new(args.ip, args.port).await;

    // Connect to the NATS server
    let client = async_nats::connect(address.address().to_string()).await?;
    let jetstream = jetstream::new(client);

    let configs = jetstream
        .create_key_value(jetstream::kv::Config {
            bucket: "config".to_string(),
            history: 10,
            ..Default::default()
        })
        .await?;

    configs.put("config", Configuration);

    let mut entries = configs.watch("config").await?;
    while let Some(config) = entries.next().await {
        info!(config=?config, "Found updated config");
    }

    Ok(())
}
