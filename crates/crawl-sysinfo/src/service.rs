//! System information service.

use super::models::SystemInfo;
use super::{compositor, display, hardware, os, session};

/// System information service.
///
/// Collects and provides read-only system state.
/// This should be queried at startup and cached - don't poll continuously.
#[derive(Clone)]
pub struct SystemService {
    snapshot: SystemInfo,
}

impl SystemService {
    /// Create a new SystemService with an initial snapshot.
    pub fn new() -> Self {
        Self {
            snapshot: Self::collect(),
        }
    }

    /// Get the current system information snapshot.
    pub fn get_info(&self) -> &SystemInfo {
        &self.snapshot
    }

    /// Get compositor info.
    pub fn compositor(&self) -> &super::models::CompositorInfo {
        &self.snapshot.compositor
    }

    /// Get OS info.
    pub fn os(&self) -> &super::models::OsInfo {
        &self.snapshot.os
    }

    /// Get session info.
    pub fn session(&self) -> &super::models::SessionInfo {
        &self.snapshot.session
    }

    /// Get hardware info.
    pub fn hardware(&self) -> &super::models::HardwareInfo {
        &self.snapshot.hardware
    }

    /// Get display info.
    pub fn display(&self) -> &super::models::DisplayInfo {
        &self.snapshot.display
    }

    /// Collect system information.
    fn collect() -> SystemInfo {
        SystemInfo {
            compositor: compositor::get_info(),
            os: os::get_info(),
            session: session::get_info(),
            hardware: hardware::get_info(),
            display: display::get_info(),
        }
    }

    /// Refresh the system snapshot.
    ///
    /// Note: Most data is static. Only display info may change (hotplug).
    pub fn refresh(&mut self) {
        self.snapshot = Self::collect();
    }

    /// Refresh display info only (for monitor hotplug events).
    pub fn refresh_display(&mut self) {
        self.snapshot.display = display::get_info();
    }
}

impl Default for SystemService {
    fn default() -> Self {
        Self::new()
    }
}
