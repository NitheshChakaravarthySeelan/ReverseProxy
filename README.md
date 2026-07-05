# Load Balancer & Reverse Proxy

A **learning project** that implements a TCP reverse proxy with HTTP header awareness, built from scratch in Rust. Designed to evolve from a raw byte-pipe proxy into a feature-rich, multi-algorithm load balancer.

```
client ──> TCP ──> proxy ──> TCP ──> backend
```

---

## Features

- **TCP Reverse Proxy** – Accepts connections on a frontend port and forwards them to a backend server.
- **HTTP Header Parsing** – Incrementally parses HTTP request lines, headers, Content-Length, and chunked transfer encoding.
- **State Machine Architecture** – Each connection progresses through explicit states: `ReadingHeaders` → `ReadingBody` → `WaitingBackend` → `RelayingResponse`.
- **Async I/O** – Built on Tokio for non-blocking, high-concurrency connection handling.
- **Raw Bidirectional Proxying** – Supports `io::copy_bidirectional` for transparent TCP passthrough.

---

## Architecture

The proxy is a **multi-crate Cargo workspace** with three packages:

| Crate       | Role                                                  |
|-------------|-------------------------------------------------------|
| `proxy`     | Core proxy server – TCP listener, state machine, HTTP parser |
| `client`    | Minimal TCP test client for manual testing            |
| `exercise`  | Scratch code / experimental utilities                 |

### Connection Lifecycle

Each client connection is handled by an async state machine:

```
Idle ──> ReadingHeaders ──> ReadingBody ──> WaitingBackend ──> RelayingResponse ──> Closed
```

The proxy reads the client's HTTP request, parses headers to determine routing metadata (method, URI, host, body type), then establishes a connection to the backend and relays both the reconstructed request and the backend's response.

### Crate Structure

```
proxy/
├── src/
│   ├── main.rs              # TCP listener, connection accept, state machine
│   ├── lib.rs               # HTTP parser: HeadParser, RequestMeta, BodyKind
│   └── check_valid.rs       # Utility: connection liveness checks, buffer helpers
├── Cargo.toml
└── README.md
```

```
client/
├── src/
│   └── main.rs              # Simple TCP sender
├── Cargo.toml
└── README.md
```

```
exercise/
├── src/
│   ├── main.rs              # Placeholder
│   └── crlf_finder.rs       # \r\n byte-slice finder
├── Cargo.toml
```

---

## Getting Started

### Prerequisites

- Rust (edition 2024) — install via [rustup](https://rustup.rs/)
- A backend server to test against (e.g., `python -m http.server 81`)

### Build

```bash
cargo build --workspace
```

### Run

```bash
# Start the proxy (listens on 127.0.0.1:80, backends on 127.0.0.1:81)
cargo run --bin proxy

# In another terminal, start a test backend
python -m http.server 81

# Send a request through the proxy
curl http://127.0.0.1:80
```

### Test Client

```bash
cargo run --bin client
```

---

## Configuration

Currently, all configuration is **hardcoded** in `proxy/src/main.rs`:

| Setting            | Value          |
|--------------------|----------------|
| Listen address     | `127.0.0.1:80` |
| Backend address    | `127.0.0.1:81` |
| Async runtime      | Tokio (full)   |

---

## HTTP Parser API

The `HeadParser` in `lib.rs` provides incremental, zero-copy-friendly header parsing:

```rust
let mut parser = HeadParser::new();
parser.consume(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n");
match parser.parse() {
    ParseEvent::End => {
        let meta = parser.parse_request_meta().unwrap();
        println!("{} {}", meta.method, meta.uri);
    }
    _ => {}
}
```

Supported `BodyKind` detection:
- `ContentLength(n)` — parsed from `Content-Length` header
- `Chunked` — parsed from `Transfer-Encoding: chunked`
- `None` — no body present

---

## Roadmap

### Phase 1 (Current) – TCP Proxy with HTTP Awareness
- [x] TCP listener with per-connection async handlers
- [x] Incremental HTTP header parser
- [x] Connection state machine (headers → body → backend → relay)
- [x] Raw bidirectional byte proxying (`copy_bidirectional`)
- [ ] Chunked transfer encoding body forwarding
- [ ] Connection keep-alive support

### Phase 2 – Load Balancer
- [ ] Round-robin, least-connections, and random backend selection
- [ ] Backend health checks (active / passive)
- [ ] Connection pooling

### Phase 3 – Production Features
- [ ] YAML/TOML configuration file
- [ ] TLS termination
- [ ] Metrics and observability (Prometheus, structured logging)
- [ ] Hot-reload of backend pool

---

## Development

```bash
# Run the proxy with logging (add env_logger or tracing as needed)
RUST_LOG=info cargo run --bin proxy

# Run tests
cargo test --workspace
```

---

## License

This project is licensed under the MIT License. See [LICENSE](./LICENSE) for details.

---

## Acknowledgements

Built as a hands-on exploration of systems programming, async I/O, HTTP semantics, and load-balancing algorithms — one connection at a time.
