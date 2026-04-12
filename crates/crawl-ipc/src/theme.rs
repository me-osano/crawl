use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Palette {
    pub base: String,
    pub mantle: String,
    pub crust: String,
    pub surface0: String,
    pub surface1: String,
    pub surface2: String,
    pub text: String,
    pub subtext1: String,
    pub subtext0: String,
    pub primary: String,
    pub secondary: String,
    pub tertiary: String,
    pub error: String,
    pub warning: String,
    pub info: String,
    pub overlay0: String,
    pub overlay1: String,
    pub overlay2: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Variant {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ThemeSource {
    Predefined { name: String },
    Dynamic { wallpaper: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeState {
    pub source: ThemeSource,
    pub variant: Variant,
    pub palette: Palette,
    pub wallpaper: Option<String>,
}
