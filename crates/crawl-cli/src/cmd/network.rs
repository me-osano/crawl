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
    #[arg(long, value_name = "IFACE")] pub eth_connect: Option<String>,
    #[arg(long, value_name = "IFACE")] pub eth_disconnect: Option<String>,
    #[arg(long)] pub eth: bool,
}

pub async fn run(client: CrawlClient, args: NetArgs, json: bool) -> Result<()> {
    if args.eth_connect.is_some() || args.eth_disconnect.is_some() {
        if let Some(iface) = args.eth_connect {
            let res = client.post("/network/eth/connect", json!({ "interface": iface })).await?;
            if json { output::print_value(&res, true); } else {
                let iface = res["interface"].as_str().unwrap_or("?");
                output::print_ok(&format!("Ethernet connected on {iface}"));
            }
        } else {
            let res = client.post("/network/eth/disconnect", json!({ "interface": args.eth_disconnect })).await?;
            if json { output::print_value(&res, true); } else {
                let iface = res["interface"].as_str().unwrap_or("?");
                output::print_ok(&format!("Ethernet disconnected on {iface}"));
            }
        }
    } else if let Some(ssid) = args.connect {
        let res = client.post("/network/connect", json!({ "ssid": ssid, "password": args.password })).await?;
        if json { output::print_value(&res, true); } else { output::print_ok(&format!("Connected to {ssid}")); }
    } else if args.eth {
        let res = client.get("/network/status").await?;
        if json {
            output::print_value(&res, true);
        } else if let Some(interfaces) = res["interfaces"].as_array() {
            println!("  {:<12}  {:<12}  {:<15}  {}", "IFACE", "STATE", "IP", "MAC");
            for iface in interfaces {
                let name = iface["name"].as_str().unwrap_or("?");
                let state = iface["state"].as_str().unwrap_or("?");
                let ip4 = iface["ip4"].as_str().unwrap_or("—");
                let mac = iface["mac"].as_str().unwrap_or("—");
                println!("  {name:<12}  {state:<12}  {ip4:<15}  {mac}");
            }
        }
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
