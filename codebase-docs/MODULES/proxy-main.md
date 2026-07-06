# Module: `proxy/src/main.rs`

## Responsibility

Entry point for the proxy binary. Contains the TCP listener loop and the per-connection state machine (`handle_client`) that drives the entire proxy lifecycle.

## Key Exports / Functions

### `main()`
- Binds a `TcpListener` to `127.0.0.1:80`.
- Enters an infinite accept loop.
- Each accepted connection is spawned into a new Tokio task via `tokio::spawn` calling `handle_client`.

### `handle_client(mut socket: TcpStream) -> io::Result<()>`
The core connection handler. Contains the state machine loop that transitions through:

1. **`ConnectionState::ReadingHeaders`**
   - Reads up to 1024 bytes from the socket into a temp buffer.
   - Extends the accumulation buffer and feeds it to `HeadParser::advance()`.
   - On `ParseEvent::End`, calls `parse_request_meta()` to extract structured metadata.
   - Transitions to `ReadingBody` or `WaitingBackend` based on `BodyKind`.

2. **`ConnectionState::ReadingBody`**
   - If `ContentLength(len)`: reads until `body.len() >= len`.
   - If `Chunked`: prints NYI and closes.
   - If `None`: transitions immediately to `WaitingBackend`.

3. **`ConnectionState::WaitingBackend`**
   - Connects to `127.0.0.1:81`.
   - Reconstructs the HTTP request from `RequestMeta` (request line, Host header, Content-Length).
   - Writes the reconstructed request + body to the backend.
   - Transitions to `RelayingResponse`.

4. **`ConnectionState::RelayingResponse`**
   - Reads from the backend socket in a loop, writing each chunk to the client socket.
   - On EOF (0 bytes), transitions to `KeepAliveDecision`.

5. **`ConnectionState::KeepAliveDecision`**
   - Currently always transitions to `Closed`.

6. **`ConnectionState::Closed`**
   - Returns `Ok(())`, dropping the socket.

### `handle_client1(mut client: TcpStream) -> io::Result<()>`
Simpler alternative that uses `tokio::io::copy_bidirectional` to create a raw TCP pipe between client and backend. No HTTP parsing. Currently unused in production.

## Dependencies

| Dependency | Purpose |
|------------|---------|
| `tokio::net::TcpListener` | Accept TCP connections |
| `tokio::net::TcpStream` | Async read/write to sockets |
| `tokio::io::AsyncReadExt` | `.read()` on sockets |
| `tokio::io::copy_bidirectional` | Raw TCP proxying |
| `proxy::HeadParser` | Incremental HTTP header parsing |
| `proxy::RequestMeta` | Parsed request metadata |
| `proxy::BodyKind` | How to handle the body |
| `proxy::ParseEvent` | Parser advance result |
| `check_valid::is_valid_read_len` | Check for peer disconnect |

## Edge Cases & Gaps

- **Chunked transfer encoding**: The `ReadingBody` arm for `BodyKind::Chunked` immediately closes the connection with a log message. Not implemented.
- **Keep-alive**: Always closes the connection. No HTTP/1.1 keep-alive logic.
- **Header loss**: When reconstructing the request in `WaitingBackend`, only `Host` and `Content-Length` headers are forwarded. All other headers (Cookie, Authorization, Accept, etc.) are silently dropped.
- **No timeouts**: If the backend never responds, the task hangs indefinitely.
- **No error backpressure**: On write error to the client, the state continues or returns error; no attempt to gracefully close the backend.
