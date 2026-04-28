//! Processes configuration types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default sort field: cpu | mem | pid | name
    #[serde(default = "default_sort")]
    pub sort_by: String,
    /// Default number of top processes to return
    pub top: usize,
    /// Include command line in process info (expensive)
    pub include_cmd: bool,
    /// Interval for top-N tracking in ms
    pub top_interval_ms: u64,
    /// Interval for full scan in ms
    pub full_interval_ms: u64,
}

fn default_sort() -> String {
    "cpu".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sort_by: "cpu".into(),
            top: 20,
            include_cmd: false,
            top_interval_ms: 1000,
            full_interval_ms: 5000,
        }
    }
}