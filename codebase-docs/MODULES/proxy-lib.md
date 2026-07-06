# Module: `proxy/src/lib.rs`

## Responsibility

Provides the incremental HTTP request-line and header parser. Transforms raw byte streams into structured `RequestMeta` objects.

## Key Exports

### `HeadParser`
Stateful, incremental HTTP header parser.

```rust
let mut parser = HeadParser::new();
parser.consume(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
// Can also work incrementally:
// parser.consume(b"GET / HT");
// parser.consume(b"TP/1.1\r\nHo");
// ...
```

**Fields:**
- `cursor: usize` — Position in the buffer up to which we've parsed.
- `lines: Vec<Vec<u8>>` — Accumulated header lines (without `\r\n`).

**Methods:**
- `new()` — Creates an empty parser.
- `advance(buf: &[u8]) -> ParseEvent` — Processes the next line from the buffer. Returns `Line`, `End`, or `NeedMore`. Stores each header line internally.
- `parse_request_meta() -> Result<RequestMeta, ParseError>` — Parses the stored lines into structured metadata. Must be called after `advance` returns `End`.

### `ParseEvent<'a>`
```rust
pub enum ParseEvent<'a> {
    Line(&'a [u8]),   // A complete header line (without \r\n)
    End,              // Empty line found — headers are complete
    NeedMore,         // No complete line available yet
}
```

### `RequestMeta`
```rust
pub struct RequestMeta {
    pub method: Vec<u8>,         // e.g., "GET"
    pub uri: Vec<u8>,            // e.g., "/index.html"
    pub http_version: Vec<u8>,   // e.g., "HTTP/1.1"
    pub host: Option<Vec<u8>>,   // From Host: header
    pub body_kind: BodyKind,     // How to handle the body
    pub connection_close: bool,  // Connection: close present?
}
```

### `BodyKind`
```rust
pub enum BodyKind {
    None,                    // No body (GET, HEAD, etc.)
    ContentLength(usize),    // Content-Length: N
    Chunked,                 // Transfer-Encoding: chunked
}
```

### `ParseError`
Currently single variant.

## How Parsing Works

1. Caller accumulates bytes in a buffer and repeatedly calls `advance(buf)`.
2. `advance` scans from `cursor` for `\r\n`:
   - If found at position 0 → returns `End` (empty line, headers done).
   - If found at position N → extracts the line, stores it in `lines`, advances `cursor`, returns `Line`.
   - If not found → returns `NeedMore`.
3. Once `End` is returned, caller calls `parse_request_meta()` which:
   - Splits `lines[0]` on spaces into `[method, uri, version]`.
   - Iterates remaining lines looking for `Content-Length`, `Transfer-Encoding: chunked`, `Connection: close`, and `Host`.
   - Returns a `RequestMeta`.

## Dependencies

| Dependency | Purpose |
|------------|---------|
| `std::collections::HashMap` | Imported but **unused** (leftover or future use) |

## Edge Cases & Limitations

- **No header value folding**: HTTP/1.0 allowed multiline header values (continuation lines starting with space/tab). This parser doesn't handle that — every header must fit in a single `\r\n`-delimited line.
- **No duplicate header merging**: If `Content-Length` appears twice, the last one wins. RFC 7230 says these should either be merged or rejected.
- **Case sensitivity**: Header names are compared case-insensitively (good). Header values like `chunked` and `close` are compared case-insensitively too (good). But some proxies/browsers send `Connection: keep-alive` which is currently ignored.
- **URI not decoded**: The raw URI bytes are stored as-is. No percent-decoding or normalization.
- **No validation**: Malformed request lines (e.g., fewer than 3 parts) return `InvalidHeader` but with no detail about what was wrong.
