//! Processes configuration types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Polling interval in milliseconds
    pub poll_interval_ms: u64,
    /// Publish a CpuSpike event when aggregate exceeds this percent
    pub cpu_spike_threshold: f32,
    /// Publish a MemPressure event when usage exceeds this percent
    pub mem_pressure_threshold: f32,
    /// Minimum change in CPU % to trigger update
    pub cpu_change_threshold: f32,
    /// Minimum change in memory % to trigger update
    pub mem_change_threshold: f32,
    /// Minimum change in network bytes to trigger update
    pub net_change_threshold: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            poll_interval_ms: 1000,
            cpu_spike_threshold: 90.0,
            mem_pressure_threshold: 85.0,
            cpu_change_threshold: 2.0,
            mem_change_threshold: 1.0,
            net_change_threshold: 1024,
        }
    }
}