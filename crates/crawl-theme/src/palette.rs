/// crawl-theme: palette.rs
///
/// The canonical Palette struct. Every theme source — predefined TOML files
/// or matugen dynamic generation — produces this exact structure.
/// Quickshell and all config writers consume only this type.
use serde::{Deserialize, Serialize};

/// 18 semantic color roles. Names describe PURPOSE, not appearance.
/// This is the single source of truth consumed by every writer and QML.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Palette {
    // ── Surfaces (backgrounds, elevated layers) ───────────────────────────
    /// Main background — the darkest/lightest base surface
    pub base: String,
    /// Slightly darker/lighter than base — used for sidebars, panels
    pub mantle: String,
    /// Darkest/lightest surface — window chrome, borders
    pub crust: String,
    /// Slightly elevated surface — cards, inputs
    pub surface0: String,
    /// More elevated surface — hover states, selected items
    pub surface1: String,
    /// Most elevated surface — active/pressed states
    pub surface2: String,

    // ── Text ─────────────────────────────────────────────────────────────
    /// Primary text — body copy, labels
    pub text: String,
    /// Secondary text — captions, descriptions
    pub subtext1: String,
    /// Tertiary text — placeholders, disabled labels
    pub subtext0: String,

    // ── Accents ───────────────────────────────────────────────────────────
    /// Primary accent — active elements, focus rings, primary buttons
    pub primary: String,
    /// Secondary accent — links, info highlights
    pub secondary: String,
    /// Positive / success — confirmations, active connections
    pub tertiary: String,
    /// Error / danger — destructive actions, disconnections, critical battery
    pub error: String,
    /// Warning — low battery, caution states
    pub warning: String,
    /// Informational — notifications, tips
    pub info: String,

    // ── Overlays (for borders, separators, subtle UI) ─────────────────────
    /// Subtle overlay — separators, inactive borders
    pub overlay0: String,
    /// Medium overlay — inactive tab text, secondary icons
    pub overlay1: String,
    /// Stronger overlay — active borders, focused input outlines
    pub overlay2: String,
}

impl Palette {
    /// Validate that all hex color strings are well-formed (#rrggbb or #rgb).
    pub fn validate(&self) -> Result<(), String> {
        let fields = [
            ("base", &self.base),
            ("mantle", &self.mantle),
            ("crust", &self.crust),
            ("surface0", &self.surface0),
            ("surface1", &self.surface1),
            ("surface2", &self.surface2),
            ("text", &self.text),
            ("subtext1", &self.subtext1),
            ("subtext0", &self.subtext0),
            ("primary", &self.primary),
            ("secondary", &self.secondary),
            ("tertiary", &self.tertiary),
            ("error", &self.error),
            ("warning", &self.warning),
            ("info", &self.info),
            ("overlay0", &self.overlay0),
            ("overlay1", &self.overlay1),
            ("overlay2", &self.overlay2),
        ];
        for (name, val) in &fields {
            if !is_valid_hex(val) {
                return Err(format!("palette.{name}: invalid hex color {val:?}"));
            }
        }
        Ok(())
    }

    /// Return a vec of (role_name, hex_value) pairs — useful for template rendering.
    pub fn as_pairs(&self) -> Vec<(&'static str, &str)> {
        vec![
            ("base", &self.base),
            ("mantle", &self.mantle),
            ("crust", &self.crust),
            ("surface0", &self.surface0),
            ("surface1", &self.surface1),
            ("surface2", &self.surface2),
            ("text", &self.text),
            ("subtext1", &self.subtext1),
            ("subtext0", &self.subtext0),
            ("primary", &self.primary),
            ("secondary", &self.secondary),
            ("tertiary", &self.tertiary),
            ("error", &self.error),
            ("warning", &self.warning),
            ("info", &self.info),
            ("overlay0", &self.overlay0),
            ("overlay1", &self.overlay1),
            ("overlay2", &self.overlay2),
        ]
    }

    /// Strip '#' prefix for formats that expect bare hex (e.g. Ghostty, some CSS)
    pub fn bare<'a>(&self, hex: &'a str) -> &'a str {
        hex.trim_start_matches('#')
    }
}

fn is_valid_hex(s: &str) -> bool {
    let s = s.trim_start_matches('#');
    (s.len() == 6 || s.len() == 3) && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Light or dark variant — affects both predefined theme selection
/// and matugen's output mode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Variant {
    #[default]
    Dark,
    Light,
}

impl std::fmt::Display for Variant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Variant::Dark => write!(f, "dark"),
            Variant::Light => write!(f, "light"),
        }
    }
}

/// What generated the current palette.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ThemeSource {
    /// A named predefined theme loaded from a TOML file.
    Predefined { name: String },
    /// Generated by matugen from a wallpaper image.
    Dynamic { wallpaper: String },
}

/// Full theme state — broadcast in ThemeEvent::PaletteChanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeState {
    pub source: ThemeSource,
    pub variant: Variant,
    pub palette: Palette,
    pub wallpaper: Option<String>,
}
