/// crawl-theme/matugen.rs
///
/// Runs `matugen image <path> --json hex` and maps Material You color roles
/// to our semantic Palette struct.
///
/// Install matugen from AUR: yay -S matugen
/// or from source: https://github.com/InioX/matugen

use crate::palette::{Palette, Variant};
use crate::ThemeError;
use serde_json::Value;
use tracing::{debug, info};

/// Run matugen against a wallpaper and return a resolved Palette.
pub async fn generate(wallpaper_path: &str, variant: Variant) -> Result<Palette, ThemeError> {
    generate_with_scheme(wallpaper_path, variant, None).await
}

/// Run matugen against a wallpaper with an optional scheme
/// (e.g. tonalspot, vibrant, monochrome, expressive, fidelity, neutral, content).
pub async fn generate_with_scheme(
    wallpaper_path: &str,
    variant: Variant,
    scheme: Option<&str>,
) -> Result<Palette, ThemeError> {
    info!("running matugen on: {wallpaper_path}");

    // Check matugen is installed
    let which = tokio::process::Command::new("which")
        .arg("matugen")
        .output()
        .await;
    if which.map(|o| !o.status.success()).unwrap_or(true) {
        return Err(ThemeError::MatugenMissing);
    }

    // Run matugen: outputs a JSON object with color roles per variant
    let mut cmd = tokio::process::Command::new("matugen");
    cmd.args(["image", wallpaper_path, "--json", "hex"]);
    if let Some(scheme) = scheme {
        cmd.args(["--scheme", scheme]);
    }
    let output = cmd
        .output()
        .await
        .map_err(|e| ThemeError::MatugenFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ThemeError::MatugenFailed(format!(
            "matugen exited {}: {stderr}",
            output.status
        )));
    }

    let json: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| ThemeError::Parse(format!("matugen JSON: {e}")))?;

    debug!("matugen output: {}", json);

    let variant_key = match variant { Variant::Dark => "dark", Variant::Light => "light" };
    let scheme_key = scheme.unwrap_or("default");
    let colors = if json["colors"].get(scheme_key).is_some() {
        &json["colors"][scheme_key][variant_key]
    } else {
        &json["colors"][variant_key]
    };

    if colors.is_null() {
        return Err(ThemeError::Parse(format!(
            "matugen output missing colors.{variant_key}"
        )));
    }

    map_material_you(colors)
}

/// Map Material You color roles to our semantic Palette.
///
/// Material You role reference:
///   https://m3.material.io/styles/color/roles
///
/// This mapping is intentionally opinionated for a dark desktop shell.
/// Adjust to your taste — the semantic names in Palette are what Quickshell binds to.
fn map_material_you(c: &Value) -> Result<Palette, ThemeError> {
    // Helper: extract a hex string from the JSON, return ThemeError if missing
    let get = |key: &str| -> Result<String, ThemeError> {
        c[key]
            .as_str()
            .map(|s| {
                // Ensure # prefix
                if s.starts_with('#') { s.to_string() } else { format!("#{s}") }
            })
            .ok_or_else(|| ThemeError::Parse(format!("matugen: missing role '{key}'")))
    };

    // Optional helper — falls back to a default if role not present
    let get_or = |key: &str, fallback: &str| -> String {
        c[key]
            .as_str()
            .map(|s| if s.starts_with('#') { s.to_string() } else { format!("#{s}") })
            .unwrap_or_else(|| fallback.to_string())
    };

    Ok(Palette {
        // ── Surfaces ──────────────────────────────────────────────────────
        // Material You "surface" = main background
        base:     get("surface")?,
        // "surface_container_lowest" is darker than surface — good for mantle
        mantle:   get_or("surface_container_lowest", &get("surface")?),
        // "surface_dim" or inverse surface — darkest chrome
        crust:    get_or("surface_dim", &get("surface")?),
        // Elevated container surfaces
        surface0: get_or("surface_container", &get("surface_variant")?),
        surface1: get_or("surface_container_high", &get("surface_variant")?),
        surface2: get_or("surface_container_highest", &get("surface_variant")?),

        // ── Text ──────────────────────────────────────────────────────────
        // "on_surface" = primary readable text on surface backgrounds
        text:     get("on_surface")?,
        // "on_surface_variant" = slightly muted text
        subtext1: get_or("on_surface_variant", &get("on_surface")?),
        // "outline" = even more muted — borders, placeholders
        subtext0: get_or("outline", &get("on_surface_variant")?),

        // ── Accents ───────────────────────────────────────────────────────
        // "primary" = the seed color's dominant role — used for primary actions
        primary:   get("primary")?,
        // "secondary" = complementary accent
        secondary: get("secondary")?,
        // "tertiary" = positive/success tone
        tertiary:  get("tertiary")?,
        // "error" = always red-family in Material You
        error:     get("error")?,
        // matugen doesn't always have a "warning" role; derive from error-adjacent
        warning:   get_or("error_container", &get("error")?),
        // "info" — use secondary as info, or override if matugen adds it
        info:      get_or("secondary_container", &get("secondary")?),

        // ── Overlays ──────────────────────────────────────────────────────
        // "outline_variant" = subtle separator / inactive border
        overlay0: get_or("outline_variant", &get("outline")?),
        // "outline" = more visible border / overlay
        overlay1: get_or("outline", &get("on_surface_variant")?),
        // "on_surface_variant" = strongest overlay — focused borders
        overlay2: get_or("on_surface_variant", &get("on_surface")?),
    })
}
