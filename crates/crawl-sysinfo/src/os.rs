//! Operating system information gathering.

use std::fs;

use super::models::OsInfo;

/// Get OS information from /etc/os-release and uname.
pub fn get_info() -> OsInfo {
    let hostname = get_hostname();
    let kernel = get_kernel();
    let (name, pretty_name, id) = parse_os_release();

    OsInfo {
        name,
        kernel,
        pretty_name,
        hostname,
        id,
    }
}

/// Get system hostname.
fn get_hostname() -> String {
    fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Get kernel version via uname.
fn get_kernel() -> String {
    // Try to read from /proc first (faster, no process spawn)
    if let Ok(version) = fs::read_to_string("/proc/version") {
        let version = version.trim();
        // Format: Linux version 6.8.1-arch1-2 ...
        if let Some(pos) = version.find("Linux version ") {
            let version_part = &version[pos + "Linux version ".len()..];
            if let Some(end) = version_part.find(' ') {
                return version_part[..end].to_string();
            }
            return version_part.to_string();
        }
        return version.to_string();
    }

    // Fallback to Command (less preferred)
    std::process::Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Parse /etc/os-release for OS details.
fn parse_os_release() -> (String, String, String) {
    let mut name = String::new();
    let mut pretty_name = String::new();
    let mut id = String::new();

    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("NAME=") {
                name = unquote(&line[5..]);
            } else if line.starts_with("PRETTY_NAME=") {
                pretty_name = unquote(&line[12..]);
            } else if line.starts_with("ID=") {
                id = unquote(&line[3..]);
            }
        }
    }

    // If pretty_name is empty, use name as fallback
    if pretty_name.is_empty() {
        pretty_name = name.clone();
    }

    (name, pretty_name, id)
}

/// Remove quotes from a value.
fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_info() {
        let info = get_info();
        // hostname and kernel should not be empty on a real system
        assert!(!info.hostname.is_empty() || !info.kernel.is_empty());
    }

    #[test]
    fn test_unquote() {
        assert_eq!(unquote("\"value\""), "value");
        assert_eq!(unquote("'value'"), "value");
        assert_eq!(unquote("value"), "value");
    }
}
