/// writers/json.rs
///
/// Writes the current ThemeState as JSON to ~/.config/crawl/current-palette.json
///
/// This is the catch-all output format. Any tool that can read JSON can consume
/// it — waybar (via custom modules), eww, AGS, scripts, other Rust tools, etc.
///
/// Also writes ~/.config/crawl/current-palette-bare.json with hex values
/// without '#' prefix, for tools that don't want it.
///
/// Output format:
/// {
///   "source": { "predefined": { "name": "catppuccin-mocha" } },
///   "variant": "dark",
///   "wallpaper": null,
///   "palette": {
///     "base": "#1e1e2e",
///     "primary": "#cba6f7",
///     ...
///   },
///   "palette_bare": {
///     "base": "1e1e2e",
///     "primary": "cba6f7",
///     ...
///   }
/// }

use crate::{palette::ThemeState, ThemeError};
use serde_json::{json, Map, Value};
use tracing::info;

pub async fn write(state: &ThemeState) -> Result<(), ThemeError> {
    let p = &state.palette;

    // Build palette objects (with and without # prefix)
    let mut palette_map     = Map::new();
    let mut palette_bare    = Map::new();

    for (role, hex) in p.as_pairs() {
        palette_map.insert(role.to_string(),  Value::String(hex.to_string()));
        palette_bare.insert(role.to_string(), Value::String(hex.trim_start_matches('#').to_string()));
    }

    let output = json!({
        "source":        state.source,
        "variant":       state.variant,
        "wallpaper":     state.wallpaper,
        "generated_by":  "crawl-theme",
        "palette":       Value::Object(palette_map),
        "palette_bare":  Value::Object(palette_bare),
    });

    let json_str = serde_json::to_string_pretty(&output)
        .map_err(|e| ThemeError::Writer(e.to_string()))?;

    let config_home = dirs::config_dir()
        .ok_or_else(|| ThemeError::Writer("cannot determine config dir".into()))?;
    let crawl_dir = config_home.join("crawl");
    tokio::fs::create_dir_all(&crawl_dir).await?;

    tokio::fs::write(crawl_dir.join("current-palette.json"), &json_str).await?;

    info!("json: current-palette.json written");
    Ok(())
}
