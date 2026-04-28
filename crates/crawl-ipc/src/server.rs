//! IPC server runtime for crawl.
//! Provides the server lifecycle (bind, accept, connection handling).
//! Business logic / command dispatch is injected via a dispatcher function.

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::broadcast;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info};

use crate::protocol::{Request, Response, EventMessage};
use crate::socket::bind_socket;

pub type EventSender = broadcast::Sender<crate::CrawlEvent>;
pub type EventReceiver = broadcast::Receiver<crate::CrawlEvent>;

/// Dispatcher function type: given (method, params, id), return a Response.
pub type RequestDispatcher = Arc<dyn Fn(String, serde_json::Value, Option<serde_json::Value>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>> + Send + Sync>;

/// IPC Server that handles JSON-RPC requests and event subscriptions.
pub struct IpcServer {
    socket_path: PathBuf,
    event_tx: EventSender,
    dispatcher: Option<RequestDispatcher>,
}

impl IpcServer {
    pub fn new(socket_path: PathBuf, event_tx: EventSender) -> Self {
        Self { socket_path, event_tx, dispatcher: None }
    }

    /// Set the request dispatcher (called by crawl-daemon).
    pub fn set_dispatcher(&mut self, dispatcher: RequestDispatcher) {
        self.dispatcher = Some(dispatcher);
    }

    pub fn event_sender(&self) -> EventSender {
        self.event_tx.clone()
    }

    /// Run the server loop (blocks until error).
    pub async fn run(&self) -> std::io::Result<()> {
        let listener = bind_socket(&self.socket_path).await?;
        info!("IPC server listening on {:?}", self.socket_path);

        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let event_rx = self.event_tx.subscribe();
                    let dispatcher = self.dispatcher.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, event_rx, dispatcher).await {
                            error!("connection error: {}", e);
                        }
                    });
                }
                Err(e) => error!("accept error: {}", e),
            }
        }
    }
}

/// Handle a single client connection (NDJSON protocol).
async fn handle_connection(
    stream: tokio::net::UnixStream,
    mut event_rx: EventReceiver,
    dispatcher: Option<RequestDispatcher>,
) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut subscribed = false;

    loop {
        tokio::select! {
            result = reader.read_line(&mut line) => {
                let n = match result { Ok(n) => n, Err(_) => break };
                if n == 0 { break; }
                let trimmed = line.trim();
                if trimmed.is_empty() { line.clear(); continue; }

                // Parse as JSON-RPC request
                let req: Request = match serde_json::from_str(trimmed) {
                    Ok(r) => r,
                    Err(_) => {
                        let resp = Response::error(None, -32600, "Invalid JSON-RPC request");
                        send_response(&mut writer, &resp).await?;
                        line.clear();
                        continue;
                    }
                };

                // Handle Subscribe specially (event subscription mode)
                if req.method == "Subscribe" {
                    subscribed = true;
                    let resp = Response::success(
                        req.id,
                        serde_json::json!({"subscribed": true, "time_ms": crate::protocol::now_ms()}),
                    );
                    send_response(&mut writer, &resp).await?;
                    line.clear();
                    continue;
                }

                // Dispatch to handler if available
                if let Some(ref dispatch) = dispatcher {
                    let resp = dispatch(req.method, req.params, req.id).await;
                    send_response(&mut writer, &resp).await?;
                }

                line.clear();
            }
            _ = async {
                if subscribed {
                    let _ = event_rx.recv().await;
                }
            } => {}
        }

        if subscribed {
            while let Ok(evt) = event_rx.try_recv() {
                let event_json = EventMessage::event(serde_json::to_value(evt).unwrap_or_default());
                let mut response = serde_json::to_string(&event_json).unwrap();
                response.push('\n');
                writer.write_all(response.as_bytes()).await?;
                writer.flush().await?;
            }
        }
    }
    Ok(())
}

async fn send_response(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    resp: &Response,
) -> std::io::Result<()> {
    let mut response = serde_json::to_string(resp).unwrap();
    response.push('\n');
    writer.write_all(response.as_bytes()).await?;
    writer.flush().await
}
