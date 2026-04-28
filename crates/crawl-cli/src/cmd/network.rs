use anyhow::Result;
use clap::{Args, Subcommand};
use serde_json::json;

use crate::{CrawlClient, output::{self, CliRenderable}};

#[derive(Args)]
pub struct NetArgs {
    #[command(subcommand)]
    pub action: NetAction,
}

#[derive(Subcommand)]
pub enum NetAction {
    /// Show network status
    Status,
    /// List network interfaces
    List,
    /// WiFi operations
    Wifi {
        #[command(subcommand)]
        action: WifiAction,
    },
    /// Ethernet operations
    Eth {
        #[command(subcommand)]
        action: EthAction,
    },
    /// Hotspot operations
    Hotspot {
        #[command(subcommand)]
        action: HotspotAction,
    },
    /// Enable or disable network
    Power {
        /// Turn power on or off
        #[arg(value_name = "on|off")]
        state: String,
    },
}

#[derive(Subcommand)]
pub enum WifiAction {
    /// List WiFi networks
    List,
    /// Scan for WiFi networks
    Scan,
    /// Show WiFi details
    Details,
    /// Connect to a WiFi network
    Connect {
        /// SSID of the network
        ssid: String,
        /// Password for the network
        #[arg(long)]
        password: Option<String>,
    },
    /// Disconnect from WiFi
    Disconnect,
    /// Forget a WiFi network
    Forget {
        /// SSID to forget
        ssid: String,
    },
}

#[derive(Subcommand)]
pub enum EthAction {
    /// List Ethernet interfaces
    List,
    /// Show Ethernet details
    Details,
    /// Connect Ethernet
    Connect {
        /// Interface name
        #[arg(long)]
        iface: Option<String>,
    },
    /// Disconnect Ethernet
    Disconnect,
}

#[derive(Subcommand)]
pub enum HotspotAction {
    /// Show hotspot status
    Status,
    /// Start hotspot
    Start {
        /// SSID for the hotspot
        #[arg(long, default_value = "Crawl-Hotspot")]
        ssid: String,
        /// Password for the hotspot
        #[arg(long)]
        password: Option<String>,
        /// Band (2.4GHz or 5GHz)
        #[arg(long)]
        band: Option<String>,
        /// Channel number
        #[arg(long)]
        channel: Option<u32>,
        /// Backend (networkmanager or hostapd)
        #[arg(long)]
        backend: Option<String>,
    },
    /// Stop hotspot
    Stop,
}

pub async fn run(client: CrawlClient, args: NetArgs, json_mode: bool) -> Result<()> {
    match args.action {
        NetAction::Status => {
            let res = client.cmd("NetStatus", json!({})).await?;
            output::handle_format(&res, json_mode, |val| {
                let headers = vec!["Property".to_string(), "Value".to_string()];
                let rows = vec![
                    vec!["Connectivity".to_string(), val["connectivity"].as_str().unwrap_or("?").to_string()],
                    vec!["WiFi".to_string(), val["wifi_enabled"].as_bool().unwrap_or(false).to_string()],
                    vec!["SSID".to_string(), val["active_ssid"].as_str().unwrap_or("—").to_string()],
                ];
                let renderable = CliRenderable::new(headers, rows);
                output::render_table(&renderable);
                Ok(())
            })
        }
        NetAction::List => {
            let res = client.cmd("NetStatus", json!({})).await?;
            output::handle_format(&res, json_mode, |val| {
                if let Some(interfaces) = val["interfaces"].as_array() {
                    let headers = vec!["IFACE".to_string(), "STATE".to_string(), "IP".to_string(), "MAC".to_string()];
                    let rows: Vec<Vec<String>> = interfaces
                        .iter()
                        .map(|iface| {
                            vec![
                                iface["name"].as_str().unwrap_or("?").to_string(),
                                iface["state"].as_str().unwrap_or("?").to_string(),
                                iface["ip4"].as_str().unwrap_or("—").to_string(),
                                iface["mac"].as_str().unwrap_or("—").to_string(),
                            ]
                        })
                        .collect();
                    let renderable = CliRenderable::new(headers, rows);
                    output::render_table(&renderable);
                }
                Ok(())
            })
        }
        NetAction::Wifi { action } => match action {
            WifiAction::List => {
                let res = client.cmd("NetWifiList", json!({})).await?;
                output::handle_format(&res, json_mode, |_| {
                    output::print_ok("WiFi networks listed");
                    Ok(())
                })
            }
            WifiAction::Scan => {
                let res = client.cmd("NetWifiScan", json!({})).await?;
                output::handle_format(&res, json_mode, |_| {
                    output::print_ok("WiFi scan requested");
                    Ok(())
                })
            }
            WifiAction::Details => {
                let res = client.cmd("NetWifiDetails", json!({})).await?;
                output::handle_format(&res, json_mode, |_| {
                    output::print_ok("WiFi details shown");
                    Ok(())
                })
            }
            WifiAction::Connect { ssid, password } => {
                let res = client.cmd("NetWifiConnect", json!({ "ssid": ssid, "password": password })).await?;
                output::handle_format(&res, json_mode, |_| {
                    output::print_ok("WiFi connect requested");
                    Ok(())
                })
            }
            WifiAction::Disconnect => {
                let res = client.cmd("NetWifiDisconnect", json!({})).await?;
                output::handle_format(&res, json_mode, |_| {
                    output::print_ok("WiFi disconnected");
                    Ok(())
                })
            }
            WifiAction::Forget { ssid } => {
                let res = client.cmd("NetWifiForget", json!({ "ssid": ssid })).await?;
                output::handle_format(&res, json_mode, |_| {
                    output::print_ok(&format!("Network '{ssid}' forgotten"));
                    Ok(())
                })
            }
        },
        NetAction::Eth { action } => match action {
            EthAction::List => {
                let res = client.cmd("NetEthList", json!({})).await?;
                output::handle_format(&res, json_mode, |_| {
                    output::print_ok("Ethernet interfaces listed");
                    Ok(())
                })
            }
            EthAction::Details => {
                let res = client.cmd("NetEthDetails", json!({})).await?;
                output::handle_format(&res, json_mode, |_| {
                    output::print_ok("Ethernet details shown");
                    Ok(())
                })
            }
            EthAction::Connect { iface } => {
                let res = client.cmd("NetEthConnect", json!({ "interface": iface })).await?;
                output::handle_format(&res, json_mode, |val| {
                    let iface_out = val["interface"].as_str().unwrap_or("?");
                    output::print_ok(&format!("Ethernet connected on {iface_out}"));
                    Ok(())
                })
            }
            EthAction::Disconnect => {
                let res = client.cmd("NetEthDisconnect", json!({})).await?;
                output::handle_format(&res, json_mode, |val| {
                    let iface_out = val["interface"].as_str().unwrap_or("?");
                    output::print_ok(&format!("Ethernet disconnected on {iface_out}"));
                    Ok(())
                })
            }
        },
        NetAction::Hotspot { action } => match action {
            HotspotAction::Status => {
                let res = client.cmd("NetHotspotStatus", json!({})).await?;
                output::handle_format(&res, json_mode, |_| {
                    output::print_ok("Hotspot status shown");
                    Ok(())
                })
            }
            HotspotAction::Start { ssid, password, band, channel, backend } => {
                let mut payload = json!({ "ssid": ssid.clone() });
                if let Some(ref pwd) = password {
                    payload["password"] = json!(pwd);
                }
                if let Some(ref b) = band {
                    payload["band"] = json!(b);
                }
                if let Some(ch) = channel {
                    payload["channel"] = json!(ch);
                }
                if let Some(ref be) = backend {
                    payload["backend"] = json!(be);
                }
                let res = client.cmd("NetHotspotStart", payload).await?;
                output::handle_format(&res, json_mode, |val| {
                    let ssid_out = val["ssid"].as_str().unwrap_or(&ssid);
                    let iface_out = val["iface"].as_str().unwrap_or("?");
                    output::print_ok(&format!("Hotspot started: '{ssid_out}' on {iface_out}"));
                    Ok(())
                })
            }
            HotspotAction::Stop => {
                let res = client.cmd("NetHotspotStop", json!({})).await?;
                output::handle_format(&res, json_mode, |_| {
                    output::print_ok("Hotspot stopped");
                    Ok(())
                })
            }
        },
        NetAction::Power { state } => {
            let enabled = matches!(state.as_str(), "on" | "true" | "1");
            let res = client.cmd("NetPower", json!({ "on": enabled })).await?;
            output::handle_format(&res, json_mode, |_| {
                output::print_ok(if enabled { "Network enabled" } else { "Network disabled" });
                Ok(())
            })
        }
    }
}