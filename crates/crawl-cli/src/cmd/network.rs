// cmd/network.rs
use clap::Args;
use anyhow::Result;
use serde_json::json;
use crate::{client::CrawlClient, output};

#[derive(Args)]
pub struct NetArgs {
    #[arg(long)] pub status: bool,
    #[arg(long)] pub wifi: bool,
    #[arg(long, value_name = "SSID")] pub connect: Option<String>,
    #[arg(long, value_name = "PASSWORD")] pub password: Option<String>,
}

pub async fn run(client: CrawlClient, args: NetArgs, json: bool) -> Result<()> {
    if let Some(ssid) = args.connect {
        let res = client.post("/network/connect", json!({ "ssid": ssid, "password": args.password })).await?;
        if json { output::print_value(&res, true); } else { output::print_ok(&format!("Connected to {ssid}")); }
    } else if args.wifi {
        let res = client.get("/network/wifi").await?;
        output::print_value(&res, json);
    } else {
        let res = client.get("/network/status").await?;
        if json {
            output::print_value(&res, true);
        } else {
            output::print_table(&[
                ("connectivity", res["connectivity"].as_str().unwrap_or("?").to_string()),
                ("wifi",         res["wifi_enabled"].as_bool().unwrap_or(false).to_string()),
                ("ssid",         res["active_ssid"].as_str().unwrap_or("—").to_string()),
            ]);
        }
    }
    Ok(())
}
