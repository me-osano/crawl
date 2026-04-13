// cmd/network.rs
use clap::Args;
use anyhow::Result;
use serde_json::json;
use crate::{client::CrawlClient, output};

#[derive(Args)]
pub struct NetArgs {
    #[arg(long)] pub status: bool,
    #[arg(long)] pub wifi: bool,
    #[arg(long)] pub eth: bool,
    #[arg(long)] pub scan: bool,
    #[arg(long)] pub list: bool,
    #[arg(long)] pub connect: bool,
    #[arg(long)] pub disconnect: bool,
    #[arg(long, value_name = "SSID")] pub ssid: Option<String>,
    #[arg(long, value_name = "PASSWORD")] pub password: Option<String>,
    #[arg(long, value_name = "IFACE")] pub iface: Option<String>,
    #[arg(long, value_name = "on|off")] pub power: Option<String>,
}

pub async fn run(client: CrawlClient, args: NetArgs, json: bool) -> Result<()> {
    if let Some(power) = args.power.as_deref() {
        let enabled = matches!(power, "on" | "true" | "1");
        let res = client.post("/network/power", json!({ "on": enabled })).await?;
        if json { output::print_value(&res, true); } else {
            output::print_ok(if enabled { "Network enabled" } else { "Network disabled" });
        }
    } else if args.wifi {
        if args.scan {
            let res = client.post("/network/wifi/scan", json!({})).await?;
            if json { output::print_value(&res, true); } else { output::print_ok("WiFi scan requested"); }
        } else if args.connect {
            let ssid = args.ssid.unwrap_or_default();
            let res = client.post("/network/wifi/connect", json!({ "ssid": ssid, "password": args.password })).await?;
            if json { output::print_value(&res, true); } else { output::print_ok("WiFi connect requested"); }
        } else if args.disconnect {
            let res = client.post("/network/wifi/disconnect", json!({})).await?;
            if json { output::print_value(&res, true); } else { output::print_ok("WiFi disconnected"); }
        } else if args.list {
            let res = client.get("/network/wifi").await?;
            output::print_value(&res, json);
        } else {
            let res = client.get("/network/wifi").await?;
            output::print_value(&res, json);
        }
    } else if args.eth {
        if args.connect {
            let res = client.post("/network/eth/connect", json!({ "interface": args.iface })).await?;
            if json { output::print_value(&res, true); } else {
                let iface = res["interface"].as_str().unwrap_or("?");
                output::print_ok(&format!("Ethernet connected on {iface}"));
            }
        } else if args.disconnect {
            let res = client.post("/network/eth/disconnect", json!({ "interface": args.iface })).await?;
            if json { output::print_value(&res, true); } else {
                let iface = res["interface"].as_str().unwrap_or("?");
                output::print_ok(&format!("Ethernet disconnected on {iface}"));
            }
        } else if args.list {
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
        } else {
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
        }
    } else if args.status {
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
    } else {
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
    }
    Ok(())
}
