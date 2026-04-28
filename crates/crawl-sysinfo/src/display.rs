//! Display/monitor information gathering.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use super::models::{DisplayInfo, MonitorInfo};

/// Get display information.
pub fn get_info() -> DisplayInfo {
    let mut monitors = get_monitors_from_drm();
    let scales = get_scales();

    // If wlr-randr returned scales, merge them into monitor info
    for scale_info in &scales {
        if let Some(mon) = monitors.iter_mut().find(|m| m.name == scale_info.0) {
            mon.scale = scale_info.1;
        }
    }

    let scales_map: HashMap<String, f32> = scales.into_iter().collect();

    DisplayInfo {
        monitors,
        scales: scales_map,
    }
}

/// Get list of monitors from DRM sysfs.
fn get_monitors_from_drm() -> Vec<MonitorInfo> {
    let mut monitors = Vec::new();

    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip card* entries without a dash (these are cards, not connectors)
            if name.starts_with("card") && !name.contains('-') {
                continue;
            }

            let path = entry.path();
            let status_path = path.join("status");
            let status = fs::read_to_string(&status_path)
                .ok()
                .map(|s| s.trim().to_string())
                .unwrap_or_default();

            let active = status == "connected";

            if active {
                let (width, height) = get_monitor_dimensions(&path);
                let (x, y) = get_monitor_position(&path);
                let scale = get_monitor_scale(&path);

                monitors.push(MonitorInfo {
                    name: name.clone(),
                    width,
                    height,
                    x,
                    y,
                    scale,
                    refresh_rate: 60.0,
                    focused: false,
                    active,
                });
            }
        }
    }

    monitors
}

/// Get display scales from wlr-randr.
fn get_scales() -> Vec<(String, f32)> {
    let output = Command::new("wlr-randr").arg("--json").output();

    match output {
        Ok(out) if out.status.success() => parse_wlr_scales(&String::from_utf8_lossy(&out.stdout)),
        _ => Vec::new(),
    }
}

/// Parse wlr-randr JSON for scales.
fn parse_wlr_scales(json: &str) -> Vec<(String, f32)> {
    let mut scales = Vec::new();
    let json = json.trim();

    if !json.is_empty() && json.starts_with('[') {
        // Simple approach: look for name/scale pairs
        // Format: {"name": "DP-1", "scale": 1.5, ...}
        let mut search = json;
        while let Some(name_start) = find_key_position(search, "name") {
            if let Some((name, after_name)) = extract_string_value(search, name_start) {
                if let Some(scale_pos) = find_key_position(&search[after_name..], "scale") {
                    if let Some((scale_val, new_pos)) =
                        extract_number_value(&search[after_name + scale_pos..])
                    {
                        if scale_val > 0.0 {
                            scales.push((name, scale_val as f32));
                        }
                        search = &search[after_name + new_pos..];
                    } else {
                        search = &search[after_name..];
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    scales
}

/// Find position of "key": in string.
fn find_key_position(s: &str, key: &str) -> Option<usize> {
    let pattern = format!("\"{key}\"");
    s.find(&pattern).map(|pos| pos + pattern.len())
}

/// Extract string value after colon.
fn extract_string_value(s: &str, after: usize) -> Option<(String, usize)> {
    if let Some(colon_pos) = s[after..].find(':') {
        let after_colon = after + colon_pos + 1;
        let rest = &s[after_colon..];
        if let Some(quote1) = rest.find('"') {
            let after_quote = after_colon + quote1 + 1;
            if let Some(quote2) = rest[quote1 + 1..].find('"') {
                let value = rest[quote1 + 1..quote1 + 1 + quote2].to_string();
                return Some((value, after_quote + quote2));
            }
        }
    }
    None
}

/// Extract number value.
fn extract_number_value(s: &str) -> Option<(f64, usize)> {
    let s = s.trim_start();
    let end = s
        .find(|c: char| !c.is_numeric() && c != '.')
        .unwrap_or(s.len());
    if end > 0 {
        s[..end].parse().ok().map(|v| (v, end))
    } else {
        None
    }
}

/// Get monitor dimensions from DRM modes.
fn get_monitor_dimensions(path: &Path) -> (u32, u32) {
    let modes_path = path.join("modes");
    if let Ok(mode) = fs::read_to_string(&modes_path) {
        let mode = mode.trim();
        if let Some((w, h)) = parse_resolution(mode) {
            return (w, h);
        }
    }
    (1920, 1080)
}

/// Get monitor position.
fn get_monitor_position(path: &Path) -> (i32, i32) {
    let x = fs::read_to_string(&path.join("crtc-x"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    let y = fs::read_to_string(&path.join("crtc-y"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    (x, y)
}

/// Get monitor scale.
fn get_monitor_scale(path: &Path) -> f32 {
    fs::read_to_string(&path.join("scale"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(1.0)
}

/// Parse resolution string like "1920x1080".
fn parse_resolution(s: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() == 2 {
        let w = parts[0].parse().ok()?;
        let h = parts[1].parse().ok()?;
        Some((w, h))
    } else {
        None
    }
}
