# Architecture

## System Context (C4 Level 1)

```mermaid
graph TB
    User([User / curl / browser])
    Proxy([TCP Reverse Proxy<br/>:80])
    Backend([Backend Server<br/>:81])

    User -- "TCP connection" --> Proxy
    Proxy -- "TCP connection" --> Backend
    Proxy -- "reads/writes HTTP" --> User
    Proxy -- "forwards reconstructed HTTP" --> Backend
```

## Container Diagram

```mermaid
graph TB
    subgraph "proxy crate"
        Listener["TcpListener<br/>:80"]
        StateMachine["handle_client()<br/>Connection State Machine"]
        Parser["HeadParser<br/>Incremental HTTP Parser"]
        BackendConn["TcpStream<br/>→ :81"]
    end

    subgraph "client crate"
        TestClient["Test TCP Client<br/>sends 'hello server'"]
    end

    subgraph "exercise crate"
        CrlfFinder["crlf_finder.rs<br/>\\r\\n byte finder"]
        Scratch["main.rs<br/>placeholder"]
    end

    Listener -->|"accepts"| StateMachine
    StateMachine -->|"reads bytes"| Parser
    StateMachine -->|"connects to"| BackendConn
    TestClient -.->|"manual test"| Listener
```

## Connection State Machine

This is the core of the proxy. Each accepted TCP connection is wrapped in an async task that runs this state machine:

```mermaid
stateDiagram-v2
    [*] --> ReadingHeaders
    ReadingHeaders --> ReadingBody : body present
    ReadingHeaders --> WaitingBackend : no body
    ReadingBody --> WaitingBackend : body complete
    ReadingBody --> Closed : chunked (NYI)
    WaitingBackend --> RelayingResponse : request sent
    RelayingResponse --> KeepAliveDecision : response complete
    KeepAliveDecision --> Closed : close (always)
    ReadingHeaders --> Closed : read error / disconnect
    ReadingBody --> Closed : read error / disconnect
    WaitingBackend --> Closed : backend connect error
    RelayingResponse --> Closed : backend read error
    Closed --> [*]
```

### State Details

| State | Responsibility | Data Carried |
|-------|---------------|--------------|
| `ReadingHeaders` | Read raw bytes, feed to `HeadParser` until headers complete | `buf: Vec<u8>`, `parser: HeadParser` |
| `ReadingBody` | Accumulate body bytes based on `Content-Length` or `Chunked` | `RequestMeta`, `body: Vec<u8>` |
| `WaitingBackend` | Connect to backend, reconstruct and forward HTTP request | `RequestMeta`, `body: Vec<u8>` |
| `RelayingResponse` | Stream bytes from backend socket to client socket | `backend_socket: TcpStream` |
| `KeepAliveDecision` | Decide whether to reuse connection (currently always closes) | Nothing |
| `Closed` | Terminal — return from handler | Nothing |

## Data Flow (End-to-End Request)

```mermaid
sequenceDiagram
    participant Client
    participant Proxy
    participant Parser as HeadParser
    participant Backend

    Client->>Proxy: TCP SYN
    Proxy->>Client: TCP SYN-ACK
    Client->>Proxy: TCP ACK
    Client->>Proxy: HTTP GET /index.html HTTP/1.1\r\nHost: ...\r\n\r\n

    Note over Proxy,Parser: State: ReadingHeaders
    loop Until \\r\\n\\r\\n
        Proxy->>Parser: advance(buf)
        Parser-->>Proxy: Line / NeedMore / End
    end
    Proxy->>Parser: parse_request_meta()
    Parser-->>Proxy: RequestMeta { method, uri, host, body_kind }

    Note over Proxy: State: WaitingBackend (no body)

    Proxy->>Backend: TCP connect :81
    Backend->>Proxy: TCP accept
    Proxy->>Backend: Reconstructed HTTP request

    Note over Proxy: State: RelayingResponse

    Backend->>Proxy: HTTP 200 OK\r\n...
    loop Until backend closes
        Proxy->>Client: relay bytes
    end

    Note over Proxy: State: KeepAliveDecision → Closed
    Proxy->>Client: TCP FIN
```

## Module Dependency Graph

```mermaid
graph LR
    subgraph "proxy binary (main.rs)"
        handle_client["handle_client()"]
        main["main()"]
    end

    subgraph "proxy library (lib.rs)"
        HeadParser["HeadParser"]
        RequestMeta["RequestMeta"]
        BodyKind["BodyKind"]
        ParseEvent["ParseEvent"]
        ParseError["ParseError"]
    end

    subgraph "check_valid.rs"
        is_valid_read_len["is_valid_read_len()"]
        buffer_window_end["buffer_window_end()"]
    end

    subgraph "external"
        Tokio["tokio"]
    end

    main --> handle_client
    handle_client --> HeadParser
    handle_client --> RequestMeta
    handle_client --> BodyKind
    handle_client --> is_valid_read_len
    handle_client --> Tokio
    HeadParser --> ParseEvent
    HeadParser --> ParseError
    HeadParser --> RequestMeta
    RequestMeta --> BodyKind
```

## Key Design Decisions

1. **Reconstructing vs. Passthrough** — The proxy parses the HTTP request, extracts metadata, then *reconstructs* the request to send to the backend. This is more complex than raw passthrough but enables future load-balancing features (header injection, URI rewriting, etc.). A simpler `handle_client1` alternative using `copy_bidirectional` exists but is unused.

2. **Incremental Parsing** — `HeadParser` does not require the entire request to arrive before parsing. It processes byte-by-byte, yielding `NeedMore` when incomplete. This enables pipelining and avoids buffering the full request unnecessarily.

3. **State Machine per Connection** — Each connection has its own state machine instance (stack-allocated in `handle_client`). This is simple and correct but means all features (parsing, connection, relaying) are in one function. Future extraction into separate modules or actors would improve testability.
