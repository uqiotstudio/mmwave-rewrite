use async_nats::rustls::pki_types::IpAddr;
use futures::StreamExt;
use mmwave_core::{address::ServerAddress, message::Id};
use std::error::Error;
use tokio::task::JoinHandle;

async fn start_awr(id: Id) -> Result<JoinHandle<()>, Box<dyn Error>> {
    // Connect to the NATS server
    let client = async_nats::connect("").await?;
    client.clone();

    // Subscribe to the "messages" subject
    let mut subscriber = client.subscribe("messages").await?;

    // Publish messages to the "messages" subject
    for _ in 0..10 {
        client.publish("messages", "data".into()).await?;
    }

    // Receive and process messages
    while let Some(message) = subscriber.next().await {
        println!("Received message {:?}", message);
    }

    Ok(tokio::task::spawn(async move {}))
}
