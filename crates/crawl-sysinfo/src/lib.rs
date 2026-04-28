//! crawl-sysinfo: System information aggregator.
//!
//! Provides read-only system state including:
//! - Compositor detection and capabilities
//! - OS/kernel information
//! - Host & Session information
//! - Hardware information
//! - Display/monitor information
//!
//! This crate is designed to be a **single source of truth** for system-level
//! information used by other parts of the Crawl desktop stack.

pub mod compositor;
pub mod config;
pub mod display;
pub mod hardware;
pub mod models;
pub mod os;
pub mod service;
pub mod session;

pub use models::{
    CompositorCapabilities, CompositorInfo, CompositorType, DisplayInfo, HardwareInfo, MonitorInfo,
    OsInfo, SessionInfo, SessionType, SystemInfo,
};
pub use service::SystemService;

pub fn get_info() -> SystemInfo {
    SystemService::new().get_info().clone()
}
