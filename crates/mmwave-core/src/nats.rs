use std::error::Error;

use async_nats::jetstream::{self, kv::Store, Context};
use tracing::{info, instrument};

use crate::config::Configuration;

#[instrument(skip_all)]
pub async fn get_store(jetstream: Context) -> Result<Store, Box<dyn Error>> {
    Ok(match jetstream.get_key_value("config").await {
        Ok(store) => store,
        Err(_) => {
            info!("config bucket does not exist, creating a default config");
            let store = jetstream
                .create_key_value(jetstream::kv::Config {
                    bucket: "config".to_string(),
                    history: 1,
                    ..Default::default()
                })
                .await?;

            let default_config = Configuration::default();
            let serialized = serde_json::to_string(&default_config)?;
            store.put("config", serialized.clone().into()).await?;

            info!(config = serialized, "put config into kv store");
            store
        }
    })
}
