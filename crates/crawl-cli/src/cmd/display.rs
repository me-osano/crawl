use anyhow::Result;
use clap::Parser;
use serde_json::Value;
use crate::{CrawlClient, output::{self, CliRenderable}};

#[derive(Parser, Debug)]
pub struct DisplayArgs {
    #[command(subcommand)]
    pub action: DisplayAction,
}

#[derive(Parser, Debug)]
pub enum DisplayAction {
    /// Show all brightness info
    Brightness,
    /// Set brightness (0-100)
    BrightnessSet { value: u32 },
    /// Increase brightness
    BrightnessInc { value: i32 },
    /// Decrease brightness
    BrightnessDec { value: i32 },
    /// Get wallpaper status
    Wallpaper,
    /// Set wallpaper
    WallpaperSet {
        path: String,
        /// Monitor to set wallpaper on (default: all monitors)
        #[arg(short, long)]
        monitor: Option<String>,
        /// Wallpaper mode: fill, fit, stretch, center, tile
        #[arg(long, default_value = "fill")]
        mode: String,
        /// Transition type: fade, wipe, wave, center, outer, random, none
        #[arg(short, long)]
        transition: Option<String>,
        /// Transition FPS (frames per second)
        #[arg(long, default_value = "30")]
        fps: u32,
    },
    /// Get wallpaper
    WallpaperGet {
        /// Monitor to get wallpaper for (default: global wallpaper)
        #[arg(short, long)]
        monitor: Option<String>,
    },
}

pub async fn run(client: CrawlClient, args: DisplayArgs, json_mode: bool) -> Result<()> {
    match &args.action {
        DisplayAction::Brightness => {
            let response: Value = client.cmd("BrightnessGet", Value::Null).await?;
            output::handle_format(&response, json_mode, |_| {
                print_brightness_table(&response);
                Ok(())
            })
        }
        DisplayAction::BrightnessSet { value } => {
            let response: Value = client.cmd("BrightnessSet", serde_json::json!({ "value": *value as i32 })).await?;
            output::handle_format(&response, json_mode, |_| {
                print_brightness_table(&response);
                Ok(())
            })
        }
        DisplayAction::BrightnessInc { value } => {
            let response: Value = client.cmd("BrightnessInc", serde_json::json!({ "value": *value })).await?;
            output::handle_format(&response, json_mode, |_| {
                print_brightness_table(&response);
                Ok(())
            })
        }
        DisplayAction::BrightnessDec { value } => {
            let response: Value = client.cmd("BrightnessDec", serde_json::json!({ "value": *value })).await?;
            output::handle_format(&response, json_mode, |_| {
                print_brightness_table(&response);
                Ok(())
            })
        }
        DisplayAction::Wallpaper => {
            let response: Value = client.cmd("WallpaperStatus", Value::Null).await?;
            output::handle_format(&response, json_mode, |_| {
                print_wallpaper_table(&response);
                Ok(())
            })
        }
        DisplayAction::WallpaperSet { path, monitor, mode, transition, fps: _ } => {
            let mut params = serde_json::json!({ "path": path, "mode": mode });
            if let Some(m) = monitor {
                params["monitor"] = serde_json::json!(m);
            }
            if let Some(t) = transition {
                params["transition"] = serde_json::json!(t);
            }
            let response: Value = client.cmd("WallpaperSet", params).await?;
            output::handle_format(&response, json_mode, |_| {
                println!("Wallpaper set to: {} (mode: {})", path, mode);
                Ok(())
            })
        }
        DisplayAction::WallpaperGet { monitor } => {
            let monitor_str = monitor.as_deref().unwrap_or("*");
            let params = serde_json::json!({ "monitor": monitor_str });
            let response: Value = client.cmd("WallpaperGet", params).await?;
            output::handle_format(&response, json_mode, |val| {
                if let Some(result) = val.get("result") {
                    if let Some(wp) = result.get("wallpaper").and_then(|v| v.as_str()) {
                        println!("{}", wp);
                    }
                }
                Ok(())
            })
        }
    }
}

fn print_brightness_table(response: &Value) {
    if let Some(result) = response.get("result") {
        let device = result.get("device").and_then(|v| v.as_str()).unwrap_or("-");
        let percent = result.get("percent").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let current = result.get("current").and_then(|v| v.as_u64()).unwrap_or(0);
        let max = result.get("max").and_then(|v| v.as_u64()).unwrap_or(0);

        let headers = vec!["Property".to_string(), "Value".to_string()];
        let rows = vec![
            vec!["Device".to_string(), device.to_string()],
            vec!["Current".to_string(), format!("{:.1}%", percent)],
            vec!["Raw Value".to_string(), format!("{}/{}", current, max)],
        ];

        let renderable = CliRenderable::new(headers, rows);
        output::render_table(&renderable);
    }
}

fn print_wallpaper_table(response: &Value) {
    if let Some(result) = response.get("result") {
        let headers = vec!["Property".to_string(), "Value".to_string()];
        let mut rows: Vec<Vec<String>> = Vec::new();

        if let Some(current) = result.get("current").and_then(|v| v.as_str()) {
            rows.push(vec!["Default".to_string(), truncate(current, 30)]);
        }

        if let Some(mode) = result.get("per_monitor_mode").and_then(|v| v.as_bool()) {
            if mode {
                rows.push(vec!["Mode".to_string(), "Per-monitor".to_string()]);
            }
        }

        if let Some(per_monitor) = result.get("per_monitor").and_then(|v| v.as_object()) {
            if !per_monitor.is_empty() {
                for (monitor, path) in per_monitor {
                    rows.push(vec![
                        monitor.clone(),
                        truncate(path.as_str().unwrap_or("-"), 30),
                    ]);
                }
            }
        }

        if !rows.is_empty() {
            let renderable = CliRenderable::new(headers, rows);
            output::render_table(&renderable);
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max.saturating_sub(1)])
    } else {
        s.to_string()
    }
}
