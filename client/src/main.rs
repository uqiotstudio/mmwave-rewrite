use std::{sync::Arc, thread, time::Duration};

use futures_util::stream::StreamExt;
use futures_util::SinkExt;
use radars::{config::Configuration, manager::Manager};
use server::message::{ConfigMessage, ServerMessage};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

#[tokio::main]
async fn main() {
    let url = loop {
        match Url::parse("ws://localhost:3000/ws") {
            Ok(url) => {
                println!("Found server, {:?}", url);
                break url;
            }
            Err(e) => {
                eprintln!("Unable to find server with err {:?}, retrying", e);
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    };

    let ws_stream = loop {
        match connect_async(url.clone()).await {
            Ok((ws_stream, _)) => {
                println!("Connected to server");
                break ws_stream;
            }
            Err(e) => {
                eprintln!("Unable to connect with err {:?}, retrying", e);
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    };

    // Get the web socket channels
    let (mut write, mut read) = ws_stream.split();

    // Create a manager and communication channels
    let (manager_tx, mut manager_rx) = mpsc::channel::<ServerMessage>(100);

    let manager_original = Arc::new(Mutex::new(Manager::new()));

    // Configure the manager
    let manager = manager_original.clone();
    tokio::task::spawn(async move {
        while let Some(thing) = manager_rx.recv().await {
            match thing {
                ServerMessage::ConfigMessage(cfg) => {
                    let mut lock = manager.lock().await;
                    println!("Reconfiguring Manager");
                    lock.set_config(cfg.config);
                    thread::sleep(Duration::from_secs(1));
                }
                ServerMessage::PointCloudMessage(_) => todo!(),
            }
        }
    });

    // Receive messages from the server and respond accordingly
    tokio::task::spawn(async move {
        while let Some(message) = read.next().await {
            match message {
                Ok(msg) => {
                    if msg.is_text() {
                        let deserialized = match serde_json::from_str::<ServerMessage>(
                            msg.into_text().unwrap().as_str(),
                        ) {
                            Ok(d) => d,
                            Err(e) => {
                                eprintln!("Error deserializing message: {}", e);
                                break;
                            }
                        };
                        println!("Received Message From Server");

                        match deserialized {
                            ServerMessage::ConfigMessage(cfg) => {
                                // Forward new config to the manager
                                manager_tx.send(ServerMessage::ConfigMessage(cfg)).await;
                            }
                            ServerMessage::PointCloudMessage(_) => todo!(),
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error receiving message: {}", e);
                    break;
                }
            }
        }
    });

    // Read from the manager and send pointclouds to the server
    let manager = manager_original.clone();
    tokio::task::spawn(async move {
        loop {
            let mut lock = manager.lock().await;
            let result = lock.receive().await;
        }
    })
    .await
    .ok();
}
