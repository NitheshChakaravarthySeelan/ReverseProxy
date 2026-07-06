# Load Balancer & Reverse Proxy — Overview

## What

A **learning project** implementing a TCP reverse proxy with HTTP header awareness, built from scratch in Rust. Designed to evolve from a raw byte-pipe proxy into a full-featured, multi-algorithm load balancer.

At its core, the proxy accepts TCP connections on a frontend port, parses HTTP headers incrementally as bytes arrive, optionally reads a request body, then forwards a reconstructed HTTP request to a backend server and relays the response back to the client.

## Why

Built as a hands-on exploration of:
- Systems programming with Rust
- Async I/O with Tokio
- TCP and HTTP at the wire level
- State machine design for network protocols
- Load-balancing algorithms and distributed systems concepts

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Language | Rust (edition 2024) |
| Async Runtime | Tokio 1.49 (feature `"full"`) |
| Build System | Cargo workspace |
| Testing | None yet |
| Dependencies | `tokio` only (proxy crate) |

## Project Structure

```
LoadBalancerAndReverserProxy/
├── proxy/                          # Core proxy and load balancer
│   ├── src/
│   │   ├── main.rs                 # TCP listener + per-connection state machine
│   │   ├── lib.rs                  # HTTP header parser (HeadParser, RequestMeta)
│   │   └── check_valid.rs          # Connection liveness and buffer helpers
│   └── Cargo.toml
├── client/                         # Minimal TCP test client
│   ├── src/
│   │   └── main.rs                 # Sends "hello server", reads response
│   └── Cargo.toml
├── exercise/                       # Scratch code and experiments
│   ├── src/
│   │   ├── main.rs                 # Placeholder
│   │   └── crlf_finder.rs          # \r\n byte-slice finder utility
│   └── Cargo.toml
├── codebase-docs/                  # Generated documentation
│   ├── OVERVIEW.md                 # This file
│   ├── ARCHITECTURE.md             # Deep architecture with diagrams
│   ├── FINDINGS.md                 # Code quality findings
│   ├── NEXT-STEPS.md               # What to implement next
│   ├── MODULES/
│   │   ├── proxy-main.md           # main.rs deep-dive
│   │   └── proxy-lib.md            # lib.rs deep-dive
│   └── assets/
│       ├── architecture.mermaid
│       └── data-flow.mermaid
└── README.md
```

## Quickstart

```bash
# Build everything
cargo build --workspace

# Start the proxy (listens on 127.0.0.1:80, backends on 127.0.0.1:81)
cargo run --bin proxy

# In another terminal, start a test backend
python -m http.server 81

# Test through the proxy
curl http://127.0.0.1:80
```

## Personas & Entry Points

| Persona | Entry Point | What they interact with |
|---------|-------------|------------------------|
| End user | `curl http://127.0.0.1:80` | The proxy's TCP listener |
| Operator | Source code in `proxy/src/` | Configuration (hardcoded), logs |
| Developer | `proxy/src/main.rs:handle_client()` | State machine, parsing, backend relay |
