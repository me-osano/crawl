//! Monitor/output management with scale tracking.
//!
//! Tracks Wayland outputs, their properties (dimensions, scale, transform),
//! and handles hotplug events (add/remove). Provides cleanup on monitor removal.

use std::collections::HashMap;
use tracing::{debug, info};
use wayland_client::protocol::wl_output;
use smithay_client_toolkit::output::OutputInfo;

/// Information about a monitored output.
#[derive(Debug, Clone)]
pub struct OutputInfoExt {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale: i32,
    pub transform: wl_output::Transform,
}

impl OutputInfoExt {
    /// Create from SCTK OutputInfo.
    pub fn from_sctk(info: &OutputInfo) -> Option<Self> {
        let name = info.name.clone()?;
        let (width, height) = logical_size(info);
        Some(Self {
            name,
            width,
            height,
            scale: info.scale_factor,
            transform: info.transform,
        })
    }
}

/// Tracks all known outputs and their state.
pub struct OutputManager {
    outputs: Mutex<HashMap<String, OutputInfoExt>>,
}

use std::sync::Mutex;

impl OutputManager {
    /// Create a new empty OutputManager.
    pub fn new() -> Self {
        Self {
            outputs: Mutex::new(HashMap::new()),
        }
    }

    /// Add or update an output.
    pub fn update_output(&self, info: &OutputInfo) -> Option<OutputInfoExt> {
        let ext = OutputInfoExt::from_sctk(info)?;
        let name = ext.name.clone();
        
        let mut outputs = self.outputs.lock().unwrap();
        let is_new = !outputs.contains_key(&name);
        
        if is_new {
            info!("new output: {} ({}x{} scale={})", name, ext.width, ext.height, ext.scale);
        } else {
            debug!("update output: {} ({}x{} scale={})", name, ext.width, ext.height, ext.scale);
        }
        
        outputs.insert(name.clone(), ext.clone());
        Some(ext)
    }

    /// Remove an output by name. Returns true if removed.
    pub fn remove_output(&self, name: &str) -> bool {
        let mut outputs = self.outputs.lock().unwrap();
        if outputs.remove(name).is_some() {
            info!("removed output: {}", name);
            true
        } else {
            false
        }
    }

    /// Remove an output by Wayland output object.
    pub fn remove_by_output(&self, output: &wl_output::WlOutput, output_state: &smithay_client_toolkit::output::OutputState) -> bool {
        if let Some(info) = output_state.info(output) {
            if let Some(name) = &info.name {
                return self.remove_output(name);
            }
        }
        false
    }

    /// Get output info by name.
    pub fn get(&self, name: &str) -> Option<OutputInfoExt> {
        let outputs = self.outputs.lock().unwrap();
        outputs.get(name).cloned()
    }

    /// Get all output names.
    pub fn names(&self) -> Vec<String> {
        let outputs = self.outputs.lock().unwrap();
        outputs.keys().cloned().collect()
    }

    /// Get all outputs.
    pub fn all(&self) -> Vec<OutputInfoExt> {
        let outputs = self.outputs.lock().unwrap();
        outputs.values().cloned().collect()
    }

    /// Get output dimensions for a specific output.
    pub fn dimensions(&self, name: &str) -> Option<(u32, u32)> {
        let outputs = self.outputs.lock().unwrap();
        outputs.get(name).map(|o| (o.width, o.height))
    }

    /// Get output scale factor.
    pub fn scale(&self, name: &str) -> Option<i32> {
        let outputs = self.outputs.lock().unwrap();
        outputs.get(name).map(|o| o.scale)
    }

    /// Clear all outputs.
    pub fn clear(&self) {
        let mut outputs = self.outputs.lock().unwrap();
        outputs.clear();
        debug!("all outputs cleared");
    }

    /// Number of known outputs.
    pub fn len(&self) -> usize {
        let outputs = self.outputs.lock().unwrap();
        outputs.len()
    }

    /// Check if there are no outputs.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for OutputManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate logical size from OutputInfo, accounting for transform.
pub fn logical_size(info: &OutputInfo) -> (u32, u32) {
    if let Some(mode) = info.modes.iter().find(|m| m.current) {
        let (pw, ph) = (mode.dimensions.0 as u32, mode.dimensions.1 as u32);
        match info.transform {
            wl_output::Transform::_90
            | wl_output::Transform::_270
            | wl_output::Transform::Flipped90
            | wl_output::Transform::Flipped270 => (ph, pw),
            _ => (pw, ph),
        }
    } else {
        (1920, 1080)
    }
}
