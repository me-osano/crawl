//! Unix socket transport layer for crawl IPC.
//! No protocol knowledge — only byte-stream transport.

use tokio::net::{UnixListener, UnixStream};
use tokio::io::{AsyncWriteExt, AsyncReadExt, BufReader, BufWriter};
use std::path::{Path, PathBuf};

/// Bind a Unix socket for listening, cleaning up stale sockets.
pub async fn bind_socket(path: impl AsRef<Path>) -> std::io::Result<UnixListener> {
    let path = path.as_ref();
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    UnixListener::bind(path)
}

/// Connect to a Unix socket as a client.
pub async fn connect_socket(path: impl AsRef<Path>) -> std::io::Result<UnixStream> {
    UnixStream::connect(path).await
}

/// Resolve the default socket path for the crawl daemon.
/// Checks CRAWL_SOCKET env var, then XDG_RUNTIME_DIR.
pub fn default_socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("CRAWL_SOCKET") {
        return PathBuf::from(path);
    }
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("crawl.sock");
    }
    // Fallback: try /run/user/<uid> using /proc/self/uid if available
    #[cfg(target_os = "linux")]
    {
        if let Ok(s) = std::fs::read_to_string("/proc/self/uid") {
            if let Ok(uid) = s.trim().parse::<u32>() {
                return PathBuf::from(format!("/run/user/{}/crawl.sock", uid));
            }
        }
    }
    PathBuf::from("/tmp/crawl.sock")
}

/// A framed IPC connection over Unix socket with length-prefixed JSON.
pub struct IpcConnection {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: BufWriter<tokio::net::unix::OwnedWriteHalf>,
}

impl IpcConnection {
    pub fn new(stream: UnixStream) -> Self {
        let (read, write) = stream.into_split();
        Self {
            reader: BufReader::new(read),
            writer: BufWriter::new(write),
        }
    }

    /// Send a message with length-prefix framing.
    pub async fn send(&mut self, data: &[u8]) -> std::io::Result<()> {
        let len = (data.len() as u32).to_be_bytes();
        self.writer.write_all(&len).await?;
        self.writer.write_all(data).await?;
        self.writer.flush().await
    }

    /// Receive a length-prefixed message.
    pub async fn recv(&mut self) -> std::io::Result<Vec<u8>> {
        let mut len_buf = [0u8; 4];
        self.reader.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        self.reader.read_exact(&mut buf).await?;
        Ok(buf)
    }

    /// Send a JSON message (convenience for JSON-RPC).
    pub async fn send_json<T: serde::Serialize>(&mut self, msg: &T) -> std::io::Result<()> {
        let data = serde_json::to_vec(msg)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.send(&data).await
    }

    /// Receive and deserialize a JSON message.
    pub async fn recv_json<T: for<'de> serde::Deserialize<'de>>(&mut self) -> std::io::Result<T> {
        let data = self.recv().await?;
        serde_json::from_slice(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Get reference to underlying stream for peer credentials (Linux).
    #[cfg(target_os = "linux")]
    pub fn peer_credentials(&self) -> std::io::Result<tokio::net::unix::UCred> {
        // This requires access to the underlying stream
        // For now, return an error
        Err(std::io::Error::new(std::io::ErrorKind::Other, "peer_credentials not implemented"))
    }
}

/// Accept loop helper (transport only — no dispatch logic).
pub async fn accept_loop(
    listener: &UnixListener,
) -> std::io::Result<(IpcConnection, tokio::net::unix::SocketAddr)> {
    let (stream, addr) = listener.accept().await?;
    Ok((IpcConnection::new(stream), addr))
}
