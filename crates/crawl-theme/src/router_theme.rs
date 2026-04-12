/// crawl-daemon/src/router_theme.rs
///
/// Theme route handlers to add to router.rs.
///
/// Wire into router.rs:
///
///   // In build() add:
///   .route("/theme/status",     get(theme_status))
///   .route("/theme/set",        post(theme_set))
///   .route("/theme/wallpaper",  post(theme_wallpaper))
///   .route("/theme/variant",    post(theme_variant))
///   .route("/theme/regenerate", post(theme_regenerate))
///   .route("/theme/list",       get(theme_list))
///
/// And add `theme_state: Arc<Mutex<ThemeState>>` to AppState.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;
use serde_json::{json, Value};

// ── Request bodies ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SetThemeBody {
    pub name: String,
}

#[derive(Deserialize)]
pub struct SetWallpaperBody {
    pub path:        String,
    pub no_generate: Option<bool>,
}

#[derive(Deserialize)]
pub struct SetVariantBody {
    pub variant: String,  // "dark" | "light"
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// GET /theme/status — current theme state + full palette
pub async fn theme_status(/* State(s): State<AppState> */) -> impl IntoResponse {
    // TODO: return s.theme_state.lock().await serialized as JSON
    // Shape:
    // {
    //   "source":   { "predefined": { "name": "catppuccin-mocha" } },
    //   "variant":  "dark",
    //   "wallpaper": null,
    //   "palette":  { "base": "#1e1e2e", ... }
    // }
    Json(json!({ "error": { "code": "not_implemented" } }))
}

/// POST /theme/set { "name": "rose-pine" }
pub async fn theme_set(/* State(s): State<AppState>, */ Json(body): Json<SetThemeBody>) -> impl IntoResponse {
    // TODO:
    // let new_state = crawl_theme::set_theme(&body.name, &s.config.theme, &s.event_tx).await?;
    // *s.theme_state.lock().await = new_state.clone();
    // Json(new_state)
    Json(json!({ "error": { "code": "not_implemented" } }))
}

/// POST /theme/wallpaper { "path": "/home/enosh/walls/forest.jpg", "no_generate": false }
pub async fn theme_wallpaper(/* State(s): State<AppState>, */ Json(body): Json<SetWallpaperBody>) -> impl IntoResponse {
    // TODO:
    // if body.no_generate.unwrap_or(false) {
    //     // Just set wallpaper, emit WallpaperChanged, no matugen
    // } else {
    //     let new_state = crawl_theme::set_wallpaper(&body.path, &s.config.theme, &s.event_tx).await?;
    //     *s.theme_state.lock().await = new_state;
    // }
    Json(json!({ "error": { "code": "not_implemented" } }))
}

/// POST /theme/variant { "variant": "light" }
pub async fn theme_variant(/* State(s): State<AppState>, */ Json(body): Json<SetVariantBody>) -> impl IntoResponse {
    // TODO:
    // let v = if body.variant == "light" { Variant::Light } else { Variant::Dark };
    // let current = s.theme_state.lock().await.clone();
    // let new_state = crawl_theme::set_variant(v, &current, &s.config.theme, &s.event_tx).await?;
    // *s.theme_state.lock().await = new_state;
    Json(json!({ "error": { "code": "not_implemented" } }))
}

/// POST /theme/regenerate — re-run matugen on current wallpaper
pub async fn theme_regenerate(/* State(s): State<AppState> */) -> impl IntoResponse {
    // TODO:
    // let current = s.theme_state.lock().await.clone();
    // if let ThemeSource::Dynamic { wallpaper } = &current.source {
    //     let new_state = crawl_theme::set_wallpaper(&wallpaper.clone(), &s.config.theme, &s.event_tx).await?;
    //     *s.theme_state.lock().await = new_state;
    // }
    Json(json!({ "error": { "code": "not_implemented" } }))
}

/// GET /theme/list — all available theme names
pub async fn theme_list(/* State(s): State<AppState> */) -> impl IntoResponse {
    // TODO:
    // let all = crawl_theme::themes::list_all();
    // let builtin_set: HashSet<_> = crawl_theme::themes::builtin_names().iter().collect();
    // let themes: Vec<Value> = all.iter().map(|n| json!({
    //     "name":    n,
    //     "builtin": builtin_set.contains(n.as_str()),
    // })).collect();
    // Json(json!({ "themes": themes }))
    Json(json!({ "themes": [] }))
}
