use clap::Args;
use anyhow::Result;
use serde_json::json;
use crate::{client::CrawlClient, output};

#[derive(Args)]
pub struct BtArgs {
    /// List paired/known devices
    #[arg(long)]
    pub list: bool,

    /// Start device discovery scan
    #[arg(long)]
    pub scan: bool,

    /// Connect to a device by address
    #[arg(long, value_name = "ADDRESS")]
    pub connect: Option<String>,

    /// Disconnect a device by address
    #[arg(long, value_name = "ADDRESS")]
    pub disconnect: Option<String>,

    /// Power the adapter on or off
    #[arg(long, value_name = "on|off")]
    pub power: Option<String>,

    /// Show adapter status
    #[arg(long)]
    pub status: bool,
}

pub async fn run(client: CrawlClient, args: BtArgs, json: bool) -> Result<()> {
    if let Some(addr) = args.connect {
        let res = client.post("/bluetooth/connect", json!({ "address": addr })).await?;
        if json { output::print_value(&res, true); } else { output::print_ok(&format!("Connected to {addr}")); }
    } else if let Some(addr) = args.disconnect {
        let res = client.post("/bluetooth/disconnect", json!({ "address": addr })).await?;
        if json { output::print_value(&res, true); } else { output::print_ok(&format!("Disconnected {addr}")); }
    } else if let Some(state) = args.power {
        let on = state == "on";
        let res = client.post("/bluetooth/power", json!({ "on": on })).await?;
        if json { output::print_value(&res, true); } else { output::print_ok(&format!("Adapter powered {state}")); }
    } else if args.scan {
        let res = client.post("/bluetooth/scan", json!({})).await?;
        if json { output::print_value(&res, true); } else { output::print_ok("Scan started"); }
    } else {
        // default: list / status
        let res = client.get("/bluetooth/status").await?;
        if json {
            output::print_value(&res, true);
        } else {
            let powered = res["powered"].as_bool().unwrap_or(false);
            let discovering = res["discovering"].as_bool().unwrap_or(false);
            println!("Bluetooth");
            output::print_table(&[
                ("powered",     powered.to_string()),
                ("discovering", discovering.to_string()),
            ]);
            if let Some(devices) = res["devices"].as_array() {
                if devices.is_empty() {
                    println!("  no devices");
                } else {
                    println!("  {:<20}  {:<24}  {:<10}  battery", "address", "name", "connected");
                    for d in devices {
                        let addr = d["address"].as_str().unwrap_or("?");
                        let name = d["name"].as_str().unwrap_or("(unknown)");
                        let conn = d["connected"].as_bool().unwrap_or(false);
                        let bat  = d["battery"].as_u64().map(|b| format!("{b}%")).unwrap_or_else(|| "—".into());
                        println!("  {addr:<20}  {name:<24}  {:<10}  {bat}", if conn { "yes" } else { "no" });
                    }
                }
            }
        }
    }
    Ok(())
}
