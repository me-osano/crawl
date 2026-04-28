//! System commands and utilities (iw, ip, sysfs, etc.).

use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

pub fn read_sysfs_speed(ifname: &str) -> Option<u32> {
    let path = PathBuf::from("/sys/class/net").join(ifname).join("speed");
    let data = std::fs::read_to_string(path).ok()?;
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<u32>().ok().filter(|v| *v > 0)
}

pub fn supports_virtual_ap(phy: &str) -> bool {
    let output = std::process::Command::new("iw")
        .args(["dev", phy, "info"])
        .output()
        .ok();
    output.map_or(false, |o| {
        String::from_utf8_lossy(&o.stdout).contains("type AP")
    })
}

pub fn detect_current_channel(iface: &str) -> Option<u32> {
    let out = std::process::Command::new("sh")
        .args(["-c", &format!("iw dev {} info | awk '/channel/ {{print $2}}'", iface)])
        .output()
        .ok()?;
    let freq: u32 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    frequency_channel(freq)
}

pub fn detect_upstream_type() -> String {
    let output = std::process::Command::new("sh")
        .args(["-c", "ip route | awk '/default/ {print $5; exit}'"])
        .output().ok();
    output
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

pub async fn detect_wifi_phy(iface: &str) -> Result<String, crate::NetError> {
    let out = run_shell(&format!(
        "iw dev {} info | awk '/wiphy/ {{print $2}}' | head -1", iface
    )).await?;
    Ok(out.trim().to_string())
}

pub async fn run_cmd(cmd: &str, args: &[&str]) -> Result<String, crate::NetError> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .await
        .map_err(|_e| crate::NetError::Unavailable)?
        .stdout;
    Ok(String::from_utf8_lossy(&output).to_string())
}

pub async fn run_cmd_bg(cmd: &str, args: Vec<String>) -> Result<(), crate::NetError> {
    Command::new(cmd)
        .args(&args)
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_e| crate::NetError::Unsupported(_e.to_string()))?;
    Ok(())
}

pub async fn run_shell(script: &str) -> Result<String, crate::NetError> {
    let output = Command::new("sh")
        .args(["-c", script])
        .output()
        .await
        .map_err(|_e| crate::NetError::Unavailable)?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub async fn nm_mark_unmanaged(iface: &str) -> Result<(), crate::NetError> {
    run_shell(&format!(
        "nmcli device set {} managed no 2>/dev/null || true", iface
    )).await?;
    Ok(())
}

pub async fn nm_restore_managed(iface: &str) -> Result<(), crate::NetError> {
    run_shell(&format!(
        "nmcli device set {} managed yes 2>/dev/null || true", iface
    )).await?;
    Ok(())
}

pub async fn setup_ip_forward(ap_iface: &str, upstream_iface: &str) -> Result<(), crate::NetError> {
    run_shell("echo 1 > /proc/sys/net/ipv4/ip_forward 2>/dev/null || sysctl -w net.ipv4.ip_forward=1 2>/dev/null || true").await?;

    let nat_rule = format!(
        "iptables -t nat -C POSTROUTING -o {} -j MASQUERADE 2>/dev/null \
         || iptables -t nat -A POSTROUTING -o {} -j MASQUERADE",
        upstream_iface,
        upstream_iface
    );
    run_shell(&nat_rule).await?;

    let fwd_rule = format!(
        "iptables -C FORWARD -i {} -o {} -m state --state RELATED,ESTABLISHED -j ACCEPT 2>/dev/null \
         || iptables -A FORWARD -i {} -o {} -m state --state RELATED,ESTABLISHED -j ACCEPT",
        ap_iface, upstream_iface, ap_iface, upstream_iface
    );
    run_shell(&fwd_rule).await?;

    let inbound = format!(
        "iptables -C FORWARD -i {} -j ACCEPT 2>/dev/null \
         || iptables -A FORWARD -i {} -j ACCEPT",
        ap_iface, ap_iface
    );
    run_shell(&inbound).await?;

    let out = format!(
        "iptables -C FORWARD -o {} -j ACCEPT 2>/dev/null \
         || iptables -A FORWARD -o {} -j ACCEPT",
        ap_iface, ap_iface
    );
    run_shell(&out).await?;

    Ok(())
}

pub fn teardown_nat(upstream_iface: &str) {
    let _ = std::process::Command::new("sh")
        .args(["-c", &format!(
            "iptables -t nat -D POSTROUTING -o {} -j MASQUERADE 2>/dev/null; \
             iptables -D FORWARD -i {} -o {} -m state --state RELATED,ESTABLISHED -j ACCEPT 2>/dev/null; \
             iptables -D FORWARD -i {} -j ACCEPT 2>/dev/null; \
             iptables -D FORWARD -o {} -j ACCEPT 2>/dev/null",
            upstream_iface, "%ap%", upstream_iface, "%ap%", upstream_iface
        )])
        .output();
}

pub fn teardown_interface(iface: &str) {
    let _ = std::process::Command::new("iw")
        .args(["dev", iface, "del"])
        .output();
}

fn frequency_channel(freq: u32) -> Option<u32> {
    if (2412..=2472).contains(&freq) {
        return Some(((freq - 2407) / 5) as u32);
    }
    if freq == 2484 {
        return Some(14);
    }
    if (5000..=5895).contains(&freq) {
        return Some(((freq - 5000) / 5) as u32);
    }
    if (5925..=7125).contains(&freq) {
        return Some(((freq - 5950) / 5) as u32);
    }
    None
}