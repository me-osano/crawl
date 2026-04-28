//! JSON-RPC 2.0 Server for Crawl
//! Protocol: {"jsonrpc": "2.0", "method": "CmdName", "params": {...}, "id": 1}
//!          -> {"jsonrpc": "2.0", "result": {...}, "id": 1}
//! Events (NDJSON): {"jsonrpc": "2.0", "method": "event", "params": {...}}


use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::RwLock as TokioRwLock;
use tracing::{debug, error, info};

use crate::state::AppState;
use crawl_ipc::{CrawlEvent, protocol::{self, Request, Response, Error, error_code, now_ms, EventMessage}};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum Command {
    Hello { client: Option<String>, version: Option<String> },
    Ping,
    Get { key: Option<String> },
    Set { key: String, value: serde_json::Value },
    Audio { action: Option<String>, device: Option<String>, percent: Option<u32> },
    BrightnessGet,
    BrightnessSet { value: i32 },
    BrightnessInc { value: i32 },
    BrightnessDec { value: i32 },
    NetStatus,
    NetWifiList,
    NetWifiDetails,
    NetWifiConnect { ssid: String, password: Option<String> },
    NetWifiDisconnect,
    NetWifiScan,
    NetWifiForget { ssid: String },
    NetHotspotStart,
    NetHotspotStop,
    NetHotspotStatus,
    NetPower { enabled: bool },
    NetEthList,
    NetEthConnect { interface: String },
    NetEthDetails { interface: String },
    NetEthDisconnect,
    BtStatus,
    BtDevices,
    BtScan,
    BtConnect { address: String },
    BtDisconnect { address: String },
    BtPower { enabled: bool },
    BtPair { address: String },
    BtRemove { address: String },
    BtDiscoverable { enabled: bool },
    BtTrust { address: String, trusted: bool },
    BtAlias { address: String, alias: String },
    BtPairable { enabled: bool },
    WallpaperStatus,
    WallpaperSet { path: String, monitor: Option<String>, mode: Option<String>, transition: Option<String> },
    WallpaperGet { monitor: Option<String> },
    Sysinfo,
}

pub struct JsonServer {
    config_path: PathBuf,
    state: Arc<TokioRwLock<Option<Arc<AppState>>>>,
    event_rx: Arc<TokioRwLock<Option<tokio::sync::broadcast::Receiver<CrawlEvent>>>>,
}

impl JsonServer {
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            state: Arc::new(TokioRwLock::new(None)),
            event_rx: Arc::new(TokioRwLock::new(None)),
        }
    }

    pub async fn set_state(&self, state: Arc<AppState>, event_rx: tokio::sync::broadcast::Receiver<CrawlEvent>) {
        let mut lock = self.state.write().await;
        *lock = Some(state);
        let mut rx_lock = self.event_rx.write().await;
        *rx_lock = Some(event_rx);
    }

    #[allow(dead_code)]
    async fn get_state(&self) -> Option<Arc<AppState>> {
        self.state.read().await.clone()
    }

    pub async fn run(&self, socket_path: PathBuf) -> anyhow::Result<()> {
        if socket_path.exists() {
            std::fs::remove_file(&socket_path)?;
        }
        let listener = UnixListener::bind(&socket_path)?;
        info!("JSON server listening on {:?}", socket_path);
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let server = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, &server).await {
                            error!("connection error: {}", e);
                        }
                    });
                }
                Err(e) => error!("accept error: {}", e),
            }
        }
    }
}

impl Clone for JsonServer {
    fn clone(&self) -> Self {
        Self {
            config_path: self.config_path.clone(),
            state: self.state.clone(),
            event_rx: self.event_rx.clone(),
        }
    }
}

async fn handle_connection(mut stream: tokio::net::UnixStream, server: &JsonServer) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut subscribed = false;
    let mut event_rx: Option<tokio::sync::broadcast::Receiver<CrawlEvent>> = {
        let rx_lock = server.event_rx.read().await;
        rx_lock.as_ref().map(|rx| rx.resubscribe())
    };
    loop {
        tokio::select! {
            result = reader.read_line(&mut line) => {
                let n = match result { Ok(n) => n, Err(_) => break };
                if n == 0 { break; }
                let trimmed = line.trim();
                if trimmed.is_empty() { line.clear(); continue; }

                // Parse as JSON-RPC request
                let req: Request = match serde_json::from_str(trimmed) {
                    Ok(r) => r,
                    Err(_) => {
                        let resp = Response::error(None, -32600, "Invalid JSON-RPC request");
                        let mut response = serde_json::to_string(&resp).unwrap();
                        response.push('\n');
                        writer.write_all(response.as_bytes()).await?;
                        writer.flush().await?;
                        line.clear();
                        continue;
                    }
                };

                // Handle Subscribe specially (event subscription mode)
                if req.method == "Subscribe" {
                    subscribed = true;
                    let resp = Response::success(
                        req.id,
                        serde_json::json!({"subscribed": true, "time_ms": now_ms()}),
                    );
                    let mut response = serde_json::to_string(&resp).unwrap();
                    response.push('\n');
                    writer.write_all(response.as_bytes()).await?;
                    writer.flush().await?;
                    line.clear();
                    continue;
                }

                // Execute command
                let resp = server.execute(req.method, req.params, req.id).await;
                let mut response = serde_json::to_string(&resp).unwrap();
                response.push('\n');
                writer.write_all(response.as_bytes()).await?;
                writer.flush().await?;
                line.clear();
            }
            _ = async { 
                if let Some(ref mut rx) = event_rx { 
                    let _ = rx.recv().await; 
                }
            } => {}
        }
        if subscribed {
            if let Some(ref mut rx) = event_rx {
                while let Ok(evt) = rx.try_recv() {
                    // NDJSON event format
                    let event_json = serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": "event",
                        "params": evt,
                    });
                    let mut response = serde_json::to_string(&event_json).unwrap();
                    response.push('\n');
                    writer.write_all(response.as_bytes()).await?;
                    writer.flush().await?;
                }
            }
        }
    }
    Ok(())
}

impl JsonServer {
    pub async fn execute(&self, method: String, params: serde_json::Value, id: Option<serde_json::Value>) -> Response {
        let cmd: Command = match serde_json::from_value(serde_json::json!({ "method": method, "params": params })) {
            Ok(c) => c,
            Err(e) => return Response::error(id, -32602, &format!("Invalid params: {}", e)),
        };
        debug!("JSON-RPC method: {:?}", cmd);
        match cmd {
            Command::Hello { client, version } => {
                Response::success(id, serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "time_ms": now_ms(),
                    "client": client,
                    "client_version": version,
                }))
            }
            Command::Ping => {
                Response::success(id, serde_json::json!({"time_ms": now_ms()}))
            }
            Command::Get { key } => self.handle_get(key, id),
            Command::Set { key, value } => self.handle_set(key, value, id),
            Command::Sysinfo => self.sysinfo().await,
            Command::Audio { action, device, percent } => self.handle_audio(action, device, percent, id).await,
            Command::BrightnessGet => self.brightness_get().await,
            Command::BrightnessSet { value } => self.brightness_set(value).await,
            Command::BrightnessInc { value } => self.brightness_inc(value).await,
            Command::BrightnessDec { value } => self.brightness_dec(value).await,
            Command::NetStatus => self.net_status().await,
            Command::NetWifiList => self.net_wifi_list().await,
            Command::NetWifiDetails => self.net_wifi_details().await,
            Command::NetWifiConnect { ssid, password } => self.net_wifi_connect(ssid, password).await,
            Command::NetWifiDisconnect => self.net_wifi_disconnect().await,
            Command::NetWifiScan => self.net_wifi_scan().await,
            Command::NetWifiForget { ssid } => self.net_wifi_forget(ssid).await,
            Command::NetHotspotStart => self.net_hotspot_start().await,
            Command::NetHotspotStop => self.net_hotspot_stop().await,
            Command::NetHotspotStatus => self.net_hotspot_status().await,
            Command::NetPower { enabled } => self.net_power(enabled).await,
            Command::NetEthList => self.net_eth_list().await,
            Command::NetEthConnect { interface } => self.net_eth_connect(interface).await,
            Command::NetEthDisconnect => self.net_eth_disconnect().await,
            Command::NetEthDetails { interface } => self.net_eth_details(interface).await,
            Command::BtStatus => self.bt_status().await,
            Command::BtDevices => self.bt_devices().await,
            Command::BtScan => self.bt_scan().await,
            Command::BtConnect { address } => self.bt_connect(address).await,
            Command::BtDisconnect { address } => self.bt_disconnect(address).await,
            Command::BtPower { enabled } => self.bt_power(enabled).await,
            Command::BtPair { address } => self.bt_pair(address).await,
            Command::BtRemove { address } => self.bt_remove(address).await,
            Command::BtDiscoverable { enabled } => self.bt_discoverable(enabled).await,
            Command::BtTrust { address, trusted } => self.bt_trust(address, trusted).await,
            Command::BtAlias { address, alias } => self.bt_alias(address, alias).await,
            Command::BtPairable { enabled } => self.bt_pairable(enabled).await,
            Command::WallpaperStatus => self.wallpaper_status().await,
            Command::WallpaperSet { path, monitor, mode, transition } => self.wallpaper_set(path, monitor, mode, transition).await,
            Command::WallpaperGet { monitor } => self.wallpaper_get(monitor).await,
        }
    }

    async fn handle_audio(&self, action: Option<String>, _device: Option<String>, percent: Option<u32>, id: Option<serde_json::Value>) -> Response {
        let action = action.unwrap_or_default();
        match action.as_str() {
            "sinks" => {
                match crawl_audio::list_sinks(&crawl_audio::Config::default()).await {
                    Ok(devices) => Response::success(id, serde_json::to_value(devices).unwrap_or_default()),
                    Err(e) => Response::error(id, -32000, &e.to_string()),
                }
            }
            "sources" => {
                match crawl_audio::list_sources(&crawl_audio::Config::default()).await {
                    Ok(devices) => Response::success(id, serde_json::to_value(devices).unwrap_or_default()),
                    Err(e) => Response::error(id, -32000, &e.to_string()),
                }
            }
            "volume" => {
                if let Some(p) = percent {
                    match crawl_audio::set_volume(&crawl_audio::Config::default(), p).await {
                        Ok(_) => Response::success(id, serde_json::json!({"ok": true})),
                        Err(e) => Response::error(id, -32000, &e.to_string()),
                    }
                } else {
                    Response::error(id, -32602, "percent required")
                }
            }
            "input_volume" => {
                if let Some(p) = percent {
                    match crawl_audio::set_input_volume(&crawl_audio::Config::default(), p).await {
                        Ok(_) => Response::success(id, serde_json::json!({"ok": true})),
                        Err(e) => Response::error(id, -32000, &e.to_string()),
                    }
                } else {
                    Response::error(id, -32602, "percent required")
                }
            }
            "mute" => {
                match crawl_audio::toggle_mute(&crawl_audio::Config::default()).await {
                    Ok(state) => Response::success(id, serde_json::json!({"muted": state})),
                    Err(e) => Response::error(id, -32000, &e.to_string()),
                }
            }
            "unmute" => {
                match crawl_audio::toggle_output_mute(&crawl_audio::Config::default()).await {
                    Ok(state) => Response::success(id, serde_json::json!({"muted": state})),
                    Err(e) => Response::error(id, -32000, &e.to_string()),
                }
            }
            _ => Response::error(id, -32602, "unknown action"),
        }
    }

    fn handle_get(&self, key: Option<String>, id: Option<serde_json::Value>) -> Response {
        if key.is_none() {
            let content = std::fs::read_to_string(&self.config_path).unwrap_or_default();
            return Response::success(id, serde_json::json!({ "config": content }));
        }
        let k = key.unwrap();
        let content = std::fs::read_to_string(&self.config_path).unwrap_or_default();
        let section = k.split('.').next().unwrap_or(&k);
        let target_key = k.split('.').nth(1).unwrap_or(&k);
        let mut in_section = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed == format!("[{}]", section) { in_section = true; continue; }
            if in_section && trimmed.starts_with('[') && trimmed.ends_with(']') { break; }
            if in_section && trimmed.starts_with(target_key) && trimmed.contains('=') {
                if let Some(val) = trimmed.splitn(2, '=').nth(1) {
                    return Response::success(id, serde_json::json!({ "key": k, "value": val.trim() }));
                }
            }
        }
        Response::error(id, -32601, "key not found")
    }

    fn handle_set(&self, key: String, value: serde_json::Value, id: Option<serde_json::Value>) -> Response {
        let content = match std::fs::read_to_string(&self.config_path) {
            Ok(c) => c,
            Err(e) => return Response::error(id, -32000, &e.to_string()),
        };
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let section = key.split('.').next().unwrap_or(&key).to_string();
        let target_key = key.split('.').nth(1).unwrap_or(&key);
        let value_str = value.to_string();
        let new_line = format!("{} = {}", target_key, value_str);
        let mut in_section = false;
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed == format!("[{}]", section) { in_section = true; continue; }
            if in_section && trimmed.starts_with('[') && trimmed.ends_with(']') { in_section = false; continue; }
            if in_section && trimmed.starts_with(target_key) && trimmed.contains('=') {
                lines[i] = new_line.clone();
                let new_content = lines.join("\n");
                if let Err(e) = std::fs::write(&self.config_path, new_content) {
                    return Response::error(id, -32000, &e.to_string());
                }
                return Response::success(id, serde_json::json!({ "ok": true }));
            }
        }
        Response::error(id, -32601, "section not found")
    }
    
    // --- Brightness handlers
    async fn brightness_get(&self) -> Response {
        if let Some(s) = self.get_state().await {
            let backlight = match crawl_display::Backlight::open(&s.config.display) {
                Ok(b) => b,
                Err(e) => return Response::error(None, -32000, &e.to_string()),
            };
            match backlight.status() {
                Ok(status) => Response::success(None, serde_json::to_value(status).unwrap_or_default()),
                Err(e) => Response::error(None, -32000, &e.to_string()),
            }
        } else { Response::error(None, -32000, "no state") }
    }

    async fn brightness_set(&self, value: i32) -> Response {
        let value = value as f32;
        if let Some(s) = self.get_state().await {
            let backlight = match crawl_display::Backlight::open(&s.config.display) {
                Ok(b) => b,
                Err(e) => return Response::error(None, -32000, &e.to_string()),
            };
            match backlight.set_percent(value) {
                Ok(status) => {
                    let _ = s.event_tx.send(CrawlEvent::Brightness(crawl_ipc::events::BrightnessEvent::Changed { status: status.clone() }));
                    Response::success(None, serde_json::to_value(status).unwrap_or_default())
                }
                Err(e) => Response::error(None, -32000, &e.to_string()),
            }
        } else { Response::error(None, -32000, "no state") }
    }

    async fn brightness_inc(&self, value: i32) -> Response {
        let delta = value as f32;
        if let Some(s) = self.get_state().await {
            let backlight = match crawl_display::Backlight::open(&s.config.display) {
                Ok(b) => b,
                Err(e) => return Response::error(None, -32000, &e.to_string()),
            };
            match backlight.adjust_percent(delta) {
                Ok(status) => {
                    let _ = s.event_tx.send(CrawlEvent::Brightness(crawl_ipc::events::BrightnessEvent::Changed { status: status.clone() }));
                    Response::success(None, serde_json::to_value(status).unwrap_or_default())
                }
                Err(e) => Response::error(None, -32000, &e.to_string()),
            }
        } else { Response::error(None, -32000, "no state") }
    }

    async fn brightness_dec(&self, value: i32) -> Response {
        let delta = -(value as f32);
        if let Some(s) = self.get_state().await {
            let backlight = match crawl_display::Backlight::open(&s.config.display) {
                Ok(b) => b,
                Err(e) => return Response::error(None, -32000, &e.to_string()),
            };
            match backlight.adjust_percent(delta) {
                Ok(status) => {
                    let _ = s.event_tx.send(CrawlEvent::Brightness(crawl_ipc::events::BrightnessEvent::Changed { status: status.clone() }));
                    Response::success(None, serde_json::to_value(status).unwrap_or_default())
                }
                Err(e) => Response::error(None, -32000, &e.to_string()),
            }
        } else { Response::error(None, -32000, "no state") }
    }
    
    // --- Bluetooth handlers -----
    async fn bt_status(&self) -> Response {
        match crawl_bluetooth::get_status().await {
            Ok(s) => Response::success(None, serde_json::to_value(s).unwrap_or_default()),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }

    async fn bt_devices(&self) -> Response {
        match crawl_bluetooth::get_devices().await {
            Ok(d) => Response::success(None, serde_json::to_value(d).unwrap_or_default()),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }

    async fn bt_scan(&self) -> Response {
        match crawl_bluetooth::scan().await {
            Ok(()) => Response::success(None, serde_json::json!({ "ok": true })),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }

    async fn bt_connect(&self, address: String) -> Response {
        match crawl_bluetooth::connect(&address).await {
            Ok(()) => Response::success(None, serde_json::json!({ "ok": true })),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }

    async fn bt_disconnect(&self, address: String) -> Response {
        match crawl_bluetooth::disconnect(&address).await {
            Ok(()) => Response::success(None, serde_json::json!({ "ok": true })),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }

    async fn bt_power(&self, enabled: bool) -> Response {
        match crawl_bluetooth::set_powered(enabled).await {
            Ok(()) => Response::success(None, serde_json::json!({ "ok": true })),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }
    
    // --- Network handlers -----
    async fn net_status(&self) -> Response {
        match crawl_network::get_status().await {
            Ok(status) => Response::success(None, serde_json::to_value(status).unwrap_or_default()),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }

    async fn net_wifi_list(&self) -> Response {
        match crawl_network::list_wifi().await {
            Ok(list) => Response::success(None, serde_json::to_value(list).unwrap_or_default()),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }

    async fn net_wifi_connect(&self, ssid: String, password: Option<String>) -> Response {
        match crawl_network::connect_wifi(&ssid, password.as_deref()).await {
            Ok(()) => Response::success(None, serde_json::json!({ "ok": true })),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }

    async fn net_wifi_disconnect(&self) -> Response {
        match crawl_network::disconnect_wifi().await {
            Ok(()) => Response::success(None, serde_json::json!({ "ok": true })),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }

    async fn net_wifi_scan(&self) -> Response {
        Response::success(None, serde_json::json!({ "ok": true }))
    }

    async fn net_wifi_forget(&self, _ssid: String) -> Response {
        Response::success(None, serde_json::json!({ "ok": true }))
    }

    async fn net_hotspot_start(&self) -> Response {
        Response::error(None, -32000, "hotspot not supported")
    }

    async fn net_hotspot_stop(&self) -> Response {
        match crawl_network::stop_hotspot().await {
            Ok(()) => Response::success(None, serde_json::json!({ "ok": true })),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }

    async fn net_hotspot_status(&self) -> Response {
        match crawl_network::hotspot_status().await {
            Ok(status) => Response::success(None, serde_json::to_value(status).unwrap_or_default()),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }

    async fn net_power(&self, _enabled: bool) -> Response {
        Response::success(None, serde_json::json!({ "ok": true }))
    }

    async fn net_eth_list(&self) -> Response {
        match crawl_network::list_ethernet().await {
            Ok(list) => Response::success(None, serde_json::to_value(list).unwrap_or_default()),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }

    async fn net_eth_connect(&self, interface: String) -> Response {
        match crawl_network::connect_ethernet(Some(&interface)).await {
            Ok(iface) => Response::success(None, serde_json::json!({ "ok": true, "interface": iface })),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }

    async fn net_eth_disconnect(&self) -> Response {
        match crawl_network::disconnect_ethernet(None).await {
            Ok(iface) => Response::success(None, serde_json::json!({ "ok": true, "interface": iface })),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }

    async fn net_eth_details(&self, interface: String) -> Response {
        match crawl_network::get_ethernet_details(Some(&interface)).await {
            Ok(details) => Response::success(None, serde_json::to_value(details).unwrap_or_default()),
            Err(e) => Response::error(None, -32000, &e.to_string()),
        }
    }
    
    // ── Wallpaper handlers ─────────────────────────────────────────────────

    async fn wallpaper_status(&self) -> Response {
        if let Some(s) = self.get_state().await {
            let state = s.wallpaper_service.get_state().await;
            Response::success(None, serde_json::to_value(state).unwrap_or_default())
        } else {
            Response::error(None, -32000, "no state")
        }
    }

    async fn wallpaper_set(
        &self,
        path: String,
        monitor: Option<String>,
        mode: Option<String>,
        transition: Option<String>,
    ) -> Response {
        if let Some(s) = self.get_state().await {
            let mode = match mode.as_deref().unwrap_or("fill") {
                "fit" => crawl_display::WallpaperMode::Fit,
                "stretch" => crawl_display::WallpaperMode::Stretch,
                "center" => crawl_display::WallpaperMode::Center,
                "tile" => crawl_display::WallpaperMode::Tile,
                _ => crawl_display::WallpaperMode::Fill,
            };
            let request = crawl_display::SetWallpaperRequest {
                path,
                monitor,
                mode,
                wallpaper_transition: transition.unwrap_or_default(),
                wallpaper_transition_duration_ms: 500,
                wallpaper_transition_fps: 30,
            };
            match s.wallpaper_service.set_wallpaper(request).await {
                Ok(()) => Response::success(None, serde_json::json!({"ok": true})),
                Err(e) => Response::error(None, -32000, &e.to_string()),
            }
        } else {
            Response::error(None, -32000, "no state")
        }
    }

    async fn wallpaper_get(&self, monitor: Option<String>) -> Response {
        if let Some(s) = self.get_state().await {
            let wallpaper = s
                .wallpaper_service
                .get_wallpaper(monitor.as_deref())
                .await;
            Response::success(
                None,
                serde_json::json!({ "wallpaper": wallpaper }),
            )
        } else {
            Response::error(None, -32000, "no state")
        }
    }

    // --- System info handler ---
    async fn sysinfo(&self) -> Response {
        let info = crawl_sysinfo::get_info();
        Response::success(None, serde_json::to_value(info).unwrap_or_default())
    }
}