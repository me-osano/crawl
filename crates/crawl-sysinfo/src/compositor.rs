//! Compositor detection and capability querying.

use std::env;

use super::models::{CompositorCapabilities, CompositorInfo, CompositorType};

/// Detect the current compositor by checking environment variables.
///
/// Detection order:
/// 1. HYPRLAND_INSTANCE_SIGNATURE → Hyprland
/// 2. SWAYSOCK → Sway (check XDG_CURRENT_DESKTOP for Scroll variant)
/// 3. NIRI_SOCKET → Niri
/// 4. LABWC_PID → Labwc
/// 5. XDG_CURRENT_DESKTOP contains "mango" → Mango
/// 6. Fallback → Unknown
pub fn detect() -> CompositorType {
    if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        return CompositorType::Hyprland;
    }

    if env::var("SWAYSOCK").is_ok() {
        // Check for Scroll variant
        if let Ok(desktop) = env::var("XDG_CURRENT_DESKTOP") {
            if desktop.to_lowercase().contains("scroll") {
                return CompositorType::Scroll;
            }
        }
        return CompositorType::Sway;
    }

    if env::var("NIRI_SOCKET").is_ok() {
        return CompositorType::Niri;
    }

    if env::var("LABWC_PID").is_ok() {
        return CompositorType::Labwc;
    }

    if let Ok(desktop) = env::var("XDG_CURRENT_DESKTOP") {
        if desktop.to_lowercase().contains("mango") {
            return CompositorType::Mango;
        }
    }

    CompositorType::Unknown
}

/// Get capabilities for a compositor type.
pub fn get_capabilities(compositor: CompositorType) -> CompositorCapabilities {
    CompositorCapabilities::for_compositor(compositor)
}

/// Create CompositorInfo for the current compositor.
pub fn get_info() -> CompositorInfo {
    let compositor_type = detect();
    let name = compositor_type.to_string();
    let capabilities = get_capabilities(compositor_type);

    CompositorInfo {
        compositor_type,
        name,
        capabilities,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_returns_type() {
        let result = detect();
        // Should return one of the known types
        match result {
            CompositorType::Unknown
            | CompositorType::Hyprland
            | CompositorType::Sway
            | CompositorType::Niri
            | CompositorType::Mango
            | CompositorType::Labwc
            | CompositorType::Scroll => {}
        }
    }

    #[test]
    fn test_capabilities_known_compositors() {
        let caps = get_capabilities(CompositorType::Hyprland);
        assert!(caps.layer_shell);
        assert!(caps.http_ipc); // Hyprland uses HTTP IPC

        let caps = get_capabilities(CompositorType::Sway);
        assert!(caps.layer_shell);
        assert!(caps.socket_ipc); // Sway uses socket IPC
    }
}
