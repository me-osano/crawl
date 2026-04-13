use clap::Args;
use anyhow::Result;
use serde_json::json;
use crate::{client::CrawlClient, output};

#[derive(Args)]
pub struct AudioArgs {
    #[arg(long)]
    pub input: bool,

    #[arg(long)]
    pub output: bool,

    /// Set volume percent (0–100)
    #[arg(long, value_name = "PERCENT")]
    pub volume: Option<u32>,

    /// Toggle mute on default device
    #[arg(long)]
    pub mute: bool,

    /// List devices
    #[arg(long)]
    pub list: bool,
}

pub async fn run(client: CrawlClient, args: AudioArgs, json: bool) -> Result<()> {
    let device = if args.input {
        Some("input")
    } else if args.output {
        Some("output")
    } else {
        None
    };

    if let Some(vol) = args.volume {
        let res = client.post("/audio/volume", json!({ "percent": vol, "device": device })).await?;
        if json { output::print_value(&res, true); } else { output::print_ok(&format!("Volume set to {vol}%")); }
    } else if args.mute {
        let res = client.post("/audio/mute", json!({ "device": device })).await?;
        if json { output::print_value(&res, true); } else { output::print_ok("Mute toggled"); }
    } else if args.list {
        let res = if args.input { client.get("/audio/sources").await? } else { client.get("/audio/sinks").await? };
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
