use anyhow::Result;
use clap::{Args, Subcommand};
use serde_json::json;
use crate::{CrawlClient, output::{self, CliRenderable}};

#[derive(Args)]
pub struct AudioArgs {
    #[command(subcommand)]
    pub action: AudioAction,
}

#[derive(Subcommand)]
pub enum AudioAction {
    /// Show audio status
    Status,
    /// List/set volume (0-100)
    Volume {
        /// Volume level 0-100
        #[arg(long, short = 'v')]
        value: Option<u32>,
    },
    /// List/input control input devices (microphones)
    Input,
    /// Toggle mute
    Mute,
    /// Unmute
    Unmute,
    /// List all devices
    List,
}

pub async fn run(client: CrawlClient, args: AudioArgs, json_mode: bool) -> Result<()> {
    match args.action {
        AudioAction::Status => {
            let action = "sinks";
            let res = client.cmd("Audio", json!({ "action": action })).await?;
            output::handle_format(&res, json_mode, |val| {
                if let Some(devices) = val.as_array() {
                    if let Some(default) = devices.iter().find(|d| d["is_default"].as_bool().unwrap_or(false)) {
                        let name = default["name"].as_str().unwrap_or("-");
                        let vol = default["volume_percent"].as_u64().unwrap_or(0);
                        let muted = default["muted"].as_bool().unwrap_or(false);
                        
                        let headers = vec!["Property".to_string(), "Value".to_string()];
                        let rows = vec![
                            vec!["Default".to_string(), name.to_string()],
                            vec!["Volume".to_string(), format!("{}%", vol)],
                            vec!["Muted".to_string(), muted.to_string()],
                        ];
                        let renderable = CliRenderable::new(headers, rows);
                        output::render_table(&renderable);
                    }
                }
                Ok(())
            })
        }
        AudioAction::Volume { value } => {
            let vol = value.unwrap_or(50);
            let res = client.cmd("Audio", json!({ "action": "volume", "percent": vol })).await?;
            output::handle_format(&res, json_mode, |_| {
                output::print_ok(&format!("Volume set to {}%", vol));
                Ok(())
            })
        }
        AudioAction::Input => {
            let res = client.cmd("Audio", json!({ "action": "sources" })).await?;
            output::handle_format(&res, json_mode, |val| {
                render_audio_table(val, "Input Devices (Microphones)");
                Ok(())
            })
        }
        AudioAction::Mute => {
            let res = client.cmd("Audio", json!({ "action": "mute" })).await?;
            output::handle_format(&res, json_mode, |_| {
                output::print_info("Muted");
                Ok(())
            })
        }
        AudioAction::Unmute => {
            let res = client.cmd("Audio", json!({ "action": "unmute" })).await?;
            output::handle_format(&res, json_mode, |_| {
                output::print_info("Unmuted");
                Ok(())
            })
        }
        AudioAction::List => {
            let res = client.cmd("Audio", json!({ "action": "sinks" })).await?;
            output::handle_format(&res, json_mode, |val| {
                render_audio_table(val, "Output Devices (Speakers)");
                Ok(())
            })
        }
    }
}

fn render_audio_table(val: &serde_json::Value, title: &str) {
    if let Some(devices) = val.as_array() {
        output::print_header(title);
        
        let headers = vec!["Name".to_string(), "Volume".to_string(), "Muted".to_string(), "Default".to_string()];
        let rows: Vec<Vec<String>> = devices
            .iter()
            .map(|dev| {
                let name = dev["name"].as_str().unwrap_or("?");
                let vol = dev["volume_percent"].as_u64().unwrap_or(0);
                let muted = dev["muted"].as_bool().unwrap_or(false);
                let default = dev["is_default"].as_bool().unwrap_or(false);
                vec![
                    name.to_string(),
                    format!("{}%", vol),
                    if muted { "Yes".to_string() } else { "No".to_string() },
                    if default { "✓".to_string() } else { "".to_string() },
                ]
            })
            .collect();
        
        let renderable = CliRenderable::new(headers, rows);
        output::render_table(&renderable);
    }
}