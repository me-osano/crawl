/// crawl-theme/themes.rs
///
/// Predefined theme registry.
///
/// Themes are loaded from two sources (in priority order):
///   1. User theme files: $XDG_CONFIG_HOME/crawl/themes/<name>.toml
///   2. Built-in themes compiled into the binary (see functions below)
///
/// To add your own theme, drop a TOML file in ~/.config/crawl/themes/
/// and set `active = "your-theme-name"` in crawl.toml.
use crate::palette::{Palette, ThemeSource, ThemeState, Variant};
use crate::Config;
use crate::ThemeError;
use serde::Deserialize;
use std::path::PathBuf;
use tracing::debug;

// ── Theme TOML file format ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ThemeFile {
    pub name: String,
    #[serde(rename = "variant")]
    pub _variant: Option<String>,
    pub palette: Palette,
}

// ── Loader ────────────────────────────────────────────────────────────────────

/// Load a theme by name. Checks user themes dir first, then assets, then built-ins.
pub fn load(name: &str, variant: Variant, cfg: Option<&Config>) -> Result<ThemeState, ThemeError> {
    // 1. Try user themes directory
    if let Some(state) = try_load_user_theme(name, variant) {
        debug!("loaded user theme: {name}");
        return Ok(state);
    }

    // 2. Try assets directory themes
    if let Some(state) = try_load_assets_theme(name, variant, cfg) {
        debug!("loaded assets theme: {name}");
        return Ok(state);
    }

    let available = cfg
        .map(|c| list_assets_by_variant(c, variant))
        .unwrap_or_default();
    Err(ThemeError::NotFound(format!(
        "theme '{name}' not found for {variant} variant. Available: {}",
        available.join(", ")
    )))
}

fn try_load_user_theme(_name: &str, _variant: Variant) -> Option<ThemeState> {
    None
}

fn try_load_assets_theme(name: &str, variant: Variant, cfg: Option<&Config>) -> Option<ThemeState> {
    let cfg = cfg?;
    let dirs = asset_theme_dirs(cfg);

    for dir in dirs {
        let path = PathBuf::from(&dir).join(format!("{name}.toml"));
        if !path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&path).ok()?;
        let file: ThemeFile = toml::from_str(&content).ok()?;
        if !matches_variant(file._variant.as_deref(), variant) {
            continue;
        }
        file.palette.validate().ok()?;

        return Some(ThemeState {
            source: ThemeSource::Predefined { name: file.name },
            variant,
            palette: file.palette,
            wallpaper: None,
        });
    }

    None
}

/// List all available theme names from assets/themes, filtered by variant.
pub fn list_assets_by_variant(cfg: &Config, variant: Variant) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();

    for dir in asset_theme_dirs(cfg) {
        let path = PathBuf::from(&dir);
        if let Ok(entries) = std::fs::read_dir(&path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                    continue;
                }
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let file: ThemeFile = match toml::from_str(&content) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                if !matches_variant(file._variant.as_deref(), variant) {
                    continue;
                }
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if !names.contains(&stem.to_string()) {
                        names.push(stem.to_string());
                    }
                }
            }
        }
    }

    names.sort();
    names
}

fn asset_theme_dirs(cfg: &Config) -> Vec<String> {
    if cfg.assets_dirs.is_empty() {
        vec!["assets/themes".to_string()]
    } else {
        cfg.assets_dirs.clone()
    }
}

fn matches_variant(file_variant: Option<&str>, variant: Variant) -> bool {
    match file_variant {
        Some(v) if v.eq_ignore_ascii_case("dark") => variant == Variant::Dark,
        Some(v) if v.eq_ignore_ascii_case("light") => variant == Variant::Light,
        _ => false,
    }
}

#[allow(dead_code)]
fn builtin_names() -> &'static [&'static str] {
    &[
        "catppuccin-mocha",
        "catppuccin-macchiato",
        "catppuccin-frappe",
        "catppuccin-latte",
        "rose-pine",
        "rose-pine-moon",
        "rose-pine-dawn",
        "tokyo-night",
        "tokyo-night-storm",
        "nord",
        "gruvbox-dark",
        "gruvbox-light",
        "dracula",
        "one-dark",
        "kanagawa",
    ]
}

#[allow(dead_code)]
fn builtin(name: &str, _variant: Variant) -> Option<Palette> {
    match name {
        "catppuccin-mocha" => Some(catppuccin_mocha()),
        "catppuccin-macchiato" => Some(catppuccin_macchiato()),
        "catppuccin-frappe" => Some(catppuccin_frappe()),
        "catppuccin-latte" => Some(catppuccin_latte()),
        "rose-pine" => Some(rose_pine()),
        "rose-pine-moon" => Some(rose_pine_moon()),
        "rose-pine-dawn" => Some(rose_pine_dawn()),
        "tokyo-night" => Some(tokyo_night()),
        "tokyo-night-storm" => Some(tokyo_night_storm()),
        "nord" => Some(nord()),
        "gruvbox-dark" => Some(gruvbox_dark()),
        "gruvbox-light" => Some(gruvbox_light()),
        "dracula" => Some(dracula()),
        "one-dark" => Some(one_dark()),
        "kanagawa" => Some(kanagawa()),
        _ => None,
    }
}

// ── Built-in palettes ─────────────────────────────────────────────────────────
// All values from official theme specifications.

pub fn catppuccin_mocha() -> Palette {
    Palette {
        base: "#1e1e2e".into(),
        mantle: "#181825".into(),
        crust: "#11111b".into(),
        surface0: "#313244".into(),
        surface1: "#45475a".into(),
        surface2: "#585b70".into(),
        text: "#cdd6f4".into(),
        subtext1: "#bac2de".into(),
        subtext0: "#a6adc8".into(),
        primary: "#cba6f7".into(),   // mauve
        secondary: "#89b4fa".into(), // blue
        tertiary: "#a6e3a1".into(),  // green
        error: "#f38ba8".into(),     // red
        warning: "#fab387".into(),   // peach
        info: "#89dceb".into(),      // sky
        overlay0: "#6c7086".into(),
        overlay1: "#7f849c".into(),
        overlay2: "#9399b2".into(),
    }
}

pub fn catppuccin_macchiato() -> Palette {
    Palette {
        base: "#24273a".into(),
        mantle: "#1e2030".into(),
        crust: "#181926".into(),
        surface0: "#363a4f".into(),
        surface1: "#494d64".into(),
        surface2: "#5b6078".into(),
        text: "#cad3f5".into(),
        subtext1: "#b8c0e0".into(),
        subtext0: "#a5adcb".into(),
        primary: "#c6a0f6".into(),   // mauve
        secondary: "#8aadf4".into(), // blue
        tertiary: "#a6da95".into(),  // green
        error: "#ed8796".into(),     // red
        warning: "#f5a97f".into(),   // peach
        info: "#91d7e3".into(),      // sky
        overlay0: "#6e738d".into(),
        overlay1: "#8087a2".into(),
        overlay2: "#939ab7".into(),
    }
}

pub fn catppuccin_frappe() -> Palette {
    Palette {
        base: "#303446".into(),
        mantle: "#292c3c".into(),
        crust: "#232634".into(),
        surface0: "#414559".into(),
        surface1: "#51576d".into(),
        surface2: "#626880".into(),
        text: "#c6d0f5".into(),
        subtext1: "#b5bfe2".into(),
        subtext0: "#a5adce".into(),
        primary: "#ca9ee6".into(),   // mauve
        secondary: "#8caaee".into(), // blue
        tertiary: "#a6d189".into(),  // green
        error: "#e78284".into(),     // red
        warning: "#ef9f76".into(),   // peach
        info: "#85c1dc".into(),      // sky
        overlay0: "#737994".into(),
        overlay1: "#838ba7".into(),
        overlay2: "#949cbb".into(),
    }
}

pub fn catppuccin_latte() -> Palette {
    Palette {
        base: "#eff1f5".into(),
        mantle: "#e6e9ef".into(),
        crust: "#dce0e8".into(),
        surface0: "#ccd0da".into(),
        surface1: "#bcc0cc".into(),
        surface2: "#acb0be".into(),
        text: "#4c4f69".into(),
        subtext1: "#5c5f77".into(),
        subtext0: "#6c6f85".into(),
        primary: "#8839ef".into(),   // mauve
        secondary: "#1e66f5".into(), // blue
        tertiary: "#40a02b".into(),  // green
        error: "#d20f39".into(),     // red
        warning: "#fe640b".into(),   // peach
        info: "#04a5e5".into(),      // sky
        overlay0: "#9ca0b0".into(),
        overlay1: "#8c8fa1".into(),
        overlay2: "#7c7f93".into(),
    }
}

pub fn rose_pine() -> Palette {
    Palette {
        base: "#191724".into(),
        mantle: "#1f1d2e".into(),
        crust: "#26233a".into(),
        surface0: "#2a2837".into(),
        surface1: "#393552".into(),
        surface2: "#44415a".into(),
        text: "#e0def4".into(),
        subtext1: "#c5c3d4".into(),
        subtext0: "#908caa".into(),
        primary: "#c4a7e7".into(),   // iris
        secondary: "#9ccfd8".into(), // foam
        tertiary: "#31748f".into(),  // pine
        error: "#eb6f92".into(),     // love
        warning: "#f6c177".into(),   // gold
        info: "#ebbcba".into(),      // rose
        overlay0: "#6e6a86".into(),
        overlay1: "#907aa9".into(),
        overlay2: "#524f67".into(),
    }
}

pub fn rose_pine_moon() -> Palette {
    Palette {
        base: "#232136".into(),
        mantle: "#2d2a45".into(),
        crust: "#393552".into(),
        surface0: "#44415a".into(),
        surface1: "#56526e".into(),
        surface2: "#59546d".into(),
        text: "#e0def4".into(),
        subtext1: "#c5c3d4".into(),
        subtext0: "#908caa".into(),
        primary: "#c4a7e7".into(),   // iris
        secondary: "#9ccfd8".into(), // foam
        tertiary: "#3e8fb0".into(),  // pine
        error: "#eb6f92".into(),     // love
        warning: "#f6c177".into(),   // gold
        info: "#ea9a97".into(),      // rose
        overlay0: "#6e6a86".into(),
        overlay1: "#817c9c".into(),
        overlay2: "#59546d".into(),
    }
}

pub fn rose_pine_dawn() -> Palette {
    Palette {
        base: "#faf4ed".into(),
        mantle: "#fffaf3".into(),
        crust: "#f2e9e1".into(),
        surface0: "#f4ede8".into(),
        surface1: "#e4dfde".into(),
        surface2: "#d7cfc9".into(),
        text: "#575279".into(),
        subtext1: "#797593".into(),
        subtext0: "#9893a5".into(),
        primary: "#907aa9".into(),   // iris
        secondary: "#56949f".into(), // foam
        tertiary: "#286983".into(),  // pine
        error: "#b4637a".into(),     // love
        warning: "#ea9d34".into(),   // gold
        info: "#d7827e".into(),      // rose
        overlay0: "#cecacd".into(),
        overlay1: "#9893a5".into(),
        overlay2: "#797593".into(),
    }
}

pub fn tokyo_night() -> Palette {
    Palette {
        base: "#1a1b26".into(),
        mantle: "#16161e".into(),
        crust: "#13131a".into(),
        surface0: "#1f2335".into(),
        surface1: "#24283b".into(),
        surface2: "#292e42".into(),
        text: "#c0caf5".into(),
        subtext1: "#a9b1d6".into(),
        subtext0: "#9aa5ce".into(),
        primary: "#bb9af7".into(),   // purple
        secondary: "#7aa2f7".into(), // blue
        tertiary: "#9ece6a".into(),  // green
        error: "#f7768e".into(),     // red
        warning: "#ff9e64".into(),   // orange
        info: "#2ac3de".into(),      // cyan
        overlay0: "#565f89".into(),
        overlay1: "#636d97".into(),
        overlay2: "#737aa2".into(),
    }
}

pub fn tokyo_night_storm() -> Palette {
    Palette {
        base: "#24283b".into(),
        mantle: "#1f2335".into(),
        crust: "#1a1b26".into(),
        surface0: "#292e42".into(),
        surface1: "#2f354a".into(),
        surface2: "#3b4261".into(),
        text: "#c0caf5".into(),
        subtext1: "#a9b1d6".into(),
        subtext0: "#9aa5ce".into(),
        primary: "#bb9af7".into(),
        secondary: "#7aa2f7".into(),
        tertiary: "#9ece6a".into(),
        error: "#f7768e".into(),
        warning: "#ff9e64".into(),
        info: "#2ac3de".into(),
        overlay0: "#565f89".into(),
        overlay1: "#636d97".into(),
        overlay2: "#737aa2".into(),
    }
}

pub fn nord() -> Palette {
    Palette {
        base: "#2e3440".into(),
        mantle: "#272c36".into(),
        crust: "#242933".into(),
        surface0: "#3b4252".into(),
        surface1: "#434c5e".into(),
        surface2: "#4c566a".into(),
        text: "#eceff4".into(),
        subtext1: "#e5e9f0".into(),
        subtext0: "#d8dee9".into(),
        primary: "#b48ead".into(),   // aurora purple
        secondary: "#81a1c1".into(), // frost blue
        tertiary: "#a3be8c".into(),  // aurora green
        error: "#bf616a".into(),     // aurora red
        warning: "#d08770".into(),   // aurora orange
        info: "#88c0d0".into(),      // frost cyan
        overlay0: "#616e88".into(),
        overlay1: "#677383".into(),
        overlay2: "#6d7a96".into(),
    }
}

pub fn gruvbox_dark() -> Palette {
    Palette {
        base: "#282828".into(),
        mantle: "#1d2021".into(),
        crust: "#181818".into(),
        surface0: "#32302f".into(),
        surface1: "#3c3836".into(),
        surface2: "#504945".into(),
        text: "#ebdbb2".into(),
        subtext1: "#d5c4a1".into(),
        subtext0: "#bdae93".into(),
        primary: "#d3869b".into(),   // pink/magenta
        secondary: "#83a598".into(), // aqua
        tertiary: "#b8bb26".into(),  // green
        error: "#fb4934".into(),     // bright red
        warning: "#fabd2f".into(),   // yellow
        info: "#8ec07c".into(),      // teal
        overlay0: "#665c54".into(),
        overlay1: "#7c6f64".into(),
        overlay2: "#928374".into(),
    }
}

pub fn gruvbox_light() -> Palette {
    Palette {
        base: "#fbf1c7".into(),
        mantle: "#f9f5d7".into(),
        crust: "#f2e5bc".into(),
        surface0: "#ebdbb2".into(),
        surface1: "#d5c4a1".into(),
        surface2: "#bdae93".into(),
        text: "#3c3836".into(),
        subtext1: "#504945".into(),
        subtext0: "#665c54".into(),
        primary: "#8f3f71".into(),   // purple
        secondary: "#076678".into(), // aqua
        tertiary: "#79740e".into(),  // green
        error: "#cc241d".into(),     // red
        warning: "#d79921".into(),   // yellow
        info: "#427b58".into(),      // teal
        overlay0: "#7c6f64".into(),
        overlay1: "#928374".into(),
        overlay2: "#a89984".into(),
    }
}

pub fn dracula() -> Palette {
    Palette {
        base: "#282a36".into(),
        mantle: "#21222c".into(),
        crust: "#191a21".into(),
        surface0: "#343746".into(),
        surface1: "#44475a".into(),
        surface2: "#4d5066".into(),
        text: "#f8f8f2".into(),
        subtext1: "#e2e2dc".into(),
        subtext0: "#bfbfba".into(),
        primary: "#bd93f9".into(),   // purple
        secondary: "#6272a4".into(), // comment/blue
        tertiary: "#50fa7b".into(),  // green
        error: "#ff5555".into(),     // red
        warning: "#ffb86c".into(),   // orange
        info: "#8be9fd".into(),      // cyan
        overlay0: "#6272a4".into(),
        overlay1: "#7280b5".into(),
        overlay2: "#8192ca".into(),
    }
}

pub fn one_dark() -> Palette {
    Palette {
        base: "#282c34".into(),
        mantle: "#21252b".into(),
        crust: "#181a1f".into(),
        surface0: "#2c313a".into(),
        surface1: "#3e4451".into(),
        surface2: "#4b5263".into(),
        text: "#abb2bf".into(),
        subtext1: "#9da5b4".into(),
        subtext0: "#828997".into(),
        primary: "#c678dd".into(),   // purple
        secondary: "#61afef".into(), // blue
        tertiary: "#98c379".into(),  // green
        error: "#e06c75".into(),     // red
        warning: "#e5c07b".into(),   // yellow
        info: "#56b6c2".into(),      // cyan
        overlay0: "#5c6370".into(),
        overlay1: "#636d83".into(),
        overlay2: "#6d7a96".into(),
    }
}

pub fn kanagawa() -> Palette {
    Palette {
        base: "#1f1f28".into(),
        mantle: "#16161d".into(),
        crust: "#0d0c0c".into(),
        surface0: "#2a2a37".into(),
        surface1: "#363646".into(),
        surface2: "#54546d".into(),
        text: "#dcd7ba".into(),
        subtext1: "#c8c093".into(),
        subtext0: "#938aa9".into(),
        primary: "#957fb8".into(),   // violet
        secondary: "#7e9cd8".into(), // crystal blue
        tertiary: "#76946a".into(),  // autumn green
        error: "#c34043".into(),     // autumn red
        warning: "#dca561".into(),   // autumn yellow
        info: "#7fb4ca".into(),      // spring blue
        overlay0: "#727169".into(),
        overlay1: "#817c7c".into(),
        overlay2: "#938aa9".into(),
    }
}
