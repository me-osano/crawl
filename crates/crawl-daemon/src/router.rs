use axum::{
    extract::{Query, State},
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
    types::Notification,
};
use crate::{sse, state::AppState};
use crawl_ipc::events::ThemeEvent;
use crawl_theme::{ThemeState, Variant, ThemeSource};
use std::time::{SystemTime, UNIX_EPOCH};

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
        .route("/bluetooth/pair",       post(bt_pair))
        .route("/bluetooth/trust",      post(bt_trust))
        .route("/bluetooth/remove",     post(bt_remove))
        .route("/bluetooth/alias",      post(bt_alias))
        .route("/bluetooth/discoverable", post(bt_discoverable))
        .route("/bluetooth/pairable",   post(bt_pairable))

        // ── Network ──────────────────────────────────────────────────────────
        .route("/network/status",    get(net_status))
        .route("/network/wifi",      get(net_wifi_list))
        .route("/network/connect",   post(net_connect))
        .route("/network/power",     post(net_power))
        .route("/network/eth/connect",   post(net_eth_connect))
        .route("/network/eth/disconnect",post(net_eth_disconnect))

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

        // ── Theme ────────────────────────────────────────────────────────────
        .route("/theme/status",     get(theme_status))
        .route("/theme/set",        post(theme_set))
        .route("/theme/wallpaper",  post(theme_wallpaper))
        .route("/theme/variant",    post(theme_variant))
        .route("/theme/regenerate", post(theme_regenerate))
        .route("/theme/list",       get(theme_list))

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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
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

fn bluetooth_error(err: crawl_bluetooth::BtError) -> ApiError {
    match err {
        crawl_bluetooth::BtError::NoAdapter => ApiError(
            StatusCode::NOT_FOUND,
            ErrorEnvelope::new("bluetooth", "no_adapter", "no bluetooth adapter found"),
        ),
        crawl_bluetooth::BtError::DeviceNotFound(addr) => ApiError(
            StatusCode::NOT_FOUND,
            ErrorEnvelope::new("bluetooth", "device_not_found", format!("device not found: {addr}")),
        ),
        crawl_bluetooth::BtError::Session(err) => ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorEnvelope::new("bluetooth", "session_error", err.to_string()),
        ),
    }
}

fn proc_error(err: crawl_proc::ProcError, pid: u32) -> ApiError {
    match err {
        crawl_proc::ProcError::NotFound(_) => ApiError(
            StatusCode::NOT_FOUND,
            ErrorEnvelope::new("proc", "not_found", format!("process not found: PID {pid}")),
        ),
        crawl_proc::ProcError::PermissionDenied(_) => ApiError(
            StatusCode::FORBIDDEN,
            ErrorEnvelope::new("proc", "permission_denied", format!("permission denied killing PID {pid}")),
        ),
        crawl_proc::ProcError::SignalFailed(msg) => ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorEnvelope::new("proc", "signal_failed", msg),
        ),
    }
}

// ── Health ───────────────────────────────────────────────────────────────────

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

// ── Bluetooth handlers (stubs) ────────────────────────────────────────────────

async fn bt_status(State(_s): State<AppState>) -> impl IntoResponse {
    match crawl_bluetooth::get_status().await {
        Ok(status) => Json(status).into_response(),
        Err(err) => bluetooth_error(err).into_response(),
    }
}
async fn bt_devices(State(_s): State<AppState>) -> impl IntoResponse {
    match crawl_bluetooth::get_devices().await {
        Ok(devices) => Json(devices).into_response(),
        Err(err) => bluetooth_error(err).into_response(),
    }
}
async fn bt_scan(State(_s): State<AppState>) -> impl IntoResponse {
    match crawl_bluetooth::scan().await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => bluetooth_error(err).into_response(),
    }
}
async fn bt_connect(
    State(_s): State<AppState>,
    Json(body): Json<BtAddressBody>,
) -> impl IntoResponse {
    match crawl_bluetooth::connect(&body.address).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => bluetooth_error(err).into_response(),
    }
}
async fn bt_disconnect(
    State(_s): State<AppState>,
    Json(body): Json<BtAddressBody>,
) -> impl IntoResponse {
    match crawl_bluetooth::disconnect(&body.address).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => bluetooth_error(err).into_response(),
    }
}
async fn bt_power(
    State(_s): State<AppState>,
    Json(body): Json<BtPowerBody>,
) -> impl IntoResponse {
    match crawl_bluetooth::set_powered(body.on).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => bluetooth_error(err).into_response(),
    }
}

#[derive(Deserialize)]
struct BtTrustBody {
    address: String,
    trusted: bool,
}

#[derive(Deserialize)]
struct BtAliasBody {
    address: String,
    alias: String,
}

#[derive(Deserialize)]
struct BtToggleBody {
    on: bool,
}

async fn bt_pair(
    State(_s): State<AppState>,
    Json(body): Json<BtAddressBody>,
) -> impl IntoResponse {
    match crawl_bluetooth::pair(&body.address).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => bluetooth_error(err).into_response(),
    }
}

async fn bt_trust(
    State(_s): State<AppState>,
    Json(body): Json<BtTrustBody>,
) -> impl IntoResponse {
    match crawl_bluetooth::set_trusted(&body.address, body.trusted).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => bluetooth_error(err).into_response(),
    }
}

async fn bt_remove(
    State(_s): State<AppState>,
    Json(body): Json<BtAddressBody>,
) -> impl IntoResponse {
    match crawl_bluetooth::remove_device(&body.address).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => bluetooth_error(err).into_response(),
    }
}

async fn bt_alias(
    State(_s): State<AppState>,
    Json(body): Json<BtAliasBody>,
) -> impl IntoResponse {
    match crawl_bluetooth::set_alias(&body.address, &body.alias).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => bluetooth_error(err).into_response(),
    }
}

async fn bt_discoverable(
    State(_s): State<AppState>,
    Json(body): Json<BtToggleBody>,
) -> impl IntoResponse {
    match crawl_bluetooth::set_discoverable(body.on).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => bluetooth_error(err).into_response(),
    }
}

async fn bt_pairable(
    State(_s): State<AppState>,
    Json(body): Json<BtToggleBody>,
) -> impl IntoResponse {
    match crawl_bluetooth::set_pairable(body.on).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => bluetooth_error(err).into_response(),
    }
}

// ── Network handlers (stubs) ──────────────────────────────────────────────────

async fn net_status(State(_s): State<AppState>) -> impl IntoResponse {
    match crawl_network::get_status().await {
        Ok(status) => Json(status).into_response(),
        Err(err) => ApiError(
            StatusCode::BAD_REQUEST,
            ErrorEnvelope::new("network", "network_error", err.to_string()),
        )
        .into_response(),
    }
    // TODO(crawl-network): Map NetError variants to precise HTTP status codes.
}
async fn net_wifi_list(State(_s): State<AppState>) -> impl IntoResponse {
    match crawl_network::list_wifi().await {
        Ok(list) => Json(list).into_response(),
        Err(err) => ApiError(
            StatusCode::BAD_REQUEST,
            ErrorEnvelope::new("network", "network_error", err.to_string()),
        )
        .into_response(),
    }
    // TODO(crawl-network): Consider returning 503 when NetworkManager is unavailable.
}
async fn net_connect(
    State(_s): State<AppState>,
    Json(payload): Json<NetConnectBody>,
) -> impl IntoResponse {
    match crawl_network::connect_wifi(&payload.ssid, payload.password.as_deref()).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => ApiError(
            StatusCode::BAD_REQUEST,
            ErrorEnvelope::new("network", "network_error", err.to_string()),
        )
        .into_response(),
    }
}

#[derive(Deserialize)]
struct NetPowerBody {
    on: bool,
}

async fn net_power(
    State(_s): State<AppState>,
    Json(payload): Json<NetPowerBody>,
) -> impl IntoResponse {
    match crawl_network::set_network_enabled(payload.on).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => ApiError(
            StatusCode::BAD_REQUEST,
            ErrorEnvelope::new("network", "network_error", err.to_string()),
        )
        .into_response(),
    }
}

async fn net_eth_connect(
    State(_s): State<AppState>,
    Json(payload): Json<NetEthBody>,
) -> impl IntoResponse {
    match crawl_network::connect_ethernet(payload.interface.as_deref()).await {
        Ok(iface) => Json(json!({ "ok": true, "interface": iface })).into_response(),
        Err(err) => ApiError(
            StatusCode::BAD_REQUEST,
            ErrorEnvelope::new("network", "network_error", err.to_string()),
        )
        .into_response(),
    }
}

async fn net_eth_disconnect(
    State(_s): State<AppState>,
    Json(payload): Json<NetEthBody>,
) -> impl IntoResponse {
    match crawl_network::disconnect_ethernet(payload.interface.as_deref()).await {
        Ok(iface) => Json(json!({ "ok": true, "interface": iface })).into_response(),
        Err(err) => ApiError(
            StatusCode::BAD_REQUEST,
            ErrorEnvelope::new("network", "network_error", err.to_string()),
        )
        .into_response(),
    }
}

// ── Notification handlers (stubs) ────────────────────────────────────────────

async fn notify_list(State(state): State<AppState>) -> impl IntoResponse {
    let list: Vec<Notification> = state.notify_store.list();
    Json(list).into_response()
}
async fn notify_send(State(state): State<AppState>, Json(body): Json<NotifySendBody>) -> impl IntoResponse {
    let notif = Notification {
        id: 0,
        app_name: "crawl".into(),
        summary: body.title,
        body: body.body,
        icon: String::new(),
        urgency: body.urgency.unwrap_or(crawl_ipc::types::Urgency::Normal),
        actions: vec![],
        expire_timeout_ms: body.timeout_ms.unwrap_or(state.config.notifications.default_timeout_ms),
        timestamp_ms: now_ms(),
    };
    let id = state.notify_store.insert(notif.clone());
    let _ = state.event_tx.send(CrawlEvent::Notify(crawl_ipc::events::NotifyEvent::New {
        notification: Notification { id, ..notif },
    }));
    Json(json!({ "id": id })).into_response()
}
async fn notify_dismiss(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<u32>,
) -> impl IntoResponse {
    if state.notify_store.remove(id).is_some() {
        let _ = state.event_tx.send(CrawlEvent::Notify(crawl_ipc::events::NotifyEvent::Closed {
            id,
            reason: 3,
        }));
    }
    Json(json!({ "ok": true })).into_response()
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

#[derive(Deserialize)]
struct BtAddressBody {
    address: String,
}

#[derive(Deserialize)]
struct BtPowerBody {
    on: bool,
}

#[derive(Deserialize)]
struct NotifySendBody {
    title: String,
    body: String,
    urgency: Option<crawl_ipc::types::Urgency>,
    timeout_ms: Option<i32>,
}

// ── Process handlers ──────────────────────────────────────────────────────────
async fn proc_list(
    State(state): State<AppState>,
    Query(params): Query<ProcListParams>,
) -> impl IntoResponse {
    let sort = params
        .sort
        .as_deref()
        .unwrap_or(&state.config.processes.default_sort);
    let top = params.top.unwrap_or(state.config.processes.default_top);
    Json(crawl_proc::list_processes(sort, top))
}

async fn proc_find(Query(params): Query<ProcFindParams>) -> impl IntoResponse {
    let name = params.name.unwrap_or_default();
    if name.is_empty() {
        return ApiError(
            StatusCode::BAD_REQUEST,
            ErrorEnvelope::new("proc", "missing_name", "query param 'name' is required"),
        )
        .into_response();
    }
    Json(crawl_proc::find_processes(&name)).into_response()
}
async fn proc_kill(
    State(_s): State<AppState>,
    axum::extract::Path(pid): axum::extract::Path<u32>,
    Json(payload): Json<ProcKillBody>,
) -> impl IntoResponse {
    match crawl_proc::kill_process(pid, payload.force) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(err) => proc_error(err, pid).into_response(),
    }
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
    match crawl_power::get_battery().await {
        Ok(status) => Json(status).into_response(),
        Err(err) => ApiError(
            StatusCode::BAD_REQUEST,
            ErrorEnvelope::new("power", "power_error", err.to_string()),
        )
        .into_response(),
    }
}

#[derive(Deserialize)]
struct ProcListParams {
    sort: Option<String>,
    top: Option<usize>,
}

#[derive(Deserialize)]
struct ProcFindParams {
    name: Option<String>,
}

#[derive(Deserialize)]
struct ProcKillBody {
    force: bool,
}

#[derive(Deserialize)]
struct NetConnectBody {
    ssid: String,
    password: Option<String>,
}

#[derive(Deserialize)]
struct NetEthBody {
    interface: Option<String>,
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

// ── Theme handlers ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SetThemeBody {
    name: String,
}

#[derive(Deserialize)]
struct SetWallpaperBody {
    path: String,
    no_generate: Option<bool>,
}

#[derive(Deserialize)]
struct SetVariantBody {
    variant: String,
}

async fn theme_status(State(state): State<AppState>) -> Json<crawl_ipc::theme::ThemeState> {
    let current = state.theme_state.lock().await.clone();
    Json(to_ipc_theme_state(&current))
}

async fn theme_set(
    State(state): State<AppState>,
    Json(body): Json<SetThemeBody>,
) -> Result<Json<crawl_ipc::theme::ThemeState>, ApiError> {
    let new_state = crawl_theme::set_theme(&body.name, &state.config.theme, &state.event_tx)
        .await
        .map_err(theme_error)?;
    *state.theme_state.lock().await = new_state.clone();
    Ok(Json(to_ipc_theme_state(&new_state)))
}

async fn theme_wallpaper(
    State(state): State<AppState>,
    Json(body): Json<SetWallpaperBody>,
) -> Result<Json<crawl_ipc::theme::ThemeState>, ApiError> {
    if body.no_generate.unwrap_or(false) {
        let path = body.path.clone();
        crawl_theme::set_wallpaper_path(&path, &state.config.theme)
            .await
            .map_err(theme_error)?;
        let _ = state.event_tx.send(CrawlEvent::Theme(ThemeEvent::WallpaperChanged { path }));
        let current = state.theme_state.lock().await.clone();
        return Ok(Json(to_ipc_theme_state(&current)));
    }

    let new_state = crawl_theme::set_wallpaper(&body.path, &state.config.theme, &state.event_tx)
        .await
        .map_err(theme_error)?;
    *state.theme_state.lock().await = new_state.clone();
    Ok(Json(to_ipc_theme_state(&new_state)))
}

async fn theme_variant(
    State(state): State<AppState>,
    Json(body): Json<SetVariantBody>,
) -> Result<Json<crawl_ipc::theme::ThemeState>, ApiError> {
    let variant = match body.variant.as_str() {
        "light" => Variant::Light,
        _ => Variant::Dark,
    };
    let current = state.theme_state.lock().await.clone();
    let new_state = crawl_theme::set_variant(variant, &current, &state.config.theme, &state.event_tx)
        .await
        .map_err(theme_error)?;
    *state.theme_state.lock().await = new_state.clone();
    Ok(Json(to_ipc_theme_state(&new_state)))
}

async fn theme_regenerate(State(state): State<AppState>) -> Result<Json<crawl_ipc::theme::ThemeState>, ApiError> {
    let current = state.theme_state.lock().await.clone();
    if let ThemeSource::Dynamic { wallpaper } = &current.source {
        let new_state = crawl_theme::set_wallpaper(wallpaper, &state.config.theme, &state.event_tx)
            .await
            .map_err(theme_error)?;
        *state.theme_state.lock().await = new_state.clone();
        return Ok(Json(to_ipc_theme_state(&new_state)));
    }

    Ok(Json(to_ipc_theme_state(&current)))
}

async fn theme_list(State(state): State<AppState>) -> Json<Value> {
    let all = crawl_theme::themes::list_all_with_config(&state.config.theme);
    Json(json!({ "themes": all }))
}

fn to_ipc_theme_state(state: &ThemeState) -> crawl_ipc::theme::ThemeState {
    crawl_ipc::theme::ThemeState {
        source: match &state.source {
            ThemeSource::Predefined { name } => crawl_ipc::theme::ThemeSource::Predefined { name: name.clone() },
            ThemeSource::Dynamic { wallpaper } => crawl_ipc::theme::ThemeSource::Dynamic { wallpaper: wallpaper.clone() },
        },
        variant: match state.variant {
            Variant::Dark => crawl_ipc::theme::Variant::Dark,
            Variant::Light => crawl_ipc::theme::Variant::Light,
        },
        palette: crawl_ipc::theme::Palette {
            base: state.palette.base.clone(),
            mantle: state.palette.mantle.clone(),
            crust: state.palette.crust.clone(),
            surface0: state.palette.surface0.clone(),
            surface1: state.palette.surface1.clone(),
            surface2: state.palette.surface2.clone(),
            text: state.palette.text.clone(),
            subtext1: state.palette.subtext1.clone(),
            subtext0: state.palette.subtext0.clone(),
            primary: state.palette.primary.clone(),
            secondary: state.palette.secondary.clone(),
            tertiary: state.palette.tertiary.clone(),
            error: state.palette.error.clone(),
            warning: state.palette.warning.clone(),
            info: state.palette.info.clone(),
            overlay0: state.palette.overlay0.clone(),
            overlay1: state.palette.overlay1.clone(),
            overlay2: state.palette.overlay2.clone(),
        },
        wallpaper: state.wallpaper.clone(),
    }
}

fn theme_error(err: crawl_theme::ThemeError) -> ApiError {
    ApiError(
        StatusCode::BAD_REQUEST,
        ErrorEnvelope::new("theme", "theme_error", err.to_string()),
    )
}
