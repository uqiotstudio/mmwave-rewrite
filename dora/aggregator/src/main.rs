use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener,
    },
    sync::{mpsc, watch},
};

// Forwards messages from the stream to the sender
async fn handle_incoming(mut stream: OwnedReadHalf, sender: mpsc::Sender<String>) {
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
                eprintln!("Error receiving from client: {}", e);
                break;
            }
        }
    }
}

// Forwards messages from receiver to the streams writehalf
async fn handle_outgoing(mut stream: OwnedWriteHalf, mut receiver: watch::Receiver<String>) {
    loop {
        match receiver.changed().await {
            Ok(()) => {
                let message = receiver.borrow_and_update().clone();
                let buffer = &message.into_bytes()[..];
                if let Err(e) = stream.write_all(buffer).await {
                    eprintln!("Error sending to client: {}", e);
                    break;
                }
                if let Err(e) = stream.flush().await {
                    eprintln!("Error sending to client: {}", e);
                    break;
                }
            }
            Err(e) => {
                eprintln!("Error sending to client: {}", e);
                break;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").await.unwrap();
    let (mpsc_tx, mut mpsc_rx) = mpsc::channel(1);
    let mpsc_sender = mpsc_tx;

    let (watch_tx, watch_rx) = watch::channel("".into());
    let watch_receiver = watch_rx;

    dbg!("a");
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let mpsc_sender = mpsc_sender.clone();
            let watch_receiver = watch_receiver.clone();

            let (read_half, write_half) = stream.into_split();

            tokio::spawn(async move {
                handle_incoming(read_half, mpsc_sender).await;
            });

            tokio::spawn(async move {
                handle_outgoing(write_half, watch_receiver).await;
            });
        }
    });

    while let Some(message) = mpsc_rx.recv().await {
        println!("Received: {}", message);
        let _ = watch_tx.send(format!("What comes around, goes around: {}", message));
    }
}
