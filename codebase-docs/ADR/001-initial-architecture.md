# ADR 001: Initial Architecture — Single-Process State Machine Proxy

## Status

Accepted (retrospective).

## Context

The project started as a learning exercise to understand TCP proxying and HTTP at the wire level. The requirements at the time were minimal:
- Accept TCP connections on a port
- Forward them to a backend
- Optionally understand the HTTP protocol to enable future load balancing

## Decision

Use a **single-process, single-threaded (async) model** with:
1. A Tokio `TcpListener` accepting connections in a loop.
2. Each connection spawned as a Tokio task.
3. An explicit `enum`-based state machine within each task to track connection progress.
4. An incremental HTTP parser that works on byte slices rather than requiring the full request.

## Consequences

**Positive:**
- Simple to reason about — the state machine is linear and explicit.
- No shared mutable state between connections (each task owns its data).
- The incremental parser enables pipelining and keeps memory usage proportional to request size, not connection count.

**Negative:**
- All logic lives in one function (`handle_client`), making it hard to unit test individual states.
- Adding features like keep-alive or load balancing will increase complexity within the same function unless it's refactored.
- No separation between parsing, routing, and IO concerns.

## Alternatives Considered

- **Raw TCP pipe (no HTTP awareness):** Simpler but cannot do load balancing based on HTTP-level info (path, headers, cookies).
- **Hyper-based proxy:** Would give production-grade HTTP handling but hides the wire-level details we wanted to learn.
- **Multi-threaded with shared state:** Unnecessarily complex for the learning goals and current scale.

## Future ADRs

When load balancing is added, a new ADR should document the backend selection strategy and whether state is managed centrally (via `Arc<Mutex<...>>`) or via channels.
