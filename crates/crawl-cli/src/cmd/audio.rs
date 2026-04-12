use clap::Args;
use anyhow::Result;
use serde_json::json;
use crate::{client::CrawlClient, output};

#[derive(Args)]
pub struct AudioArgs {
    /// Get default sink volume
    #[arg(long)]
    pub get: bool,

    /// Set volume percent (0–100)
    #[arg(long, value_name = "PERCENT")]
    pub volume: Option<u32>,

    /// Toggle mute on default sink
    #[arg(long)]
    pub mute: bool,

    /// List all sinks
    #[arg(long)]
    pub sinks: bool,

    /// List all sources (microphones)
    #[arg(long)]
    pub sources: bool,
}

pub async fn run(client: CrawlClient, args: AudioArgs, json: bool) -> Result<()> {
    if let Some(vol) = args.volume {
        let res = client.post("/audio/volume", json!({ "percent": vol })).await?;
        if json { output::print_value(&res, true); } else { output::print_ok(&format!("Volume set to {vol}%")); }
    } else if args.mute {
        let res = client.post("/audio/mute", json!({})).await?;
        if json { output::print_value(&res, true); } else { output::print_ok("Mute toggled"); }
    } else if args.sources {
        let res = client.get("/audio/sources").await?;
        output::print_value(&res, json);
    } else {
        let res = client.get("/audio/sinks").await?;
        if json {
            output::print_value(&res, true);
        } else if let Some(sinks) = res.as_array() {
            println!("Audio sinks");
            for s in sinks {
                let name    = s["name"].as_str().unwrap_or("?");
                let vol     = s["volume_percent"].as_u64().unwrap_or(0);
                let muted   = s["muted"].as_bool().unwrap_or(false);
                let default = s["is_default"].as_bool().unwrap_or(false);
                let marker  = if default { "* " } else { "  " };
                let mute_s  = if muted { " [muted]" } else { "" };
                println!("{marker}{name:<40}  {vol:>3}%{mute_s}");
            }
        }
    }
    Ok(())
}
