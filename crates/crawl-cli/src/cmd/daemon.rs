use clap::Args;
use anyhow::Result;
use crate::{client::CrawlClient, output};

#[derive(Args)]
pub struct DaemonArgs {
    #[arg(long)] pub status: bool,
    #[arg(long)] pub stop: bool,
    #[arg(long)] pub restart: bool,
}

pub async fn run(client: CrawlClient, args: DaemonArgs, json: bool) -> Result<()> {
    if args.stop || args.restart {
        // Issue systemctl commands since the daemon can't stop itself cleanly via socket
        let action = if args.restart { "restart" } else { "stop" };
        let status = std::process::Command::new("systemctl")
            .args(["--user", action, "crawl"])
            .status()?;
        if status.success() {
            output::print_ok(&format!("crawl daemon {action}ed"));
        } else {
            output::print_err(&format!("failed to {action} crawl daemon"));
        }
    } else {
        match client.get("/health").await {
            Ok(res) => {
                if json { output::print_value(&res, true); }
                else {
                    output::print_table(&[
                        ("status",  res["status"].as_str().unwrap_or("?").to_string()),
                        ("version", res["version"].as_str().unwrap_or("?").to_string()),
                    ]);
                }
            }
            Err(e) => {
                output::print_err(&format!("daemon unreachable: {e}"));
                std::process::exit(1);
            }
        }
    }
    Ok(())
}
