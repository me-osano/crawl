//! crawl-clipboard: Wayland clipboard access via wl-clipboard-rs.
//!
//! Watches both the clipboard and primary selections for changes,
//! maintains a bounded history, and broadcasts ClipboardEvents.
//!
//! Note: Requires a running Wayland session (WAYLAND_DISPLAY must be set).

use crawl_ipc::{
    events::{ClipboardEvent, CrawlEvent},
    types::ClipEntry,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};
use wl_clipboard_rs::paste::{get_contents, ClipboardType, Error as PasteError, MimeType, Seat};

// ── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Maximum number of clipboard history entries to retain
    pub history_size: usize,
    /// Also watch the primary selection (middle-click paste)
    pub watch_primary: bool,
    /// Polling interval in milliseconds (wl-clipboard-rs watch mode)
    pub poll_interval_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            history_size: 50,
            watch_primary: false,
            poll_interval_ms: 500,
        }
    }
}

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ClipboardError {
    #[error("Wayland display unavailable (WAYLAND_DISPLAY not set?)")]
    NoDisplay,
    #[error("paste error: {0}")]
    Paste(String),
    #[error("copy error: {0}")]
    Copy(String),
}

// ── History store ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ClipHistory {
    inner: Arc<Mutex<VecDeque<ClipEntry>>>,
    capacity: usize,
}

impl ClipHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    pub fn push(&self, entry: ClipEntry) {
        let mut inner = self.inner.lock().unwrap();
        // Deduplicate: don't re-push identical content
        if inner.front().map(|e| e.content == entry.content).unwrap_or(false) {
            return;
        }
        if inner.len() >= self.capacity {
            inner.pop_back();
        }
        inner.push_front(entry);
    }

    pub fn list(&self) -> Vec<ClipEntry> {
        self.inner.lock().unwrap().iter().cloned().collect()
    }

    pub fn latest(&self) -> Option<ClipEntry> {
        self.inner.lock().unwrap().front().cloned()
    }
}

// ── Domain runner ─────────────────────────────────────────────────────────────

pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    info!("crawl-clipboard starting (history={}, primary={})", cfg.history_size, cfg.watch_primary);

    if std::env::var("WAYLAND_DISPLAY").is_err() {
        warn!("WAYLAND_DISPLAY not set — clipboard domain will be inactive");
        std::future::pending::<()>().await;
        return Ok(());
    }

    let history = ClipHistory::new(cfg.history_size);
    let interval = std::time::Duration::from_millis(cfg.poll_interval_ms);
    let mut last_content = String::new();

    loop {
        tokio::time::sleep(interval).await;

        // Poll clipboard content
        // wl-clipboard-rs is synchronous; run in a blocking thread
        let result = tokio::task::spawn_blocking(|| {
            get_contents(ClipboardType::Regular, Seat::Unspecified, MimeType::Text)
        }).await;

        match result {
            Ok(Ok((mut reader, _mime))) => {
                let mut content = String::new();
                use std::io::Read;
                if reader.read_to_string(&mut content).is_ok() && content != last_content {
                    last_content = content.clone();
                    let entry = ClipEntry {
                        content,
                        mime: "text/plain".into(),
                        timestamp_ms: now_ms(),
                    };
                    debug!("clipboard changed ({} bytes)", entry.content.len());
                    history.push(entry.clone());
                    let _ = tx.send(CrawlEvent::Clipboard(ClipboardEvent::Changed { entry }));
                }
            }
            Ok(Err(PasteError::NoSeats)) | Ok(Err(PasteError::ClipboardEmpty)) => {
                // Normal — clipboard is empty or compositor has no seats yet
            }
            Ok(Err(e)) => {
                debug!("clipboard read: {e}");
            }
            Err(e) => {
                warn!("clipboard task panicked: {e}");
            }
        }
    }
}

// ── Public query API ──────────────────────────────────────────────────────────

/// Read current clipboard content synchronously.
pub fn get_clipboard() -> Result<Option<ClipEntry>, ClipboardError> {
    use std::io::Read;
    match get_contents(ClipboardType::Regular, Seat::Unspecified, MimeType::Text) {
        Ok((mut reader, mime)) => {
            let mut content = String::new();
            reader.read_to_string(&mut content)
                .map_err(|e| ClipboardError::Paste(e.to_string()))?;
            Ok(Some(ClipEntry { content, mime, timestamp_ms: now_ms() }))
        }
        Err(PasteError::ClipboardEmpty) | Err(PasteError::NoSeats) => Ok(None),
        Err(e) => Err(ClipboardError::Paste(e.to_string())),
    }
}

/// Write text to the clipboard.
pub async fn set_clipboard(text: String) -> Result<(), ClipboardError> {
    use wl_clipboard_rs::copy::{MimeType as CopyMime, Options, Source};
    tokio::task::spawn_blocking(move || {
        let opts = Options::new();
        opts.copy(
            Source::Bytes(text.into_bytes().into()),
            CopyMime::Autodetect,
        ).map_err(|e| ClipboardError::Copy(e.to_string()))
    })
    .await
    .map_err(|e| ClipboardError::Copy(e.to_string()))?
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
