use anyhow::Result;
use clap::{Args, Subcommand};

use crate::{CrawlClient, output::{self, CliRenderable}};

#[derive(Args)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub action: DaemonAction,
}

#[derive(Subcommand)]
pub enum DaemonAction {
    /// Daemon status
    Status,
    /// Stop daemon
    Stop,
    /// Restart daemon
    Restart,
}

pub async fn run(client: CrawlClient, args: DaemonArgs, json: bool) -> Result<()> {
    match args.action {
        DaemonAction::Stop | DaemonAction::Restart => {
            // Issue systemctl commands since the daemon can't stop itself cleanly via socket
            let action = match args.action {
                DaemonAction::Restart => "restart",
                _ => "stop",
            };
            let status = std::process::Command::new("systemctl")
                .args(["--user", action, "crawl"])
                .status()?;
            if status.success() {
                output::print_ok(&format!("crawl daemon {action}ed"));
            } else {
                output::print_err(&format!("failed to {action} crawl daemon"));
            }
        }
        DaemonAction::Status => {
            match client.cmd("Health", serde_json::json!({})).await {
                Ok(res) => {
                    if json { output::print_value(&res, true); }
                    else {
                        let _ = output::handle_format(&res, json, |val| {
                            let headers = vec!["Property".to_string(), "Value".to_string()];
                            let rows = vec![
                                vec!["Status".to_string(), val["status"].as_str().unwrap_or("?").to_string()],
                                vec!["Version".to_string(), val["version"].as_str().unwrap_or("—").to_string()],
                            ];
                            let renderable = CliRenderable::new(headers, rows);
                            output::render_table(&renderable);
                            Ok(())
                        });
                    }
                }
                Err(e) => {
                    output::print_err(&format!("daemon unreachable: {e}"));
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}
