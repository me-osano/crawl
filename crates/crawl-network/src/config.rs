//! Network configuration types.

use crate::HotspotBackend;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Power the adapter on startup if true
    pub auto_enable: bool,
    ///
    pub wifi_scan_on_start: bool,
    ///
    pub wifi_scan_finish_delay_ms: u64,
    ///
    #[serde(default)]
    pub hotspot_backend: Option<HotspotBackend>,
    #[serde(default = "default_true")]
    pub hotspot_virtual_iface: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_enable: false,
            wifi_scan_on_start: false,
            wifi_scan_finish_delay_ms: 30000,
            hotspot_backend: Some(HotspotBackend::default()),
            hotspot_virtual_iface: true,
        }
    }
}