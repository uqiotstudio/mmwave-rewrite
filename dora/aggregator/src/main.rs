use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};

async fn handle_client(mut stream: OwnedReadHalf, sender: Arc<mpsc::Sender<String>>) {
    let mut buffer = [0; 1024];

    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => {
                // Client has closed the connection
                println!("Client disconnected");
                break;
            }
            Ok(size) => {
                let message = String::from_utf8_lossy(&buffer[0..size]).to_string();
                let _ = sender.send(message).await;
            }
            Err(e) => {
                println!("Error with client: {}", e);
                break;
            }
        }
    }
}

async fn handle_configuration(stream: OwnedWriteHalf, receiver: Arc<watch::Receiver<String>>) {
    let mut buffer = [0; 1024];
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").await.unwrap();
    let (mpsc_tx, mut mpsc_rx) = mpsc::channel(1);
    let mpsc_sender = Arc::new(mpsc_tx);

    let (watch_tx, mut watch_rx) = watch::channel("".into());
    let watch_receiver = Arc::new(watch_rx);

    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let mpsc_sender = mpsc_sender.clone();
            let watch_receiver = watch_receiver.clone();

            let (mut read_half, mut write_half) = stream.into_split();

            tokio::spawn(async move {
                handle_client(read_half, mpsc_sender).await;
            });

            tokio::spawn(async move {
                handle_configuration(write_half, watch_receiver).await;
            });
        }
    });

    while let Some(message) = mpsc_rx.recv().await {
        println!("Received: {}", message);
        let _ = watch_tx.send(format!("What comes around, goes around: {}", message));
    }
}
