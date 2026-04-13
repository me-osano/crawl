use anyhow::Result;
use clap::Args;
use serde_json::json;
use crate::output;

#[derive(Args)]
pub struct UpdateArgs {
    /// Pass additional args to the updater script
    #[arg(last = true)]
    pass_through: Vec<String>,

    /// Only print the latest release tag (no install)
    #[arg(long)]
    dry_run: bool,
}

pub async fn run(_client: crate::client::CrawlClient, args: UpdateArgs, json: bool) -> Result<()> {
    let mut passthrough = args.pass_through.clone();
    if args.dry_run {
        passthrough.insert(0, "--dry-run".to_string());
    }

    let mut cmd = std::process::Command::new("bash");
    cmd.arg("-c")
        .arg("curl -fsSL https://raw.githubusercontent.com/me-osano/crawl/main/pkg/update.sh | bash -s -- \"$@\"")
        .arg("--")
        .args(&passthrough);

    let output_res = cmd.output()?;
    let success = output_res.status.success();
    let stderr = String::from_utf8_lossy(&output_res.stderr).trim().to_string();
    if args.dry_run {
        let tag = String::from_utf8_lossy(&output_res.stdout).trim().to_string();
        let installed = get_installed_version();
        let tag_missing = tag.is_empty();
        if json {
            output::print_value(
                &json!({
                    "ok": success && !tag_missing,
                    "tag": tag,
                    "installed": installed,
                    "error": if success && !tag_missing { None } else { Some(stderr) }
                }),
                true,
            );
        } else if success && !tag_missing {
            let installed_msg = installed.as_deref().unwrap_or("unknown");
            output::print_ok(&format!("latest release tag: {tag}"));
            output::print_ok(&format!("installed version: {installed_msg}"));
        } else {
            let err_msg = if stderr.is_empty() {
                "latest release tag not found".to_string()
            } else {
                format!("latest release tag not found: {stderr}")
            };
            output::print_err(&err_msg);
        }
    } else if json {
        output::print_value(&json!({"ok": success}), true);
    } else if success {
        output::print_ok("updated crawl to latest release");
    } else {
        output::print_err("update failed");
    }

    Ok(())
}

fn get_installed_version() -> Option<String> {
    let output = std::process::Command::new("pacman")
        .args(["-Qi", "crawl"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("Version") {
            let version = rest.splitn(2, ':').nth(1).map(|s| s.trim())?;
            if !version.is_empty() {
                return Some(version.to_string());
            }
        }
    }

    None
}
