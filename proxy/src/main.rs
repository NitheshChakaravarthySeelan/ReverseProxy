mod check_valid;
use std::io;

use check_valid::is_valid_read_len;
use proxy::{HeadParser, ParseEvent, RequestMeta, BodyKind, ParseError};
use tokio::io::{AsyncReadExt, copy_bidirectional};
use tokio::net::{TcpListener, TcpStream};

enum ConnectionState {
    Idle,
    ReadingHeaders,
    ReadingBody(RequestMeta), // Holds RequestMeta once headers are parsed
    WaitingBackend,
    RelayingResponse,
    KeepAliveDecision,
    Closed,
}

/// 1. Read all data from the client connection.
/// 2. Write all that data to the backend connection.
/// 3. Read all that data from the backend connection.
/// 4. Write all that data to the client connection.
async fn handle_client(mut socket: TcpStream) -> tokio::io::Result<()> {
    let mut buf: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut temp = [0u8; 1024];
    let mut parser = HeadParser::new();
    let mut state = ConnectionState::ReadingHeaders;

    loop {
        match state {
            ConnectionState::ReadingHeaders => {
                let n = socket.read(&mut temp).await?;

                if !is_valid_read_len(n) {
                    state = ConnectionState::Closed;
                    continue;
                }

                buf.extend_from_slice(&temp[..n]);

                loop {
                    match parser.advance(&buf) {
                        ParseEvent::Line(_line) => {
                            // Header line processed, stored in parser.lines
                        }
                        ParseEvent::End => {
                            println!("Header complete");
                            // Headers are complete, parse RequestMeta
                            match parser.parse_request_meta() {
                                Ok(req_meta) => {
                                    // Decide next state based on body_kind
                                    state = match req_meta.body_kind {
                                        BodyKind::None => ConnectionState::WaitingBackend,
                                        BodyKind::ContentLength(_) | BodyKind::Chunked => {
                                            ConnectionState::ReadingBody(req_meta)
                                        }
                                    };
                                    // Clear parser lines for the next request if connection is kept alive
                                    parser.lines.clear();
                                    // Clear buffer for the next request as headers are processed
                                    buf.clear();
                                    break; // Exit inner loop, proceed to next state
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse request meta: {:?}", e);
                                    state = ConnectionState::Closed;
                                    break;
                                }
                            }
                        }
                        ParseEvent::NeedMore => {
                            // Need more data to complete headers
                            break; // Exit inner loop, wait for more data
                        }
                    }
                }
            }
            ConnectionState::ReadingBody(req_meta) => {
                println!("Reading body based on {:?}", req_meta.body_kind);
                // Placeholder for reading the body
                state = ConnectionState::WaitingBackend;
            }
            ConnectionState::WaitingBackend => {
                println!("Waiting for backend connection...");
                // Placeholder for connecting to backend and forwarding request
                let mut backend_socket = TcpStream::connect("127.0.0.1:81").await?;
                println!("Connected to the backend");
                state = ConnectionState::RelayingResponse;
            }
            ConnectionState::RelayingResponse => {
                println!("Relaying response...");
                // Placeholder for relaying response
                state = ConnectionState::KeepAliveDecision;
            }
            ConnectionState::KeepAliveDecision => {
                println!("Deciding on keep-alive...");
                // For now, just close the connection
                state = ConnectionState::Closed;
            }
            ConnectionState::Closed => {
                println!("Connection closed.");
                return Ok(());
            }
            ConnectionState::Idle => {
                eprintln!("Unexpected Idle state.");
                return Err(io::Error::new(io::ErrorKind::Other, "Unexpected Idle state"));
            }
        }
    }
}

// will change just for end product.
async fn handle_client1(mut client: TcpStream) -> io::Result<()> {
    let mut backend = TcpStream::connect("127.0.0.1:81").await?;
    copy_bidirectional(&mut client, &mut backend).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:80").await?;

    loop {
        // Accepting connection async socket should be mut cause we will be writing back the
        // response in the socket.
        // it would give socket and addr
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            if let Err(e) = handle_client(socket).await {
                eprintln!("connection error: {e}");
            }
        });
    }
}