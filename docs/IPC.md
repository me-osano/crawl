# crawl-ipc: IPC Architecture

## Overview

`crawl-ipc` is the shared IPC (Inter-Process Communication) crate for crawl. It provides:

- **Transport layer** (Unix sockets)
- **Protocol types** (JSON-RPC 2.0)
- **Client/Server runtimes**
- **Event subscription** (pub/sub)

No system dependencies beyond `tokio` — safe to use in any crate including future QML/FFI bridges.

## Module Structure

```
crawl-ipc/src/
├── lib.rs           # Re-exports all public types
├── socket.rs        # Unix socket transport (bind, connect, IpcConnection)
├── protocol.rs      # JSON-RPC 2.0 types (Request, Response, EventMessage)
├── client.rs        # CrawlClient for sending commands
├── server.rs        # IpcServer for handling connections
├── subscription.rs  # EventSubscription for receiving events
├── error.rs         # ErrorEnvelope, CrawlError
├── events.rs        # CrawlEvent enum (all event types)
└── types.rs         # Shared data types (AudioDevice, NetInterface, etc.)
```

## Socket Transport (`socket.rs`)

Low-level Unix socket operations. No protocol knowledge.

### Key Types

```rust
pub struct IpcConnection {
    reader: BufReader<OwnedReadHalf>,
    writer: BufWriter<OwnedWriteHalf>,
}
```

### Key Functions

```rust
/// Bind a Unix socket for listening (cleans up stale sockets)
pub async fn bind_socket(path: impl AsRef<Path>) -> std::io::Result<UnixListener>

/// Connect to a Unix socket as a client
pub async fn connect_socket(path: impl AsRef<Path>) -> std::io::Result<UnixStream>

/// Resolve default socket path: $CRAWL_SOCKET > $XDG_RUNTIME_DIR/crawl.sock
pub fn default_socket_path() -> PathBuf
```

### IpcConnection Methods

```rust
impl IpcConnection {
    /// Send raw bytes with length-prefix framing
    pub async fn send(&mut self, data: &[u8]) -> std::io::Result<()>

    /// Receive length-prefixed message
    pub async fn recv(&mut self) -> std::io::Result<Vec<u8>>

    /// Convenience: send JSON-serialized message
    pub async fn send_json<T: Serialize>(&mut self, msg: &T) -> std::io::Result<()>

    /// Convenience: receive and deserialize JSON message
    pub async fn recv_json<T: DeserializeOwned>(&mut self) -> std::io::Result<T>
}
```

## Protocol Types (`protocol.rs`)

JSON-RPC 2.0 message structures.

### Request

```rust
pub struct Request {
    pub jsonrpc: String,  // "2.0"
    pub method: String,
    pub params: Value,
    pub id: Option<Value>,
}

impl Request {
    pub fn new(method: &str, params: Value) -> Self
    pub fn with_id(method: &str, params: Value, id: Value) -> Self
}
```

### Response

```rust
pub struct Response {
    pub jsonrpc: String,
    pub result: Option<Value>,
    pub error: Option<Error>,
    pub id: Option<Value>,
}

impl Response {
    pub fn success(id: Option<Value>, result: Value) -> Self
    pub fn error(id: Option<Value>, code: i32, message: &str) -> Self
}
```

### EventMessage

Used for NDJSON event subscription format:

```rust
pub struct EventMessage {
    pub jsonrpc: String,  // "2.0"
    pub method: String,  // "event"
    pub params: Value,
}

impl EventMessage {
    pub fn event(params: Value) -> Self
}
```

### Error Codes

```rust
pub mod error_code {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const APP_BASE: i32 = -32000;  // Application errors: -32000 to -32099
}
```

## Client Runtime (`client.rs`)

`CrawlClient` for sending commands to the daemon.

```rust
pub struct CrawlClient {
    socket_path: PathBuf,
}

impl CrawlClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self
    pub fn socket_path(&self) -> &PathBuf
    pub async fn cmd(&self, method: &str, params: Value) -> Result<Value>
}
```

### Usage

```rust
use crawl_ipc::CrawlClient;

let client = CrawlClient::new("/run/user/1000/crawl.sock");
let response = client.cmd("Ping", serde_json::json!({})).await?;
```

## Server Runtime (`server.rs`)

`IpcServer` handles JSON-RPC connections and event subscriptions.

```rust
pub struct IpcServer {
    socket_path: PathBuf,
    event_tx: EventSender,
    dispatcher: Option<RequestDispatcher>,
}

impl IpcServer {
    pub fn new(socket_path: PathBuf, event_tx: EventSender) -> Self
    pub fn set_dispatcher(&mut self, dispatcher: RequestDispatcher)
    pub fn event_sender(&self) -> EventSender
    pub async fn run(&self) -> std::io::Result<()>
}
```

### RequestDispatcher

Inject business logic via a dispatcher function:

```rust
pub type RequestDispatcher = Arc<dyn Fn(
    String,                          // method
    serde_json::Value,               // params
    Option<serde_json::Value>         // id
) -> Pin<Box<dyn Future<Output = Response> + Send>> + Send + Sync>;
```

### Usage in crawl-daemon

```rust
use crawl_ipc::{IpcServer, RequestDispatcher, EventSender};

let (event_tx, _rx) = broadcast::channel(100);
let mut server = IpcServer::new(socket_path, event_tx);

// Inject dispatcher
let dispatcher: RequestDispatcher = Arc::new(|method, params, id| {
    Box::pin(async move {
        // Handle command...
        Response::success(id, serde_json::json!({ "ok": true }))
    })
});
server.set_dispatcher(dispatcher);

// Run (blocks)
server.run().await?;
```

## Event Subscription (`subscription.rs`)

`EventSubscription` for receiving events from the daemon.

```rust
pub struct EventSubscription {
    socket_path: PathBuf,
}

impl EventSubscription {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self
    pub async fn subscribe<F, T>(&self, handler: F) -> Result<()>
    pub async fn subscribe_filtered<F, T, P>(&self, handler: F, predicate: P) -> Result<()>
}
```

### Usage

```rust
use crawl_ipc::EventSubscription;

let sub = EventSubscription::new("/run/user/1000/crawl.sock");
sub.subscribe(|event: CrawlEvent| {
    println!("Event: {:?}", event);
}).await?;
```

## Socket Path Resolution

The `default_socket_path()` function follows this priority:

1. `$CRAWL_SOCKET` environment variable
2. `$XDG_RUNTIME_DIR/crawl.sock` (e.g., `/run/user/1000/crawl.sock`)
3. `/run/user/<uid>/crawl.sock` (read from `/proc/self/uid` on Linux)
4. `/tmp/crawl.sock` (fallback, not recommended for production)

## NDJSON Protocol

The daemon uses NDJSON (Newline-Delimited JSON) for both requests and events:

### Request

```json
{"jsonrpc":"2.0", "method":"Ping", "params":{"method":"Ping"}, "id":1}
```

### Response

```json
{"jsonrpc":"2.0", "result":{"time_ms":1234567890}, "id":1}
```

### Event (after Subscribe)

```json
{"jsonrpc":"2.0", "method":"event", "params":{"WallpaperChanged":{...}}}
```

## Error Handling

JSON-RPC errors follow the standard format:

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32600,
    "message": "Invalid JSON-RPC request"
  },
  "id": null
}
```

Application errors use codes from -32000 to -32099.

## Integration with crawl-daemon

The daemon should:

1. **Own the IPC server lifecycle**
2. **Inject business logic via `RequestDispatcher`**
3. **Not implement transport logic** (that's in `crawl-ipc`)

Example structure for `crawl-daemon`:

```
crawl-daemon/
├── src/
│   ├── main.rs          # Entry point
│   ├── daemon.rs        # Daemon orchestrator
│   ├── config.rs        # Config loading
│   ├── dispatcher.rs    # Command dispatch (implements RequestDispatcher)
│   ├── services/        # Service modules
│   │   ├── audio.rs
│   │   ├── bluetooth.rs
│   │   ├── network.rs
│   │   └── ...
│   └── state.rs         # Shared state
```

See [DAEMON.md](DAEMON.md) for daemon-specific documentation.

## Future: Length-Prefixed Framing

Currently using NDJSON for debugging ergonomics. For production, consider switching to length-prefixed framing:

```
[4-byte length (big-endian)][JSON payload]
```

Benefits:
- Handles binary data
- More robust (no newline issues)
- Better for streaming

This can be implemented in `socket.rs` without changing the protocol types.
