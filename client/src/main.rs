use futures_util::stream::StreamExt;
use futures_util::SinkExt;
use radars::manager::Manager;
use server::message::{ConfigMessage, ServerMessage};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

#[tokio::main]
async fn main() {
    let url = Url::parse("ws://localhost:3000/ws").expect("Invalid WebSocket URL");

    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");

    let (mut write, mut read) = ws_stream.split();

    let manager: Manager = Manager::new();
    let (manager_tx, manager_rx) = mpsc::channel::<ServerMessage>(100);

    while let Some(message) = read.next().await {
        match message {
            Ok(msg) => {
                if msg.is_binary() {
                    let deserialized =
                        match bincode::deserialize::<ServerMessage>(msg.into_data().as_slice()) {
                            Ok(d) => d,
                            Err(e) => {
                                eprintln!("Error deserializing message: {}", e);
                                break;
                            }
                        };

                    dbg!(&deserialized);
                }
            }
            Err(e) => {
                eprintln!("Error receiving message: {}", e);
                break;
            }
        }
    }

    write
        .send(Message::Text("yeet".to_string()))
        .await
        .expect("Failed to send");
}
