/// crawl-theme: Theme management domain.
///
/// Owns:
///   - Predefined palette loading from ~/.config/crawl/themes/*.toml
///   - matugen subprocess execution and output parsing
///   - Wallpaper watching via inotify (notify crate)
///   - Writing resolved palettes to GTK, Ghostty, shell env, and JSON
///   - Broadcasting ThemeEvents to the SSE stream

pub mod matugen;
pub mod palette;
pub mod themes;
pub mod writers;

pub use palette::{Palette, ThemeSource, ThemeState, Variant};

use crawl_ipc::events::{CrawlEvent, ThemeEvent as IpcThemeEvent};
use crawl_ipc::theme::{Palette as IpcPalette, ThemeSource as IpcThemeSource, ThemeState as IpcThemeState, Variant as IpcVariant};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{error, info, warn};

// ── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Active theme name (predefined) or "dynamic" for matugen mode
    pub active: String,
    /// Dark or light variant
    pub variant: Variant,
    /// Path to watch for wallpaper changes.
    /// crawl writes to this path when setting a wallpaper.
    /// Defaults to $XDG_CONFIG_HOME/crawl/current_wallpaper
    pub wallpaper_state_file: String,
    /// Wallpaper setter command. {path} is replaced with the wallpaper path.
    /// e.g. "swww img {path}" or "swaybg -i {path}"
    pub wallpaper_cmd: String,

    /// Directories to search for built-in theme templates (TOML files).
    /// Entries are checked in order. Common defaults:
    ///   /usr/share/crawl/themes
    ///   /usr/local/share/crawl/themes
    ///   assets/themes (for development)
    pub assets_dirs: Vec<String>,

    // Writer toggles — disable any you don't want
    pub write_gtk:     bool,
    pub write_ghostty: bool,
    pub write_shell:   bool,
    pub write_json:    bool,
}

impl Default for Config {
    fn default() -> Self {
        let config_home = dirs::config_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        Self {
            active:              "catppuccin-mocha".into(),
            variant:             Variant::Dark,
            wallpaper_state_file: format!("{config_home}/crawl/current_wallpaper"),
            wallpaper_cmd:       "swww img {path}".into(),
            assets_dirs: vec![
                "/usr/share/crawl/themes".into(),
                "/usr/local/share/crawl/themes".into(),
                "assets/themes".into(),
            ],
            write_gtk:           true,
            write_ghostty:       true,
            write_shell:         true,
            write_json:          true,
        }
    }
}

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ThemeError {
    #[error("theme not found: {0}")]
    NotFound(String),
    #[error("invalid palette: {0}")]
    InvalidPalette(String),
    #[error("matugen failed: {0}")]
    MatugenFailed(String),
    #[error("matugen not installed — install it from AUR: yay -S matugen")]
    MatugenMissing,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("writer error: {0}")]
    Writer(String),
}

// ── Events ────────────────────────────────────────────────────────────────────

/// Theme-domain events published to the CrawlEvent broadcast channel.
/// Quickshell listens for domain="theme" on the SSE stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ThemeEvent {
    /// Full palette resolved and applied — primary event Quickshell reacts to.
    PaletteChanged { state: ThemeState },
    /// Wallpaper path changed (before matugen runs).
    WallpaperChanged { path: String },
    /// matugen is currently running.
    Generating { wallpaper: String },
    /// matugen or a writer failed (non-fatal, previous theme still active).
    Error { reason: String },
    /// Variant flipped between dark/light.
    VariantChanged { variant: Variant },
}

// ── Domain runner ─────────────────────────────────────────────────────────────

pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    info!("crawl-theme starting (active={}, variant={})", cfg.active, cfg.variant);

    // Load initial theme
    let state = Arc::new(Mutex::new(
        load_initial(&cfg).await.unwrap_or_else(|e| {
            warn!("failed to load initial theme: {e} — falling back to catppuccin-mocha");
            fallback_state()
        }),
    ));

    // Apply initial theme to disk
    {
        let s = state.lock().await;
        apply_and_broadcast(&s, &cfg, &tx).await;
    }

    // Watch the wallpaper state file for changes
    if cfg.active == "dynamic" {
        let cfg2  = cfg.clone();
        let tx2   = tx.clone();
        let state2 = Arc::clone(&state);
        tokio::spawn(watch_wallpaper(cfg2, tx2, state2));
    }

    std::future::pending::<()>().await;
    Ok(())
}

pub async fn initial_state(cfg: &Config) -> ThemeState {
    load_initial(cfg).await.unwrap_or_else(|e| {
        warn!("failed to load initial theme: {e} — falling back to catppuccin-mocha");
        fallback_state()
    })
}

// ── Initial load ──────────────────────────────────────────────────────────────

async fn load_initial(cfg: &Config) -> Result<ThemeState, ThemeError> {
    if cfg.active == "dynamic" {
        // Load from the wallpaper state file if it exists
        let wallpaper_path = PathBuf::from(&cfg.wallpaper_state_file);
        if wallpaper_path.exists() {
            let path = tokio::fs::read_to_string(&wallpaper_path).await
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if !path.is_empty() {
                let palette = matugen::generate(&path, cfg.variant).await?;
                return Ok(ThemeState {
                    source:    ThemeSource::Dynamic { wallpaper: path.clone() },
                    variant:   cfg.variant,
                    palette,
                    wallpaper: Some(path),
                });
            }
        }
        // No wallpaper yet — fall back to default predefined
        warn!("dynamic theme selected but no wallpaper set yet — using catppuccin-mocha");
        themes::load("catppuccin-mocha", cfg.variant, Some(cfg))
    } else {
        themes::load(&cfg.active, cfg.variant, Some(cfg))
    }
}

// ── Wallpaper watcher ─────────────────────────────────────────────────────────

async fn watch_wallpaper(
    cfg: Config,
    tx: broadcast::Sender<CrawlEvent>,
    state: Arc<Mutex<ThemeState>>,
) {
    let watch_path = PathBuf::from(&cfg.wallpaper_state_file);
    let watch_dir  = watch_path.parent().unwrap_or(&watch_path).to_path_buf();

    let (fs_tx, mut fs_rx) = mpsc::channel::<notify::Result<Event>>(32);

    let mut watcher = match RecommendedWatcher::new(
        move |res| { let _ = fs_tx.blocking_send(res); },
        notify::Config::default(),
    ) {
        Ok(w)  => w,
        Err(e) => { error!("failed to create file watcher: {e}"); return; }
    };

    if let Err(e) = watcher.watch(&watch_dir, RecursiveMode::NonRecursive) {
        error!("failed to watch {watch_dir:?}: {e}");
        return;
    }

    info!("watching wallpaper state file: {:?}", watch_path);

    while let Some(event) = fs_rx.recv().await {
        match event {
            Ok(ev) if ev.paths.iter().any(|p| p == &watch_path) => {
                // Read new wallpaper path
                let Ok(content) = tokio::fs::read_to_string(&watch_path).await else { continue };
                let wallpaper = content.trim().to_string();
                if wallpaper.is_empty() { continue; }

                info!("wallpaper changed: {wallpaper}");

                // Emit WallpaperChanged immediately (Quickshell can update bg before palette)
                emit_theme(&tx, ThemeEvent::WallpaperChanged { path: wallpaper.clone() });

                // Emit Generating so Quickshell can show a spinner
                emit_theme(&tx, ThemeEvent::Generating { wallpaper: wallpaper.clone() });

                // Run matugen
                match matugen::generate(&wallpaper, cfg.variant).await {
                    Ok(palette) => {
                        let new_state = ThemeState {
                            source:    ThemeSource::Dynamic { wallpaper: wallpaper.clone() },
                            variant:   cfg.variant,
                            palette,
                            wallpaper: Some(wallpaper),
                        };
                        // Apply writers + broadcast
                        apply_and_broadcast(&new_state, &cfg, &tx).await;
                        // Update shared state
                        *state.lock().await = new_state;
                    }
                    Err(e) => {
                        error!("matugen failed: {e}");
                        emit_theme(&tx, ThemeEvent::Error { reason: e.to_string() });
                    }
                }
            }
            Ok(_)  => {}
            Err(e) => warn!("watcher error: {e}"),
        }
    }
}

// ── Apply helpers ─────────────────────────────────────────────────────────────

async fn apply_and_broadcast(
    state: &ThemeState,
    cfg: &Config,
    tx: &broadcast::Sender<CrawlEvent>,
) {
    // Run all writers concurrently
    let results = tokio::join!(
        run_if(cfg.write_gtk,     writers::gtk::write(&state.palette)),
        run_if(cfg.write_ghostty, writers::ghostty::write(&state.palette)),
        run_if(cfg.write_shell,   writers::shell::write(&state.palette)),
        run_if(cfg.write_json,    writers::json::write(state)),
    );

    for (name, result) in [
        ("gtk", results.0),
        ("ghostty", results.1),
        ("shell", results.2),
        ("json", results.3),
    ] {
        if let Err(e) = result {
            warn!("theme writer '{name}' failed: {e}");
        }
    }

    emit_theme(tx, ThemeEvent::PaletteChanged { state: state.clone() });
    info!(
        source = ?state.source,
        variant = %state.variant,
        "palette applied"
    );
}

async fn run_if(
    enabled: bool,
    fut: impl std::future::Future<Output = Result<(), ThemeError>>,
) -> Result<(), ThemeError> {
    if enabled { fut.await } else { Ok(()) }
}

fn emit_theme(tx: &broadcast::Sender<CrawlEvent>, evt: ThemeEvent) {
    let ipc_evt = match evt {
        ThemeEvent::PaletteChanged { state } => IpcThemeEvent::PaletteChanged { state: to_ipc_state(&state) },
        ThemeEvent::WallpaperChanged { path } => IpcThemeEvent::WallpaperChanged { path },
        ThemeEvent::Generating { wallpaper } => IpcThemeEvent::Generating { wallpaper },
        ThemeEvent::Error { reason } => IpcThemeEvent::Error { reason },
        ThemeEvent::VariantChanged { variant } => IpcThemeEvent::VariantChanged { variant: to_ipc_variant(variant) },
    };

    let _ = tx.send(CrawlEvent::Theme(ipc_evt));
}

fn to_ipc_variant(v: Variant) -> IpcVariant {
    match v { Variant::Dark => IpcVariant::Dark, Variant::Light => IpcVariant::Light }
}

fn to_ipc_source(s: &ThemeSource) -> IpcThemeSource {
    match s {
        ThemeSource::Predefined { name } => IpcThemeSource::Predefined { name: name.clone() },
        ThemeSource::Dynamic { wallpaper } => IpcThemeSource::Dynamic { wallpaper: wallpaper.clone() },
    }
}

fn to_ipc_palette(p: &Palette) -> IpcPalette {
    IpcPalette {
        base: p.base.clone(),
        mantle: p.mantle.clone(),
        crust: p.crust.clone(),
        surface0: p.surface0.clone(),
        surface1: p.surface1.clone(),
        surface2: p.surface2.clone(),
        text: p.text.clone(),
        subtext1: p.subtext1.clone(),
        subtext0: p.subtext0.clone(),
        primary: p.primary.clone(),
        secondary: p.secondary.clone(),
        tertiary: p.tertiary.clone(),
        error: p.error.clone(),
        warning: p.warning.clone(),
        info: p.info.clone(),
        overlay0: p.overlay0.clone(),
        overlay1: p.overlay1.clone(),
        overlay2: p.overlay2.clone(),
    }
}

fn to_ipc_state(state: &ThemeState) -> IpcThemeState {
    IpcThemeState {
        source: to_ipc_source(&state.source),
        variant: to_ipc_variant(state.variant),
        palette: to_ipc_palette(&state.palette),
        wallpaper: state.wallpaper.clone(),
    }
}

pub fn fallback_state() -> ThemeState {
    ThemeState {
        source:    ThemeSource::Predefined { name: "catppuccin-mocha".into() },
        variant:   Variant::Dark,
        palette:   themes::catppuccin_mocha(),
        wallpaper: None,
    }
}

// ── Public API (called by crawl-daemon router) ────────────────────────────────

/// Switch to a named predefined theme.
pub async fn set_theme(
    name: &str,
    cfg: &Config,
    tx: &broadcast::Sender<CrawlEvent>,
) -> Result<ThemeState, ThemeError> {
    let state = themes::load(name, cfg.variant, Some(cfg))?;
    apply_and_broadcast(&state, cfg, tx).await;
    Ok(state)
}

/// Set a wallpaper, run matugen, apply dynamic palette.
pub async fn set_wallpaper(
    path: &str,
    cfg: &Config,
    tx: &broadcast::Sender<CrawlEvent>,
) -> Result<ThemeState, ThemeError> {
    set_wallpaper_path(path, cfg).await?;

    emit_theme(tx, ThemeEvent::WallpaperChanged { path: path.to_string() });
    emit_theme(tx, ThemeEvent::Generating { wallpaper: path.to_string() });

    let palette = matugen::generate(path, cfg.variant).await?;
    let state = ThemeState {
        source:    ThemeSource::Dynamic { wallpaper: path.to_string() },
        variant:   cfg.variant,
        palette,
        wallpaper: Some(path.to_string()),
    };
    apply_and_broadcast(&state, cfg, tx).await;
    Ok(state)
}

pub async fn set_wallpaper_path(path: &str, cfg: &Config) -> Result<(), ThemeError> {
    let state_file = PathBuf::from(&cfg.wallpaper_state_file);
    if let Some(parent) = state_file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&state_file, path).await?;

    if !cfg.wallpaper_cmd.is_empty() {
        let cmd_str = cfg.wallpaper_cmd.replace("{path}", path);
        let parts: Vec<&str> = cmd_str.split_whitespace().collect();
        if let Some((bin, args)) = parts.split_first() {
            tokio::process::Command::new(bin).args(args).spawn().ok();
        }
    }

    Ok(())
}

/// Toggle dark/light variant, re-resolve current theme source.
pub async fn set_variant(
    variant: Variant,
    current: &ThemeState,
    cfg: &Config,
    tx: &broadcast::Sender<CrawlEvent>,
) -> Result<ThemeState, ThemeError> {
    let state = match &current.source {
        ThemeSource::Predefined { name } => themes::load(name, variant, Some(cfg))?,
        ThemeSource::Dynamic { wallpaper } => {
            let palette = matugen::generate(wallpaper, variant).await?;
            ThemeState {
                source:    current.source.clone(),
                variant,
                palette,
                wallpaper: current.wallpaper.clone(),
            }
        }
    };
    emit_theme(tx, ThemeEvent::VariantChanged { variant });
    apply_and_broadcast(&state, cfg, tx).await;
    Ok(state)
}
