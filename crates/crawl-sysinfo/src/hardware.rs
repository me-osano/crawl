//! Hardware information gathering.

use std::fs;

use super::models::HardwareInfo;

/// Get hardware information snapshot.
pub fn get_info() -> HardwareInfo {
    HardwareInfo {
        cpu_model: get_cpu_model(),
        cpu_cores: get_cpu_cores(),
        memory_total: get_memory_total(),
        gpu: detect_gpu(),
        disk_total: get_disk_total(),
        disk_used: get_disk_used(),
    }
}

/// Read CPU model name from /proc/cpuinfo.
fn get_cpu_model() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|content| {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("model name") || line.starts_with("Model") {
                    if let Some(pos) = line.find(':') {
                        let model = line[pos + 1..].trim().to_string();
                        if !model.is_empty() {
                            return Some(model);
                        }
                    }
                }
            }
            None
        })
        .unwrap_or_else(|| "Unknown CPU".to_string())
}

/// Get number of CPU cores.
fn get_cpu_cores() -> usize {
    // Method 1: Try /proc/cpuinfo
    let count_from_cpuinfo = fs::read_to_string("/proc/cpuinfo")
        .ok()
        .map(|content| {
            content
                .lines()
                .filter(|l| l.starts_with("processor"))
                .count()
        })
        .filter(|&c| c > 0);

    if let Some(count) = count_from_cpuinfo {
        return count;
    }

    // Method 2: Try nproc
    std::process::Command::new("nproc")
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8(o.stdout)
                .ok()
                .and_then(|s| s.trim().parse().ok())
        })
        .unwrap_or(1)
}

/// Get total memory in bytes from /proc/meminfo.
fn get_memory_total() -> u64 {
    fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|content| {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("MemTotal:") {
                    if let Some(kb) = extract_meminfo_value(line) {
                        return Some(kb * 1024); // Convert KB to bytes
                    }
                }
            }
            None
        })
        .unwrap_or(0)
}

/// Extract numeric value from meminfo line (e.g., "MemTotal: 16384000 kB").
fn extract_meminfo_value(line: &str) -> Option<u64> {
    if let Some(colon_pos) = line.find(':') {
        let value_part = line[colon_pos + 1..].trim();
        // Extract number (may have kB, MB, GB suffix)
        let value_str = value_part.split_whitespace().next().unwrap_or("0");
        value_str.parse().ok()
    } else {
        None
    }
}

/// Simple GPU detection (just looking for common GPU device files).
fn detect_gpu() -> Option<String> {
    // Try to get GPU info from lspci
    let output = std::process::Command::new("lspci")
        .args(["-d", "::0300"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Get first VGA compatible device
    for line in stdout.lines() {
        if line.contains("VGA") || line.contains("Display") {
            // Extract just the description part after "VGA controller"
            if let Some(pos) = line.find("VGA controller:") {
                let desc = line[pos + "VGA controller: ".len()..].trim();
                if !desc.is_empty() {
                    return Some(desc.to_string());
                }
            }
            return Some(line.trim().to_string());
        }
    }

    // Fallback: check for backlight (laptop indicator)
    if std::path::Path::new("/sys/class/backlight").exists() {
        return Some("Integrated graphics (laptop)".to_string());
    }

    None
}

/// Get total disk space for root partition.
fn get_disk_total() -> Option<u64> {
    std::process::Command::new("df")
        .args(["-B1", "/"])
        .output()
        .ok()
        .and_then(|o| {
            let output = String::from_utf8_lossy(&o.stdout);
            output.lines()
                .nth(1)
                .and_then(|l| l.split_whitespace().nth(2))
                .and_then(|v| v.parse::<u64>().ok())
        })
}

/// Get used disk space for root partition.
fn get_disk_used() -> Option<u64> {
    std::process::Command::new("df")
        .args(["-B1", "/"])
        .output()
        .ok()
        .and_then(|o| {
            let output = String::from_utf8_lossy(&o.stdout);
            output.lines()
                .nth(1)
                .and_then(|l| l.split_whitespace().nth(3))
                .and_then(|v| v.parse::<u64>().ok())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_info() {
        let info = get_info();
        assert!(!info.cpu_model.is_empty());
        assert!(info.cpu_cores > 0);
        assert!(info.memory_total > 0);
    }

    #[test]
    fn test_cpu_cores() {
        let cores = get_cpu_cores();
        assert!(cores > 0);
    }
}
