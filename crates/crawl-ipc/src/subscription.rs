//! Event subscription handling for crawl IPC.
//! Provides EventSubscription for receiving events from the daemon.

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::protocol::Request;

/// Handles event subscriptions to crawl-daemon.
/// Subscribes to the daemon's event broadcast and receives NDJSON events.
pub struct EventSubscription {
    socket_path: PathBuf,
}

impl EventSubscription {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }

    /// Subscribe to events, calling `handler` for each received event.
    /// The handler receives deserialized event data.
    /// This blocks until the connection is closed.
    pub async fn subscribe<F, T>(&self, mut handler: F) -> Result<()>
    where
        F: FnMut(T),
        T: DeserializeOwned,
    {
        let stream = UnixStream::connect(&self.socket_path)
            .await
            .with_context(|| format!("failed to connect to crawl at {:?}", self.socket_path))?;

        let (reader, mut writer) = tokio::io::split(stream);
        let mut reader = BufReader::new(reader);

        // Send Subscribe request
        let subscribe = Request::with_id("Subscribe", serde_json::json!({}), serde_json::json!(0));
        let req_str = serde_json::to_string(&subscribe)?;
        writer.write_all(req_str.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        // Read subscription confirmation
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        let _initial: crate::protocol::Response = serde_json::from_str(&line)
            .context("failed to parse subscription confirmation")?;

        // Listen for events
        line.clear();
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(&line) {
                Ok(value) => {
                    if let Some(params) = value.get("params") {
                        match serde_json::from_value(params.clone()) {
                            Ok(event) => handler(event),
                            Err(e) => {
                                eprintln!("WARN: failed to parse event: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("WARN: failed to parse NDJSON line: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Subscribe to events with a filter.
    /// Only events matching the predicate will be passed to the handler.
    pub async fn subscribe_filtered<F, T, P>(&self, mut handler: F, mut predicate: P) -> Result<()>
    where
        F: FnMut(T),
        T: DeserializeOwned,
        P: FnMut(&T) -> bool,
    {
        self.subscribe(|event: T| {
            if predicate(&event) {
                handler(event);
            }
        })
        .await
    }
}

/// Helper to create a subscription with a closure.
pub async fn subscribe_to<T, F>(
    socket_path: impl Into<PathBuf>,
    handler: F,
) -> Result<()>
where
    T: DeserializeOwned,
    F: FnMut(T),
{
    let sub = EventSubscription::new(socket_path);
    sub.subscribe(handler).await
}
