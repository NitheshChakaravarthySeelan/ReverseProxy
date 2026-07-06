# Findings & Recommendations

| ID | Severity | Category | Finding | Location | Recommendation |
|----|----------|----------|---------|----------|---------------|
| F1 | HIGH | Bug | All request headers except Host and Content-Length are silently dropped when reconstructing | `proxy/src/main.rs:117-138` | Forward all original headers, or at minimum preserve Cookie, Authorization, Accept, User-Agent |
| F2 | HIGH | Completeness | Chunked transfer-encoding body reading is stubbed with "not yet implemented" — will close the connection | `proxy/src/main.rs:99-102` | Implement chunked body parser (read chunk-size, chunk-data, trailing CRLF) |
| F3 | HIGH | Completeness | Keep-alive decision always closes the connection, breaking HTTP/1.1 persistent connections | `proxy/src/main.rs:164-168` | Check `connection_close` in `RequestMeta` and loop back to `ReadingHeaders` if connection should persist |
| F4 | MEDIUM | Complexity | `handle_client` is a single 180-line async function with nested state machine, IO, and parsing logic | `proxy/src/main.rs:23-178` | Extract state transitions into separate async functions or a trait-based state machine |
| F5 | MEDIUM | Configuration | Backend address is hardcoded as `127.0.0.1:81` | `proxy/src/main.rs:111` | Read from CLI args, environment variable, or a config file |
| F6 | MEDIUM | Robustness | No timeouts on backend connection or response read — a slow/malicious backend hangs the client forever | `proxy/src/main.rs:111,149` | Add `tokio::time::timeout` around backend operations |
| F7 | LOW | Consistency | Two proxying approaches exist: the state machine (`handle_client`) and raw copy (`handle_client1`). Only one is used. | `proxy/src/main.rs:23,182` | Remove `handle_client1` or use it as a fallback for non-HTTP traffic |
| F8 | LOW | Code Quality | `HashMap` is imported in `lib.rs` but never used | `proxy/src/lib.rs:1` | Remove unused import |
| F9 | LOW | Code Quality | `client/src/main.rs` defines a `client()` function and a `write_to_server()` function, but `main()` is empty | `client/src/main.rs:39` | Either call `client()` from `main()` or remove dead code |
| F10 | LOW | Completeness | `check_valid::buffer_window_end()` is defined but never called anywhere | `proxy/src/check_valid.rs:4-8` | Remove or use it |
| F11 | LOW | Testing | No tests exist anywhere in the workspace | Entire project | Add unit tests for `HeadParser` and integration tests for the proxy |
