use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type CrawlResult<T> = Result<T, CrawlError>;

#[derive(Debug, Error)]
pub enum CrawlError {
    #[error("bluetooth error: {0}")]
    Bluetooth(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("notification error: {0}")]
    Notification(String),
    #[error("clipboard error: {0}")]
    Clipboard(String),
    #[error("sysmon error: {0}")]
    Sysmon(String),
    #[error("brightness error: {0}")]
    Brightness(String),
    #[error("process error: {0}")]
    Process(String),
    #[error("media error: {0}")]
    Media(String),
    #[error("power error: {0}")]
    Power(String),
    #[error("disk error: {0}")]
    Disk(String),
    #[error("audio error: {0}")]
    Audio(String),
    #[error("D-Bus error: {0}")]
    DBus(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("internal error: {0}")]
    Internal(String),
}

/// Standard JSON error envelope returned by all API endpoints on failure.
///
/// Example:
/// ```json
/// { "error": { "domain": "bluetooth", "code": "not_powered", "message": "Adapter is off" } }
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorBody {
    pub domain: String,
    pub code: String,
    pub message: String,
}

impl ErrorEnvelope {
    pub fn new(
        domain: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            error: ErrorBody {
                domain: domain.into(),
                code: code.into(),
                message: message.into(),
            },
        }
    }
}

impl From<CrawlError> for ErrorEnvelope {
    fn from(e: CrawlError) -> Self {
        let (domain, code) = match &e {
            CrawlError::Bluetooth(_) => ("bluetooth", "bluetooth_error"),
            CrawlError::Network(_) => ("network", "network_error"),
            CrawlError::Notification(_) => ("notify", "notification_error"),
            CrawlError::Clipboard(_) => ("clipboard", "clipboard_error"),
            CrawlError::Sysmon(_) => ("sysmon", "sysmon_error"),
            CrawlError::Brightness(_) => ("brightness", "brightness_error"),
            CrawlError::Process(_) => ("proc", "process_error"),
            CrawlError::Media(_) => ("media", "media_error"),
            CrawlError::Power(_) => ("power", "power_error"),
            CrawlError::Disk(_) => ("disk", "disk_error"),
            CrawlError::Audio(_) => ("audio", "audio_error"),
            CrawlError::DBus(_) => ("dbus", "dbus_error"),
            CrawlError::NotFound(_) => ("crawl", "not_found"),
            CrawlError::PermissionDenied(_) => ("crawl", "permission_denied"),
            CrawlError::Internal(_) => ("crawl", "internal_error"),
        };
        Self::new(domain, code, e.to_string())
    }
}
