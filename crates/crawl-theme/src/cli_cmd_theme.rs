/// crawl-cli/src/cmd/theme.rs
///
/// Theme management CLI subcommand.
/// Add to crawl-cli/src/cmd/mod.rs and wire in main.rs.

use clap::Args;
use anyhow::Result;
use serde_json::json;
use crate::{client::CrawlClient, output};

#[derive(Args)]
pub struct ThemeArgs {
    /// Switch to a named predefined theme
    #[arg(long, value_name = "NAME")]
    pub set: Option<String>,

    /// Set wallpaper and generate a dynamic palette via matugen
    #[arg(long, value_name = "PATH")]
    pub wallpaper: Option<String>,

    /// Set wallpaper without running matugen (keep current palette)
    #[arg(long)]
    pub no_generate: bool,

    /// Switch variant to dark
    #[arg(long, conflicts_with = "light")]
    pub dark: bool,

    /// Switch variant to light
    #[arg(long, conflicts_with = "dark")]
    pub light: bool,

    /// Force regenerate dynamic palette from current wallpaper
    #[arg(long)]
    pub regenerate: bool,

    /// List all available themes (built-ins + user themes)
    #[arg(long)]
    pub list: bool,

    /// Show current theme status
    #[arg(long)]
    pub status: bool,
}

pub async fn run(client: CrawlClient, args: ThemeArgs, json: bool) -> Result<()> {
    if let Some(name) = args.set {
        let res = client.post("/theme/set", json!({ "name": name })).await?;
        if json {
            output::print_value(&res, true);
        } else {
            output::print_ok(&format!("Theme set to '{name}'"));
            print_palette_preview(&res);
        }
    } else if let Some(path) = args.wallpaper {
        let res = client.post("/theme/wallpaper", json!({
            "path":        path,
            "no_generate": args.no_generate,
        })).await?;
        if json {
            output::print_value(&res, true);
        } else {
            if args.no_generate {
                output::print_ok(&format!("Wallpaper set to '{path}'"));
            } else {
                output::print_ok(&format!("Wallpaper set — generating palette from '{path}'..."));
                println!("  Use `crawl theme --status` to see the result.");
            }
        }
    } else if args.dark {
        let res = client.post("/theme/variant", json!({ "variant": "dark" })).await?;
        if json { output::print_value(&res, true); } else { output::print_ok("Switched to dark variant"); }
    } else if args.light {
        let res = client.post("/theme/variant", json!({ "variant": "light" })).await?;
        if json { output::print_value(&res, true); } else { output::print_ok("Switched to light variant"); }
    } else if args.regenerate {
        let res = client.post("/theme/regenerate", json!({})).await?;
        if json { output::print_value(&res, true); } else { output::print_ok("Regenerating dynamic palette..."); }
    } else if args.list {
        let res = client.get("/theme/list").await?;
        if json {
            output::print_value(&res, true);
        } else if let Some(themes) = res["themes"].as_array() {
            println!("Available themes:");
            for t in themes {
                let name     = t["name"].as_str().unwrap_or("?");
                let builtin  = t["builtin"].as_bool().unwrap_or(false);
                let marker   = if builtin { "  " } else { "* " };
                println!("{marker}{name}");
            }
            println!();
            println!("  * = user theme from ~/.config/crawl/themes/");
        }
    } else {
        // Default: show status
        let res = client.get("/theme/status").await?;
        if json {
            output::print_value(&res, true);
        } else {
            let source  = if let Some(name) = res["source"]["predefined"]["name"].as_str() {
                format!("predefined: {name}")
            } else if let Some(wp) = res["source"]["dynamic"]["wallpaper"].as_str() {
                format!("dynamic: {wp}")
            } else {
                "unknown".into()
            };
            let variant   = res["variant"].as_str().unwrap_or("?");
            let wallpaper = res["wallpaper"].as_str().unwrap_or("none");

            println!("Theme");
            output::print_table(&[
                ("source",    source),
                ("variant",   variant.to_string()),
                ("wallpaper", wallpaper.to_string()),
            ]);

            println!();
            print_palette_preview(&res);
        }
    }
    Ok(())
}

/// Print a compact color swatch preview using ANSI 24-bit truecolor escape codes.
/// Falls back gracefully in terminals that don't support it.
fn print_palette_preview(res: &serde_json::Value) {
    let pal = &res["palette"];
    if pal.is_null() { return; }

    let roles = [
        ("base",      "base"),
        ("surface0",  "surf"),
        ("primary",   "pri "),
        ("secondary", "sec "),
        ("tertiary",  "ter "),
        ("error",     "err "),
        ("warning",   "warn"),
        ("text",      "text"),
    ];

    print!("  ");
    for (role, label) in &roles {
        if let Some(hex) = pal[role].as_str() {
            if let Some((r, g, b)) = parse_hex_rgb(hex) {
                // Background swatch + label
                print!("\x1b[48;2;{r};{g};{b}m  {label}  \x1b[0m ");
            }
        }
    }
    println!();
}

fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 { return None; }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some((r, g, b))
}
