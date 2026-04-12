use clap::Args;
use anyhow::Result;
use serde_json::json;
use crate::{client::CrawlClient, output};

#[derive(Args)]
pub struct BrightnessArgs {
    /// Get current brightness
    #[arg(long)]
    pub get: bool,

    /// Set brightness to an absolute percentage (0–100)
    #[arg(long, value_name = "PERCENT")]
    pub set: Option<u8>,

    /// Increase brightness by N percent
    #[arg(long, value_name = "PERCENT")]
    pub inc: Option<u8>,

    /// Decrease brightness by N percent
    #[arg(long, value_name = "PERCENT")]
    pub dec: Option<u8>,
}

pub async fn run(client: CrawlClient, args: BrightnessArgs, json: bool) -> Result<()> {
    if let Some(val) = args.set {
        let res = client.post("/brightness/set", json!({ "value": val })).await?;
        if json { output::print_value(&res, true); } else { output::print_ok(&format!("Brightness set to {val}%")); }
    } else if let Some(val) = args.inc {
        let res = client.post("/brightness/inc", json!({ "value": val })).await?;
        if json { output::print_value(&res, true); } else { output::print_ok(&format!("Brightness increased by {val}%")); }
    } else if let Some(val) = args.dec {
        let res = client.post("/brightness/dec", json!({ "value": val })).await?;
        if json { output::print_value(&res, true); } else { output::print_ok(&format!("Brightness decreased by {val}%")); }
    } else {
        // default: get
        let res = client.get("/brightness").await?;
        if json {
            output::print_value(&res, true);
        } else {
            let pct = res["percent"].as_f64().unwrap_or(0.0);
            let cur = res["current"].as_u64().unwrap_or(0);
            let max = res["max"].as_u64().unwrap_or(0);
            output::print_table(&[
                ("device",  res["device"].as_str().unwrap_or("unknown").to_string()),
                ("percent", format!("{pct:.0}%")),
                ("raw",     format!("{cur}/{max}")),
            ]);
        }
    }
    Ok(())
}
