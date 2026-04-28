use anyhow::{Result, Context};
use clap::Parser;
use serde_json::Value;

use crate::CrawlClient;
use crate::logo::get_logo;
use crate::output::{self, CliRenderable};

#[derive(Parser, Debug)]
pub struct SysinfoArgs {
    /// Show only specific field (compositor, os, session, hardware, display)
    #[arg(long, short = 'f')]
    field: Option<String>,
    /// Logo to display (default: autodetect from OS)
    #[arg(long)]
    logo: Option<String>,
    /// Don't show logo
    #[arg(long)]
    no_logo: bool,
    /// Use table format instead of logo
    #[arg(long)]
    table: bool,
}

pub async fn run(client: CrawlClient, args: SysinfoArgs, json_mode: bool) -> Result<()> {
    let response: Value = client.cmd("Sysinfo", Value::Null).await?;

    let result = response.get("result")
        .context("Missing 'result' in response")?;

    if let Some(field) = &args.field {
        let value = result.get(field)
            .context(format!("Field not found: {}", field))?;
        println!("{}", serde_json::to_string_pretty(value)?);
        return Ok(());
    }

    output::handle_format(&response, json_mode, |_| {
        if args.table {
            let pairs = collect_pairs(result);
            let headers = vec!["Property".to_string(), "Value".to_string()];
            let rows: Vec<Vec<String>> = pairs
                .into_iter()
                .map(|(k, v)| vec![k.to_string(), v])
                .collect();
            let renderable = CliRenderable::new(headers, rows);
            output::render_table(&renderable);
        } else if args.no_logo {
            let pairs = collect_pairs(result);
            let headers = vec!["Property".to_string(), "Value".to_string()];
            let rows: Vec<Vec<String>> = pairs
                .into_iter()
                .map(|(k, v)| vec![k.to_string(), v])
                .collect();
            let renderable = CliRenderable::new(headers, rows);
            output::render_table(&renderable);
        } else {
    let os_id = result.get("os")
        .and_then(|v: &serde_json::Value| v.get("id"))
        .and_then(|v: &serde_json::Value| v.as_str())
        .unwrap_or("default");
            let logo_name = args.logo.as_deref().unwrap_or(os_id);
            let logo = get_logo(logo_name);
            print_logo(logo, result);
        }
        Ok(())
    })
}

fn print_logo(logo: &crate::logo::Logo, info: &Value) {
    let mut output = String::new();
    let lines = &logo.lines;
    let logo_width = logo.width;

    // Collect all info as key-value pairs
    let pairs = collect_pairs(info);

    // Terminal width
    let term_width = term_size().map(|(w, _)| w).unwrap_or(80);
    let info_start = (logo_width + 2).min(term_width.saturating_sub(logo_width + 2));

    // Print line by line
    let max_lines = lines.len().max(pairs.len());
    for i in 0..max_lines {
        if i < lines.len() {
            let line = &lines[i];
            let padding = if line.len() < logo_width {
                " ".repeat(logo_width - line.len())
            } else {
                String::new()
            };
            output.push_str(line);
            output.push_str(&padding);
            output.push_str("  ");
        } else {
            output.push_str(&" ".repeat(logo_width + 2));
        }

        if i < pairs.len() {
            let (key, val) = &pairs[i];
            let key_width = key.len() + 1;
            output.push_str(key);
            output.push_str(&" ".repeat(info_start.saturating_sub(key_width)));
            output.push_str(val);
        }

        output.push('\n');
    }

    println!("{}", output);
}

fn term_size() -> Option<(usize, usize)> {
    term_size::dimensions().map(|(w, h)| (w as usize, h as usize))
}

fn collect_pairs(info: &Value) -> Vec<(&'static str, String)> {
    let mut pairs: Vec<(&str, String)> = Vec::new();

    // OS
    if let Some(os) = info.get("os") {
        let name = os.get("name").and_then(|v| v.as_str()).unwrap_or("-");
        let kernel = os.get("kernel").and_then(|v| v.as_str()).unwrap_or("-");
        let hostname = os.get("hostname").and_then(|v| v.as_str()).unwrap_or("-");
        pairs.push(("OS", name.to_string()));
        pairs.push(("Kernel", kernel.to_string()));
        pairs.push(("Host", hostname.to_string()));
    }

    // Uptime
    if let Some(sess) = info.get("session") {
        if let Some(user) = sess.get("user").and_then(|v| v.as_str()) {
            pairs.push(("User", user.to_string()));
        }
        if let Some(uptime) = sess.get("uptime_seconds").and_then(|v| v.as_u64()) {
            pairs.push(("Uptime", format_uptime(uptime)));
        }
        if let Some(shell) = sess.get("shell").and_then(|v| v.as_str()) {
            pairs.push(("Shell", shell.to_string()));
        }
        if let Some(term) = sess.get("terminal").and_then(|v| v.as_str()) {
            pairs.push(("Terminal", term.to_string()));
        }
    }

    // Compositor
    if let Some(c) = info.get("compositor") {
        let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("-");
        let ct = c.get("type").and_then(|v| v.as_str()).unwrap_or("unknown");
        pairs.push(("DE/WM", format!("{} ({})", name, ct)));
    }

    // Resolution
    if let Some(dpy) = info.get("display") {
        if let Some(mons) = dpy.get("monitors").and_then(|v| v.as_array()) {
            if let Some(m) = mons.first() {
                let w = m.get("width").and_then(|v| v.as_u64()).unwrap_or(0);
                let h = m.get("height").and_then(|v| v.as_u64()).unwrap_or(0);
                let scale = m.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0);
                pairs.push(("Resolution", format!("{}x{} @ {:.1}x", w, h, scale)));
            }
        }
    }

    // CPU
    if let Some(hw) = info.get("hardware") {
        let cpu = hw.get("cpu_model").and_then(|v| v.as_str()).unwrap_or("-");
        let cores = hw.get("cpu_cores").and_then(|v| v.as_u64()).unwrap_or(0);
        pairs.push(("CPU", format!("{} ({} cores)", cpu, cores)));

        let mem = hw.get("memory_total").and_then(|v| v.as_u64()).unwrap_or(0);
        let mem_gb = mem as f64 / 1_073_741_824.0;
        pairs.push(("Memory", format!("{:.1} GB", mem_gb)));

        if let Some(gpu) = hw.get("gpu").and_then(|v| v.as_str()) {
            pairs.push(("GPU", gpu.to_string()));
        }

        // Disk
        if let (Some(total), Some(used)) = (
            hw.get("disk_total").and_then(|v| v.as_u64()),
            hw.get("disk_used").and_then(|v| v.as_u64()),
        ) {
            let total_gb = total as f64 / 1_073_741_824.0;
            let used_gb = used as f64 / 1_073_741_824.0;
            pairs.push(("Disk", format!("{:.0}G / {:.0}G", used_gb, total_gb)));
        }
    }

    pairs
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let mins = (seconds % 3600) / 60;

    if days > 0 {
        format!("{} days, {} hours", days, hours)
    } else if hours > 0 {
        format!("{} hours, {} mins", hours, mins)
    } else {
        format!("{} mins", mins)
    }
}