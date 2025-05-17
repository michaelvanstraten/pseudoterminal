use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;
use tokio_tungstenite::{
    accept_async,
    tungstenite::{Error, Result},
};
use tungstenite::protocol::Message;

use pseudoterminal::AsyncCommandExt;

async fn accept_connection(peer: SocketAddr, stream: TcpStream) {
    if let Err(e) = handle_connection(peer, stream).await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => eprintln!("Error processing connection: {err}"),
        }
    }
}

async fn handle_connection(peer: SocketAddr, stream: TcpStream) -> Result<()> {
    let ws_stream = accept_async(stream).await.expect("Failed to accept");
    println!("New WebSocket connection: {peer}");

    // Spawn a new pseudoterminal
    cfg_if::cfg_if! {
        if #[cfg(unix)] {
            let mut cmd = Command::new("bash");
        } else if #[cfg(windows)] {
            let mut cmd = Command::new("powershell.exe");
        } else {
            panic!("Unsupported platform")
        }
    }

    let mut terminal = cmd.spawn_terminal().expect("Failed to spawn terminal");

    // Split the WebSocket stream and terminal for concurrent reads/writes
    let (mut ws_sink, mut ws_stream) = ws_stream.split();
    let (mut terminal_in, mut terminal_out) = terminal.split().unwrap();

    // Forward WebSocket messages to terminal
    let ws_to_terminal = tokio::spawn(async move {
        while let Some(message) = ws_stream.next().await {
            match message {
                Ok(msg) if msg.is_binary() || msg.is_text() => {
                    if let Err(e) = terminal_in.write_all(&msg.into_data()).await {
                        eprintln!("Error writing to terminal: {e}");
                        break;
                    }
                    if let Err(e) = terminal_in.flush().await {
                        eprintln!("Error flushing terminal: {e}");
                        break;
                    }
                }
                Ok(_) => (), // Ignore other message types
                Err(e) => {
                    eprintln!("WebSocket error: {e}");
                    break;
                }
            }
        }
    });

    // Forward terminal output to WebSocket
    let terminal_to_ws = tokio::spawn(async move {
        // Buffer for reading from terminal
        let mut buf = [0u8; 1024];

        loop {
            match terminal_out.read(&mut buf).await {
                Ok(0) => break, // EOF
                Ok(n) => {
                    if let Err(e) = ws_sink.send(Message::binary(buf[..n].to_vec())).await {
                        eprintln!("Error sending to WebSocket: {e}");
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Error reading from terminal: {e}");
                    break;
                }
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = ws_to_terminal => {},
        _ = terminal_to_ws => {},
    };

    Ok(())
}

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:3001";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    println!("Listening on: {addr}");

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream
            .peer_addr()
            .expect("connected streams should have a peer address");
        println!("Peer address: {peer}");

        tokio::spawn(accept_connection(peer, stream));
    }
}
