use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use crawl_ipc::{
    ErrorEnvelope,
    CrawlEvent,
    events::BrightnessEvent,
};
use crate::{sse, state::AppState};

pub fn build(state: AppState) -> Router {
    Router::new()
        // ── Health ───────────────────────────────────────────────────────────
        .route("/health", get(health))

        // ── SSE event stream ─────────────────────────────────────────────────
        .route("/events", get(sse::handler))

        // ── Bluetooth ────────────────────────────────────────────────────────
        .route("/bluetooth/status",     get(bt_status))
        .route("/bluetooth/devices",    get(bt_devices))
        .route("/bluetooth/scan",       post(bt_scan))
        .route("/bluetooth/connect",    post(bt_connect))
        .route("/bluetooth/disconnect", post(bt_disconnect))
        .route("/bluetooth/power",      post(bt_power))

        // ── Network ──────────────────────────────────────────────────────────
        .route("/network/status",    get(net_status))
        .route("/network/wifi",      get(net_wifi_list))
        .route("/network/connect",   post(net_connect))

        // ── Notifications ────────────────────────────────────────────────────
        .route("/notify/list",   get(notify_list))
        .route("/notify/send",   post(notify_send))
        .route("/notify/:id",    delete(notify_dismiss))

        // ── Clipboard ────────────────────────────────────────────────────────
        .route("/clipboard",     get(clip_get))
        .route("/clipboard",     post(clip_set))
        .route("/clipboard/history", get(clip_history))

        // ── Sysmon ───────────────────────────────────────────────────────────
        .route("/sysmon/cpu",    get(sysmon_cpu))
        .route("/sysmon/mem",    get(sysmon_mem))
        .route("/sysmon/disk",   get(sysmon_disk))

        // ── Brightness ───────────────────────────────────────────────────────
        .route("/brightness",    get(brightness_get))
        .route("/brightness/set",post(brightness_set))
        .route("/brightness/inc",post(brightness_inc))
        .route("/brightness/dec",post(brightness_dec))

        // ── Processes ────────────────────────────────────────────────────────
        .route("/proc/list",     get(proc_list))
        .route("/proc/find",     get(proc_find))
        .route("/proc/:pid/kill",post(proc_kill))

        // ── Media (MPRIS) ────────────────────────────────────────────────────
        .route("/media/players", get(media_players))
        .route("/media/active",  get(media_active))
        .route("/media/play",    post(media_play))
        .route("/media/pause",   post(media_pause))
        .route("/media/next",    post(media_next))
        .route("/media/prev",    post(media_prev))
        .route("/media/volume",  post(media_volume))

        // ── Power (UPower) ───────────────────────────────────────────────────
        .route("/power/battery", get(power_battery))

        // ── Disk (UDisks2) ───────────────────────────────────────────────────
        .route("/disk/list",     get(disk_list))
        .route("/disk/mount",    post(disk_mount))
        .route("/disk/unmount",  post(disk_unmount))
        .route("/disk/eject",    post(disk_eject))

        // ── Audio (PipeWire) ─────────────────────────────────────────────────
        .route("/audio/sinks",   get(audio_sinks))
        .route("/audio/sources", get(audio_sources))
        .route("/audio/volume",  post(audio_volume))
        .route("/audio/mute",    post(audio_mute))

        .with_state(state)
}

// ── Response helpers ─────────────────────────────────────────────────────────

pub struct ApiError(StatusCode, ErrorEnvelope);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(self.1)).into_response()
    }
}

fn not_implemented(domain: &str) -> ApiError {
    ApiError(
        StatusCode::NOT_IMPLEMENTED,
        ErrorEnvelope::new(domain, "not_implemented", "This endpoint is not yet implemented"),
    )
}

fn brightness_error(err: crawl_brightness::BrightnessError) -> ApiError {
    match err {
        crawl_brightness::BrightnessError::NoDevice => ApiError(
            StatusCode::NOT_FOUND,
            ErrorEnvelope::new("brightness", "no_device", "No backlight device found"),
        ),
        crawl_brightness::BrightnessError::OutOfRange(_) => ApiError(
            StatusCode::BAD_REQUEST,
            ErrorEnvelope::new("brightness", "out_of_range", err.to_string()),
        ),
        crawl_brightness::BrightnessError::ReadError { .. } => ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorEnvelope::new("brightness", "read_error", err.to_string()),
        ),
        crawl_brightness::BrightnessError::WriteError(_) => ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorEnvelope::new("brightness", "write_error", err.to_string()),
        ),
    }
}

// ── Health ───────────────────────────────────────────────────────────────────

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

// ── Bluetooth handlers (stubs) ────────────────────────────────────────────────

async fn bt_status(State(_s): State<AppState>) -> impl IntoResponse {
    // TODO: return crawl_bluetooth::get_status().await
    not_implemented("bluetooth")
}
async fn bt_devices(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("bluetooth")
}
async fn bt_scan(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("bluetooth")
}
async fn bt_connect(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("bluetooth")
}
async fn bt_disconnect(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("bluetooth")
}
async fn bt_power(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("bluetooth")
}

// ── Network handlers (stubs) ──────────────────────────────────────────────────

async fn net_status(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("network")
}
async fn net_wifi_list(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("network")
}
async fn net_connect(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("network")
}

// ── Notification handlers (stubs) ────────────────────────────────────────────

async fn notify_list(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("notify")
}
async fn notify_send(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("notify")
}
async fn notify_dismiss(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("notify")
}

// ── Clipboard handlers (stubs) ───────────────────────────────────────────────

async fn clip_get(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("clipboard")
}
async fn clip_set(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("clipboard")
}
async fn clip_history(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("clipboard")
}

// ── Sysmon handlers (stubs) ───────────────────────────────────────────────────

async fn sysmon_cpu(State(_s): State<AppState>) -> impl IntoResponse {
    let cpu = crawl_sysmon::get_cpu();
    Json(cpu)
}
async fn sysmon_mem(State(_s): State<AppState>) -> impl IntoResponse {
    let mem = crawl_sysmon::get_mem();
    Json(mem)
}
async fn sysmon_disk(State(_s): State<AppState>) -> impl IntoResponse {
    let disks = crawl_sysmon::get_disks();
    Json(disks)
}

// ── Brightness handlers (stubs) ───────────────────────────────────────────────

async fn brightness_get(State(state): State<AppState>) -> Result<Json<crawl_ipc::types::BrightnessStatus>, ApiError> {
    let backlight = crawl_brightness::Backlight::open(&state.config.brightness)
        .map_err(brightness_error)?;
    let status = backlight.status().map_err(brightness_error)?;
    Ok(Json(status))
}
async fn brightness_set(
    State(state): State<AppState>,
    Json(payload): Json<BrightnessValue>,
) -> Result<Json<crawl_ipc::types::BrightnessStatus>, ApiError> {
    let backlight = crawl_brightness::Backlight::open(&state.config.brightness)
        .map_err(brightness_error)?;

    let status = backlight
        .set_percent(payload.value, &state.config.brightness)
        .map_err(brightness_error)?;
    let _ = state
        .event_tx
        .send(CrawlEvent::Brightness(BrightnessEvent::Changed { status: status.clone() }));
    Ok(Json(status))
}
async fn brightness_inc(
    State(state): State<AppState>,
    Json(payload): Json<BrightnessValue>,
) -> Result<Json<crawl_ipc::types::BrightnessStatus>, ApiError> {
    let backlight = crawl_brightness::Backlight::open(&state.config.brightness)
        .map_err(brightness_error)?;

    let status = backlight
        .adjust_percent(payload.value, &state.config.brightness)
        .map_err(brightness_error)?;
    let _ = state
        .event_tx
        .send(CrawlEvent::Brightness(BrightnessEvent::Changed { status: status.clone() }));
    Ok(Json(status))
}
async fn brightness_dec(
    State(state): State<AppState>,
    Json(payload): Json<BrightnessValue>,
) -> Result<Json<crawl_ipc::types::BrightnessStatus>, ApiError> {
    let backlight = crawl_brightness::Backlight::open(&state.config.brightness)
        .map_err(brightness_error)?;

    let status = backlight
        .adjust_percent(-payload.value, &state.config.brightness)
        .map_err(brightness_error)?;
    let _ = state
        .event_tx
        .send(CrawlEvent::Brightness(BrightnessEvent::Changed { status: status.clone() }));
    Ok(Json(status))
}

#[derive(Deserialize)]
struct BrightnessValue {
    value: f32,
}

// ── Process handlers (stubs) ──────────────────────────────────────────────────

async fn proc_list(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("proc")
}
async fn proc_find(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("proc")
}
async fn proc_kill(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("proc")
}

// ── Media handlers (stubs) ────────────────────────────────────────────────────

async fn media_players(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("media")
}
async fn media_active(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("media")
}
async fn media_play(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("media")
}
async fn media_pause(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("media")
}
async fn media_next(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("media")
}
async fn media_prev(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("media")
}
async fn media_volume(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("media")
}

// ── Power handlers (stubs) ────────────────────────────────────────────────────

async fn power_battery(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("power")
}

// ── Disk handlers (stubs) ─────────────────────────────────────────────────────

async fn disk_list(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("disk")
}
async fn disk_mount(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("disk")
}
async fn disk_unmount(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("disk")
}
async fn disk_eject(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("disk")
}

// ── Audio handlers (stubs) ────────────────────────────────────────────────────

async fn audio_sinks(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("audio")
}
async fn audio_sources(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("audio")
}
async fn audio_volume(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("audio")
}
async fn audio_mute(State(_s): State<AppState>) -> impl IntoResponse {
    not_implemented("audio")
}
