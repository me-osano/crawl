use anyhow::Result;
use clap::{Args, Subcommand};
use serde_json::json;

use crate::{CrawlClient, output::{self, CliRenderable}};

#[derive(Args)]
pub struct BtArgs {
    #[command(subcommand)]
    pub action: BtAction,
}

#[derive(Subcommand)]
pub enum BtAction {
    /// List paired/known devices
    List,
    /// Start device discovery scan
    Scan,
    /// Show adapter status
    Status,
    /// Connect to a device by address
    Connect {
        /// Device address
        address: String,
    },
    /// Disconnect a device by address
    Disconnect {
        /// Device address
        address: String,
    },
    /// Power the adapter on or off
    Power {
        /// Turn power on or off
        #[arg(value_name = "on|off")]
        state: String,
    },
    /// Set adapter discoverable on/off
    Discoverable {
        /// Turn discoverable on or off
        #[arg(value_name = "on|off")]
        state: String,
    },
    /// Set adapter pairable on/off
    Pairable {
        /// Turn pairable on or off
        #[arg(value_name = "on|off")]
        state: String,
    },
    /// Set device alias (name)
    Alias {
        /// Device address
        address: String,
        /// New alias name
        alias: String,
    },
    /// Pair with a device by address
    Pair {
        /// Device address
        address: String,
    },
    /// Trust or untrust a device by address
    Trust {
        /// Device address
        address: String,
        /// Enable or disable trust
        #[arg(value_name = "on|off")]
        trusted: String,
    },
    /// Remove/forget a device by address
    Remove {
        /// Device address
        address: String,
    },
}

pub async fn run(client: CrawlClient, args: BtArgs, json_mode: bool) -> Result<()> {
    match args.action {
        BtAction::Connect { address } => {
            let res = client.cmd("BtConnect", json!({ "address": address })).await?;
            output::handle_format(&res, json_mode, |_| {
                output::print_ok(&format!("Connected to {address}"));
                Ok(())
            })
        }
        BtAction::Disconnect { address } => {
            let res = client.cmd("BtDisconnect", json!({ "address": address })).await?;
            output::handle_format(&res, json_mode, |_| {
                output::print_ok(&format!("Disconnected {address}"));
                Ok(())
            })
        }
        BtAction::Power { state } => {
            let on = state == "on";
            let res = client.cmd("BtPower", json!({ "enabled": on })).await?;
            output::handle_format(&res, json_mode, |_| {
                output::print_ok(&format!("Adapter powered {state}"));
                Ok(())
            })
        }
        BtAction::Scan => {
            let res = client.cmd("BtScan", json!({})).await?;
            output::handle_format(&res, json_mode, |_| {
                output::print_ok("Scan started");
                Ok(())
            })
        }
        BtAction::Discoverable { state } => {
            let on = state == "on";
            let res = client.cmd("BtDiscoverable", json!({ "on": on })).await?;
            output::handle_format(&res, json_mode, |_| {
                output::print_ok(&format!("Discoverable {state}"));
                Ok(())
            })
        }
        BtAction::Pairable { state } => {
            let on = state == "on";
            let res = client.cmd("BtPairable", json!({ "on": on })).await?;
            output::handle_format(&res, json_mode, |_| {
                output::print_ok(&format!("Pairable {state}"));
                Ok(())
            })
        }
        BtAction::Alias { address, alias } => {
            let res = client.cmd("BtAlias", json!({ "address": address, "alias": alias })).await?;
            output::handle_format(&res, json_mode, |_| {
                output::print_ok(&format!("Set {address} alias to '{alias}'"));
                Ok(())
            })
        }
        BtAction::Pair { address } => {
            let res = client.cmd("BtPair", json!({ "address": address })).await?;
            output::handle_format(&res, json_mode, |_| {
                output::print_ok(&format!("Pairing with {address}"));
                Ok(())
            })
        }
        BtAction::Trust { address, trusted } => {
            let trusted_bool = trusted == "on";
            let res = client.cmd("BtTrust", json!({ "address": address, "trusted": trusted_bool })).await?;
            output::handle_format(&res, json_mode, |_| {
                output::print_ok(&format!("{address} trust: {}", if trusted_bool { "enabled" } else { "disabled" }));
                Ok(())
            })
        }
        BtAction::Remove { address } => {
            let res = client.cmd("BtRemove", json!({ "address": address })).await?;
            output::handle_format(&res, json_mode, |_| {
                output::print_ok(&format!("Removed {address}"));
                Ok(())
            })
        }
        BtAction::Status => {
            let res = client.cmd("BtStatus", json!({})).await?;
            output::handle_format(&res, json_mode, |val| {
                let powered    = val["powered"].as_bool().unwrap_or(false);
                let discovering = val["discovering"].as_bool().unwrap_or(false);
                println!("Bluetooth");
                let headers = vec!["Property".to_string(), "Value".to_string()];
                let rows = vec![
                    vec!["Powered".to_string(), powered.to_string()],
                    vec!["Discovering".to_string(), discovering.to_string()],
                ];
                let renderable = CliRenderable::new(headers, rows);
                output::render_table(&renderable);

                if let Some(devices) = val["devices"].as_array() {
                    if devices.is_empty() {
                        println!("  no devices");
                    } else {
                        let headers = vec![
                            "Address".to_string(),
                            "Name".to_string(),
                            "Connected".to_string(),
                            "Paired".to_string(),
                            "Battery".to_string(),
                        ];
                        let rows: Vec<Vec<String>> = devices
                            .iter()
                            .map(|d| {
                                let addr   = d["address"].as_str().unwrap_or("?");
                                let name   = d["name"].as_str().unwrap_or("(unknown)");
                                let conn   = d["connected"].as_bool().unwrap_or(false);
                                let paired = d["paired"].as_bool().unwrap_or(false);
                                let bat    = d["battery"].as_u64().map(|b| format!("{b}%")).unwrap_or_else(|| "—".into());
                                vec![
                                    addr.to_string(),
                                    name.to_string(),
                                    if conn { "Yes".to_string() } else { "No".to_string() },
                                    if paired { "Yes".to_string() } else { "No".to_string() },
                                    bat,
                                ]
                            })
                            .collect();
                        let renderable = CliRenderable::new(headers, rows);
                        output::render_table(&renderable);
                    }
                }
                Ok(())
            })
        }
        BtAction::List => {
            let res = client.cmd("BtStatus", json!({})).await?;
            output::handle_format(&res, json_mode, |val| {
                if let Some(devices) = val["devices"].as_array() {
                    if devices.is_empty() {
                        println!("  no devices");
                    } else {
                        let headers = vec![
                            "Address".to_string(),
                            "Name".to_string(),
                            "Connected".to_string(),
                            "Paired".to_string(),
                            "Battery".to_string(),
                        ];
                        let rows: Vec<Vec<String>> = devices
                            .iter()
                            .map(|d| {
                                let addr   = d["address"].as_str().unwrap_or("?");
                                let name   = d["name"].as_str().unwrap_or("(unknown)");
                                let conn   = d["connected"].as_bool().unwrap_or(false);
                                let paired = d["paired"].as_bool().unwrap_or(false);
                                let bat    = d["battery"].as_u64().map(|b| format!("{b}%")).unwrap_or_else(|| "—".into());
                                vec![
                                    addr.to_string(),
                                    name.to_string(),
                                    if conn { "Yes".to_string() } else { "No".to_string() },
                                    if paired { "Yes".to_string() } else { "No".to_string() },
                                    bat,
                                ]
                            })
                            .collect();
                        let renderable = CliRenderable::new(headers, rows);
                        output::render_table(&renderable);
                    }
                }
                Ok(())
            })
        }
    }
}