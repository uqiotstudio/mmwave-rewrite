use futures_util::stream::StreamExt;
use futures_util::SinkExt;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

#[tokio::main]
async fn main() {
    let url = Url::parse("ws://localhost:3000/ws").expect("Invalid WebSocket URL");

    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");

    let (mut write, mut read) = ws_stream.split();

    while let Some(message) = read.next().await {
        match message {
            Ok(msg) => {
                dbg!(msg);
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
