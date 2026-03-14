mod check_valid;
use std::io;

use check_valid::is_valid_read_len;
use proxy::{HeadParser, ParseEvent, RequestMeta, BodyKind, ParseError};
use tokio::io::{AsyncReadExt, copy_bidirectional};
use tokio::net::{TcpListener, TcpStream};

enum ConnectionState {
    Idle,
    ReadingHeaders,
    ReadingBody(RequestMeta, Vec<u8>), // Holds RequestMeta and initial body
    WaitingBackend(RequestMeta, Vec<u8>), // Holds RequestMeta and full body
    RelayingResponse(TcpStream), // Holds the backend socket
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
                                    // Extract the initial body part that was read with headers
                                    let initial_body = buf[parser.cursor..].to_vec();
                                    
                                    // Decide next state based on body_kind
                                    state = match req_meta.body_kind {
                                        BodyKind::None => ConnectionState::WaitingBackend(req_meta, initial_body),
                                        BodyKind::ContentLength(_) | BodyKind::Chunked => {
                                            ConnectionState::ReadingBody(req_meta, initial_body)
                                        }
                                    };
                                    
                                    // We keep buf as is for now if we want to relay headers later, 
                                    // or clear it if we'll reconstruct headers.
                                    // For now, let's clear it since we have RequestMeta and initial_body.
                                    buf.clear();
                                    parser.lines.clear();
                                    parser.cursor = 0;
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
            ConnectionState::ReadingBody(req_meta, mut body) => {
                match req_meta.body_kind {
                    BodyKind::ContentLength(len) => {
                        if body.len() >= len {
                            state = ConnectionState::WaitingBackend(req_meta, body);
                        } else {
                            let n = socket.read(&mut temp).await?;
                            if !is_valid_read_len(n) {
                                state = ConnectionState::Closed;
                            } else {
                                body.extend_from_slice(&temp[..n]);
                                state = ConnectionState::ReadingBody(req_meta, body);
                            }
                        }
                    }
                    BodyKind::Chunked => {
                        println!("Reading chunked body (not yet implemented)");
                        state = ConnectionState::Closed; // Placeholder
                    }
                    BodyKind::None => {
                        state = ConnectionState::WaitingBackend(req_meta, body);
                    }
                }
            }
            ConnectionState::WaitingBackend(req_meta, body) => {
                println!("Waiting for backend connection...");
                // Placeholder for connecting to backend and forwarding request
                let mut backend_socket = TcpStream::connect("127.0.0.1:81").await?;
                println!("Connected to the backend");
                
                // For now, let's just relay what we have. 
                // We need to reconstruct the headers or keep the original ones.
                // Reconstructing is cleaner for a Load Balancer.
                let mut request = format!(
                    "{} {} {:?}\r\n",
                    String::from_utf8_lossy(&req_meta.method),
                    String::from_utf8_lossy(&req_meta.uri),
                    String::from_utf8_lossy(&req_meta.http_version)
                ).into_bytes();
                
                // Add Host header if present
                if let Some(host) = &req_meta.host {
                    request.extend_from_slice(b"Host: ");
                    request.extend_from_slice(host);
                    request.extend_from_slice(b"\r\n");
                }
                
                // Add other headers... this is where we'd need more logic to preserve headers.
                // For learning, let's just add Content-Length if body is present.
                if !body.is_empty() {
                    request.extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
                }
                
                request.extend_from_slice(b"\r\n");
                request.extend(body);
                
                use tokio::io::AsyncWriteExt;
                backend_socket.write_all(&request).await?;
                
                state = ConnectionState::RelayingResponse(backend_socket);
            }
            ConnectionState::RelayingResponse(mut backend_socket) => {
                println!("Relaying response...");
                // Stream from backend to client
                let mut backend_buf = [0u8; 1024];
                match backend_socket.read(&mut backend_buf).await {
                    Ok(0) => {
                        state = ConnectionState::KeepAliveDecision;
                    }
                    Ok(n) => {
                        use tokio::io::AsyncWriteExt;
                        socket.write_all(&backend_buf[..n]).await?;
                        state = ConnectionState::RelayingResponse(backend_socket);
                    }
                    Err(e) => {
                        eprintln!("Failed to read from backend: {}", e);
                        state = ConnectionState::Closed;
                    }
                }
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