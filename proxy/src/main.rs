mod check_valid;
use std::io;

use check_valid::is_valid_read_len;
use proxy::{HeadParser, ParseEvent, RequestMeta, BodyKind, ParseError};
use tokio::io::{AsyncReadExt, copy_bidirectional};
use tokio::net::{TcpListener, TcpStream};

struct ResponseMeta {
    content_length: Option<usize>,
    connection_close: bool,
}

enum ConnectionState {
    Idle,
    ReadingHeaders,
    ReadingBody(RequestMeta, Vec<u8>, Vec<u8>), // (meta, raw_headers, body)
    WaitingBackend(RequestMeta, Vec<u8>, Vec<u8>), // (meta, raw_headers, body)
    ReadingResponseHeaders(RequestMeta, Vec<u8>, Vec<u8>, Vec<u8>, TcpStream),
    ReadingResponseBody(RequestMeta, Vec<u8>, Vec<u8>, TcpStream, usize, bool),
    KeepAliveDecision(bool), // true if connection should close
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
            ConnectionState::WaitingBackend(req_meta, raw_headers, body) => {
                println!("Waiting for backend connection...");
                let mut backend_socket = TcpStream::connect("127.0.0.1:81").await?;
                println!("Connected to the backend");

                use tokio::io::AsyncWriteExt;
                let mut request = raw_headers.clone();
                request.extend(&body);
                backend_socket.write_all(&request).await?;

                state = ConnectionState::ReadingResponseHeaders(
                    req_meta, raw_headers, body, Vec::new(), backend_socket,
                );
            }
            ConnectionState::ReadingResponseHeaders(
                req_meta, raw_headers, body, mut resp_buf, mut backend_socket,
            ) => {
                let n = backend_socket.read(&mut temp).await;
                match n {
                    Ok(n) if n > 0 => {
                        resp_buf.extend_from_slice(&temp[..n]);
                    }
                    _ => {
                        state = ConnectionState::Closed;
                        continue;
                    }
                }

                if let Some(end) = resp_buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let resp_headers = resp_buf[..end + 4].to_vec();
                    let resp_meta = parse_response_headers(&resp_headers);
                    let body_start = end + 4;
                    let initial_body = resp_buf[body_start..].to_vec();

                    use tokio::io::AsyncWriteExt;
                    socket.write_all(&resp_headers).await?;
                    socket.write_all(&initial_body).await?;

                    let body_len = initial_body.len();
                    let should_close = req_meta.connection_close || resp_meta.connection_close;

                    match resp_meta.content_length {
                        Some(total_len) if body_len < total_len => {
                            state = ConnectionState::ReadingResponseBody(
                                req_meta, raw_headers, body, backend_socket, total_len - body_len, should_close,
                            );
                        }
                        _ => {
                            state = ConnectionState::KeepAliveDecision(should_close);
                        }
                    }
                } else {
                    state = ConnectionState::ReadingResponseHeaders(
                        req_meta, raw_headers, body, resp_buf, backend_socket,
                    );
                }
            }
            ConnectionState::ReadingResponseBody(
                req_meta, raw_headers, body, mut backend_socket, mut remaining, should_close,
            ) => {
                let n = backend_socket.read(&mut temp).await;
                match n {
                    Ok(n) if n > 0 => {
                        let to_write = n.min(remaining);
                        use tokio::io::AsyncWriteExt;
                        socket.write_all(&temp[..to_write]).await?;
                        remaining -= to_write;

                        if remaining == 0 {
                            state = ConnectionState::KeepAliveDecision(should_close);
                        } else {
                            state = ConnectionState::ReadingResponseBody(
                                req_meta, raw_headers, body, backend_socket, remaining, should_close,
                            );
                        }
                    }
                    _ => {
                        state = ConnectionState::Closed;
                    }
                }
            }
            ConnectionState::KeepAliveDecision(should_close) => {
                if should_close {
                    println!("Closing connection (Connection: close)");
                    state = ConnectionState::Closed;
                } else {
                    println!("Keeping connection alive, waiting for next request");
                    buf.clear();
                    parser = HeadParser::new();
                    state = ConnectionState::ReadingHeaders;
                }
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

fn parse_response_headers(headers: &[u8]) -> ResponseMeta {
    let mut content_length: Option<usize> = None;
    let mut connection_close = false;

    for line in headers.split(|&b| b == b'\n') {
        let line = if line.ends_with(b"\r") { &line[..line.len() - 1] } else { line };
        if line.is_empty() {
            continue;
        }
        // Skip status line (starts with "HTTP/")
        if line.starts_with(b"HTTP/") {
            continue;
        }
        if let Some(colon) = line.iter().position(|&b| b == b':') {
            let name = &line[..colon];
            let value = &line[colon + 1..].trim_ascii_start();
            if name.eq_ignore_ascii_case(b"Content-Length") {
                if let Ok(s) = std::str::from_utf8(value) {
                    if let Ok(len) = s.trim().parse() {
                        content_length = Some(len);
                    }
                }
            } else if name.eq_ignore_ascii_case(b"Connection") {
                if value.eq_ignore_ascii_case(b"close") {
                    connection_close = true;
                }
            }
        }
    }

    ResponseMeta { content_length, connection_close }
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