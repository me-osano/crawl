//! Bluetooth configuration types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Power the adapter on startup if true
    pub auto_enable: bool,
    /// Scan timeout in seconds (0 = no timeout)
    pub scan_timeout_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self { auto_enable: false, scan_timeout_secs: 30 }
    }
}
