Phase 1: 
We are writing 
  1. A TCP listener
  2. A per-connection handler
  3. A bidirectional byte pipe.

client -> TCP -> proxy -> TCP -> backend
first we will be doing: 
  - std only 
  - blocking``
  - one thread per connection 
  - later move to tokito, and hyper

We must code out the client for tcp

1. ConnectionState Enum
  - We introduce this to explicitly define and track the different phases of a client connection.
    - Idle: Represents a connection that is open but not currently processing a request. While present in the enum, our current handle_client starts directly in ReadingHeaders.
    - ReadingHeaders: The initial and primary state where the proxy is actively reading incoming bytes from the client and attempting to parse the HTTP request headers.
    - ReadingBody (RequestMeta): Once the headers are successfully parsed and indicate the presence of a request body. (eg. via Content-length or transfer-encoding: chunked), the connection transition to this state. It carries the RequestMeta struct, which contains all the parsed information about the request.
    - WritingBackend: This is a placeholder state for when the proxy needs to establish or retrieve a connection to a suitable backend server to forward the client's request.
    - RelayingRequest: This is a placeholder state for when the proxy is forwarding the backend's response back to the client.
    - KeepAliveDecision: A placeholder state where the proxy will decide whether the client conection can be reused for subsequent requests or if it should be closed. 
    - Closed: The final state, indicating that the connection has been terminated.

2. HeadParser Modifications and RequestMeta: 
  - HeadParser: It collects all raw header lines into its pub lines: Vec<Vec<u8>> field. This allows for all comprehensive analysis of all header line once they are fully received.
  - ParseEvent: 
  - BodyKind: A enum which is a critical output of header parsing. It explicitly defines how request body should be handled.
    - None: No request body.
    - ContentLength(usize): The request body has a fixed length, specified by the Content-length header. The usize indicate the length.
    - Chunked: The request body uses chunked transfer encoding, indicated by the transfer-encoding: chunked
  - RequestMeta: This struct encapsulates the essential metadata extracted from the parsed headers. It includes
    - body_kind: BodyKind
    - connection_close: bool 
    - host:
    - method: 
    - uri: 
    - http_version
