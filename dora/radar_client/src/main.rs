use std::str::from_utf8;
use tokio::io::{self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut stream = TcpStream::connect("127.0.0.1:7878").await?;

    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin).lines();

    while let Some(line) = reader.next_line().await? {
        let line = line + "\n"; // Append newline, since next_line() trims it
        stream.write_all(line.as_bytes()).await?;

        let mut buffer = [0; 1024];
        let duration = Duration::from_millis(250);
        if let Ok(res) = timeout(duration, stream.read(&mut buffer)).await {
            match res {
                Ok(0) => {
                    println!("Server closed the connection");
                    break;
                }
                Ok(_) => {
                    print!("Server response: {}", from_utf8(&buffer).unwrap());
                }
                Err(e) => {
                    println!("Failed to receive data: {}", e);
                    break;
                }
            }
        } else {
            println!("Server response timed out!");
        }
    }
    Ok(())
}
