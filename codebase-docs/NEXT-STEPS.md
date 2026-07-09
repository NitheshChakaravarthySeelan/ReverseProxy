# Next Steps — What to Implement

This document breaks down everything needed to evolve the proxy into a working load balancer. Each item is ordered by dependency — implementing earlier items unblocks later ones.

---

## Phase 1 — Fix the Proxy Bugs (High Priority)

These are issues that make the proxy incorrect or broken for real HTTP traffic.

### 1.1 Forward All Headers

**Where:** `proxy/src/main.rs:117-138` (the request reconstruction in `WaitingBackend`)

**Problem:** Currently only `Host` and `Content-Length` are forwarded. All other headers (Cookie, Authorization, Accept, User-Agent, etc.) are silently dropped. This breaks most real HTTP traffic.

**What to do:** Instead of reconstructing from scratch, keep a copy of the original raw headers (everything except the body) and forward them verbatim. The reconstruction approach is good for a load balancer (you want to control what goes through), but you need a whitelist or a pass-through mechanism.

**Design decision:** Do you want to:
- (a) Forward all original headers verbatim + add/modify a few?
- (b) Reconstruct from `RequestMeta` with a whitelist of allowed headers?
- (c) Forward original but strip `Proxy-*` headers?

Start with (a) — simplest and most correct.

### 1.2 Implement Chunked Body Reading

**Where:** `proxy/src/main.rs:99-102`

**Problem:** `BodyKind::Chunked` just prints "not yet implemented" and closes the connection. Any HTTP request with `Transfer-Encoding: chunked` (common for POST/PUT with large bodies) will fail.

**What to do:** Implement a chunked body reader that:
1. Reads the chunk size line (hex digits + `\r\n`)
2. Reads that many bytes of chunk data
3. Reads the trailing `\r\n`
4. Repeats until a zero-length chunk is found
5. Handles optional trailer headers after the last chunk

**Tip:** Write a separate `ChunkedDecoder` struct rather than putting this logic in the state machine. Test it with known chunked payloads.

### 1.3 Implement Keep-Alive

**Where:** `proxy/src/main.rs:164-168`

**Problem:** The proxy always closes the TCP connection after one request-response cycle. HTTP/1.1 defaults to persistent connections (`Connection: keep-alive`). Clients must create a new TCP connection for every request, which is very slow.

**What to do:**
1. Check `connection_close` in `RequestMeta` after the response is relayed.
2. If `Connection: close` was sent by either side → close.
3. If both sides support keep-alive → reset the parser and buffer, transition back to `ReadingHeaders` (without dropping the socket).
4. **Critical:** You must read the correct `Content-Length` of the response from the backend too, so you know when to stop reading the response and start reading the next request.

**This is the hardest of the three Phase 1 items** because now you need to:
- Parse the backend's response headers too (to know `Content-Length` of the response)
- Handle the case where a pipelined request is already in the buffer

---

## Phase 2 — Multiple Backends & Load Balancing

### 2.1 Backend Pool

Replace the hardcoded `127.0.0.1:81` with a dynamic list of backend addresses.

```rust
struct BackendPool {
    backends: Vec<SocketAddr>,
    // ... strategy state
}
```

Start by loading from a hardcoded `vec![]` in `main()`, then later from a config file.

### 2.2 Load Balancing Algorithms

Implement at least two strategies:

| Algorithm | Strategy | When to use |
|-----------|----------|-------------|
| **Round Robin** | Cycle through backends in order | Equal-capacity backends |
| **Least Connections** | Pick backend with fewest active connections | Variable-load backends |
| **Random** | Pick a random backend | Testing, simplicity |

Make it pluggable:

```rust
trait LoadBalanceStrategy {
    fn pick(&mut self, pool: &BackendPool) -> Option<usize>;
}
```

### 2.3 Connection Pooling

Rather than opening a new TCP connection to the backend for every client request, maintain a pool of pre-connected backend sockets. This reduces latency (no TCP handshake per request) and load on backends.

**What to do:**
1. Create a `BackendConnectionPool` that holds a set of idle `TcpStream` instances for each backend.
2. On `WaitingBackend`, check out an idle connection or create a new one.
3. On `KeepAliveDecision`, return the backend connection to the pool instead of dropping it.

---

## Phase 3 — Health Checks

### 3.1 Passive Health Checks

If a backend connection fails (connect error, read error), mark that backend as "unhealthy" and skip it for a cooldown period. If it succeeds on retry, mark it healthy again.

### 3.2 Active Health Checks

Periodically (e.g., every 5 seconds) send a health-check request (e.g., `GET /health`) to each backend. If it doesn't respond with 200 within a timeout, mark it unhealthy.

---

## Phase 4 — Configuration & Observability

### 4.1 Configuration File

Add a YAML or TOML config file:

```toml
listen = "0.0.0.0:80"

[[backends]]
address = "10.0.0.1:8080"
weight = 3

[[backends]]
address = "10.0.0.2:8080"
weight = 1

[strategy]
type = "round-robin"

[health_check]
interval_secs = 5
path = "/health"
timeout_ms = 2000
```

### 4.2 Structured Logging

Add the `tracing` or `log` crate. Log:
- Connection accepted / closed
- Backend selected
- Request method + URI + status code
- Health check results

### 4.3 Metrics

Expose Prometheus-style metrics:
- `requests_total` (counter, by backend)
- `active_connections` (gauge)
- `backend_up` (gauge, by backend)
- `request_duration_seconds` (histogram)

---

## Status — All Items Implemented ✅

### Phase 1 — Proxy Correctness
- [x] 1.1 Forward all headers (raw header bytes preserved)
- [x] 1.2 Chunked body reading
- [x] 1.3 Keep-alive (response header parsing, Content-Length tracking, connection reuse)

### Phase 2 — Load Balancing
- [x] 2.1 Backend pool with round-robin selection
- [x] 2.2 Multiple backends (configurable via proxy.toml)
- [x] 2.3 Connection pooling (idle connection reuse)

### Phase 3 — Health Checks
- [x] 3.1 Passive health checks (mark unhealthy on connection failure)
- [x] 3.2 Active health checks (periodic GET /health probes)

### Phase 4 — Production Features
- [x] 4.1 TOML configuration file
- [x] 4.2 Structured logging (tracing crate)
- [x] 4.3 Prometheus metrics endpoint (:9090/metrics)

---

## Future Ideas (beyond this implementation)

- **TLS termination** — Accept HTTPS, forward HTTP to backends.
- **HTTP/2 support** — Significantly more complex but increasingly necessary.
- **WebSocket support** — Detect `Upgrade: websocket` and switch to raw TCP pipe mode.
- **Rate limiting** — Per-IP or per-route request throttling.
- **Circuit breaker** — If a backend fails N times in a window, stop sending traffic for a recovery period.
- **Admin API** — An HTTP endpoint to view backend status, toggle health, update config at runtime.
