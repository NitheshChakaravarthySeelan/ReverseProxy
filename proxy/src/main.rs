mod check_valid;
use std::io;

use check_valid::is_valid_read_len;
use proxy::{HeadParser, ParseEvent, RequestMeta, BodyKind, ParseError};
use tokio::io::{AsyncReadExt, copy_bidirectional};
use tokio::net::{TcpListener, TcpStream};

enum ConnectionState {
    Idle,
    ReadingHeaders,
    ReadingBody(RequestMeta, Vec<u8>, Vec<u8>), // (meta, raw_headers, body)
    WaitingBackend(RequestMeta, Vec<u8>, Vec<u8>), // (meta, raw_headers, body)
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
                            match parser.parse_request_meta() {
                                Ok(req_meta) => {
                                    let raw_headers = buf[..parser.cursor].to_vec();
                                    let initial_body = buf[parser.cursor..].to_vec();
                                    
                                    state = match req_meta.body_kind {
                                        BodyKind::None => ConnectionState::WaitingBackend(req_meta, raw_headers, initial_body),
                                        BodyKind::ContentLength(_) | BodyKind::Chunked => {
                                            ConnectionState::ReadingBody(req_meta, raw_headers, initial_body)
                                        }
                                    };
                                    
                                    buf.clear();
                                    parser.lines.clear();
                                    parser.cursor = 0;
                                    break;
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
            ConnectionState::ReadingBody(req_meta, raw_headers, mut body) => {
                match req_meta.body_kind {
                    BodyKind::ContentLength(len) => {
                        if body.len() >= len {
                            state = ConnectionState::WaitingBackend(req_meta, raw_headers, body);
                        } else {
                            let n = socket.read(&mut temp).await?;
                            if !is_valid_read_len(n) {
                                state = ConnectionState::Closed;
                            } else {
                                body.extend_from_slice(&temp[..n]);
                                state = ConnectionState::ReadingBody(req_meta, raw_headers, body);
                            }
                        }
                    }
                    BodyKind::Chunked => {
                        if is_chunked_body_complete(&body) {
                            state = ConnectionState::WaitingBackend(req_meta, raw_headers, body);
                        } else {
                            let n = socket.read(&mut temp).await?;
                            if !is_valid_read_len(n) {
                                state = ConnectionState::Closed;
                            } else {
                                body.extend_from_slice(&temp[..n]);
                                state = ConnectionState::ReadingBody(req_meta, raw_headers, body);
                            }
                        }
                    }
                    BodyKind::None => {
                        state = ConnectionState::WaitingBackend(req_meta, raw_headers, body);
                    }
                }
            }
            ConnectionState::WaitingBackend(_req_meta, raw_headers, body) => {
                println!("Waiting for backend connection...");
                let mut backend_socket = TcpStream::connect("127.0.0.1:81").await?;
                println!("Connected to the backend");

                use tokio::io::AsyncWriteExt;
                let mut request = raw_headers;
                request.extend(body);
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

fn is_chunked_body_complete(body: &[u8]) -> bool {
    let mut pos = 0;
    loop {
        let crlf = match body[pos..].windows(2).position(|w| w == b"\r\n") {
            Some(p) => p,
            None => return false,
        };
        let size_str = match std::str::from_utf8(&body[pos..pos + crlf]) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let chunk_size = match usize::from_str_radix(size_str.trim(), 16) {
            Ok(s) => s,
            Err(_) => return false,
        };
        pos += crlf + 2;

        if chunk_size == 0 {
            return pos + 2 <= body.len() && body[pos..pos + 2] == *b"\r\n";
        }

        if pos + chunk_size + 2 > body.len() {
            return false;
        }
        pos += chunk_size + 2;
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