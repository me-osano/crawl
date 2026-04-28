//! IPC client runtime for crawl.
//! Provides CrawlClient for sending commands to the daemon.
//! For event subscriptions, use `crawl_ipc::subscription::EventSubscription`.

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::time::timeout;

use crate::protocol::{Request, Response};

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    REQUEST_ID.fetch_add(1, Ordering::SeqCst)
}

pub struct CrawlClient {
    socket_path: PathBuf,
}

impl CrawlClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }

    #[allow(dead_code)]
    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    pub async fn cmd(&self, method: &str, params: Value) -> Result<Value> {
        let id = next_id();
        let request = Request::with_id(method, serde_json::json!({"method": method}), serde_json::json!(id));
        let mut request_obj = serde_json::to_value(&request).unwrap();
        if let Some(obj) = params.as_object() {
            for (k, v) in obj {
                request_obj["params"].as_object_mut().unwrap().insert(k.clone(), v.clone());
            }
        }

        let stream = timeout(
            Duration::from_secs(5),
            UnixStream::connect(&self.socket_path)
        )
        .await
        .with_context(|| format!(
            "failed to connect to crawl daemon at {:?}\n\
             Is crawl-daemon running? Try: systemctl --user start crawl",
            self.socket_path
        ))??;

        let (reader, mut writer) = tokio::io::split(stream);
        let mut reader = BufReader::new(reader);

        let req_str = serde_json::to_string(&request_obj)?;
        writer.write_all(req_str.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        let mut line = String::new();
        timeout(
            Duration::from_secs(5),
            reader.read_line(&mut line)
        )
        .await
        .with_context(|| "request timed out")??;

        let response: Response = serde_json::from_str(&line)
            .context("failed to parse JSON-RPC response")?;

        if let Some(error) = response.error {
            anyhow::bail!("daemon error: {}", error.message);
        }

        Ok(serde_json::to_value(response).unwrap_or(serde_json::json!(null)))
    }
}
