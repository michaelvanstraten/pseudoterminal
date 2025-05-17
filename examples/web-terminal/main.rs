use std::io::{Read, Write};
use std::process::Command;
use std::thread;

use websocket::OwnedMessage;
use websocket::sync::Server;

use pseudoterminal::sync::CommandExt;

fn main() {
    let server = Server::bind("127.0.0.1:3001").unwrap();

    for request in server.filter_map(Result::ok) {
        // Spawn a new thread for each connection.
        thread::spawn(|| {
            let ws = request.accept().unwrap();

            let ip = ws.peer_addr().unwrap();

            println!("Connection from {}", ip);

            let (mut receiver, mut sender) = ws.split().unwrap();

            // Spawn a new pseudoterminal
            cfg_if::cfg_if! {
                if #[cfg(unix)] {
                    let mut cmd = Command::new("bash");
                } else if #[cfg(windows)] {
                    let mut cmd = Command::new("cmd.exe");
                } else {
                    panic!("Unsupported platform")
                }
            }
            let mut terminal = match cmd.spawn_terminal() {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Failed to spawn terminal: {}", e);
                    return;
                }
            };
            let mut termin = terminal.terminal_in.take().unwrap();
            let mut termout = terminal.terminal_out.take().unwrap();

            let send_thread = thread::spawn(move || {
                let mut buffer = [0u8; 1024];
                loop {
                    match termout.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(n) => {
                            let msg = OwnedMessage::Binary(buffer[..n].to_vec());
                            if let Err(e) = sender.send_message(&msg) {
                                eprintln!("Error sending to WebSocket: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("Error reading from PTY: {}", e);
                            break;
                        }
                    }
                }
            });

            for message in receiver.incoming_messages() {
                match message.unwrap() {
                    OwnedMessage::Text(data) => {
                        if let Err(e) = termin.write_all(data.as_bytes()) {
                            eprintln!("Error writing to PTY: {}", e);
                            break;
                        }
                    }
                    OwnedMessage::Close(_) => {
                        // Client closed connection
                        break;
                    }
                    _ => {} // Ignore other message types
                }
            }

            // Wait for both threads to finish
            let _ = send_thread.join();
            println!("Connection from {} closed", ip);
        });
    }
}
