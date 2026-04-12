use anyhow::{Context, Result};
use http_body_util::Full;
use hyper::{body::Bytes, Method, Request};
use hyper_util::rt::TokioIo;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::path::PathBuf;
use tokio::net::UnixStream;

/// Thin HTTP client that speaks to crawl-daemon over a Unix socket.
pub struct CrawlClient {
    socket_path: PathBuf,
}

impl CrawlClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self { socket_path: socket_path.into() }
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    async fn connect(&self) -> Result<hyper::client::conn::http1::SendRequest<Full<Bytes>>> {
        let stream = UnixStream::connect(&self.socket_path)
            .await
            .with_context(|| format!(
                "failed to connect to crawl daemon at {:?}\n\
                 Is crawl-daemon running? Try: systemctl --user start crawl",
                self.socket_path
            ))?;

        let io = TokioIo::new(stream);
        let (sender, conn) = hyper::client::conn::http1::handshake(io).await?;
        tokio::spawn(conn);
        Ok(sender)
    }

    pub async fn get(&self, path: &str) -> Result<Value> {
        self.request(Method::GET, path, None).await
    }

    pub async fn post(&self, path: &str, body: Value) -> Result<Value> {
        self.request(Method::POST, path, Some(body)).await
    }

    pub async fn delete(&self, path: &str) -> Result<Value> {
        self.request(Method::DELETE, path, None).await
    }

    #[allow(dead_code)]
    pub async fn get_typed<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let val = self.get(path).await?;
        serde_json::from_value(val).context("failed to deserialize response")
    }

    async fn request(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        use http_body_util::{BodyExt, Full};

        let mut sender = self.connect().await?;

        let body_bytes = match body {
            Some(v) => Bytes::from(serde_json::to_vec(&v)?),
            None    => Bytes::new(),
        };

        let req = Request::builder()
            .method(method)
            .uri(format!("http://localhost{path}"))
            .header("content-type", "application/json")
            .body(Full::new(body_bytes))?;

        let res = sender.send_request(req).await?;
        let status = res.status();
        let bytes = res.into_body().collect().await?.to_bytes();
        let val: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);

        if !status.is_success() {
            let msg = val["error"]["message"]
                .as_str()
                .unwrap_or("unknown error")
                .to_string();
            anyhow::bail!("daemon error ({}): {}", status, msg);
        }

        Ok(val)
    }
}
