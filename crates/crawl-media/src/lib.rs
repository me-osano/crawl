//! crawl-media: MPRIS2 media player aggregator.
//!
//! Discovers all org.mpris.MediaPlayer2.* names on the session bus,
//! watches their properties, and exposes a unified media control API.
//! Equivalent to playerctl but native, async, and integrated with crawl.

use crawl_ipc::{
    events::{CrawlEvent, MediaEvent},
    types::{MediaPlayer, PlaybackStatus},
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{debug, info};
use zbus::{proxy, Connection};

// ── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Preferred player to treat as "active" when multiple are running.
    /// Empty = most recently active.
    pub preferred_player: String,
}

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("D-Bus error: {0}")]
    DBus(#[from] zbus::Error),
    #[error("no active player found")]
    NoPlayer,
    #[error("player not found: {0}")]
    PlayerNotFound(String),
}

// ── D-Bus proxies ─────────────────────────────────────────────────────────────

/// org.mpris.MediaPlayer2.Player interface proxy
#[proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait MprisPlayer {
    fn next(&self)       -> zbus::Result<()>;
    fn previous(&self)   -> zbus::Result<()>;
    fn play(&self)       -> zbus::Result<()>;
    fn pause(&self)      -> zbus::Result<()>;
    fn play_pause(&self) -> zbus::Result<()>;
    fn stop(&self)       -> zbus::Result<()>;
    fn seek(&self, offset: i64) -> zbus::Result<()>;

    #[zbus(property)]
    fn playback_status(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn metadata(&self) -> zbus::Result<HashMap<String, zbus::zvariant::OwnedValue>>;

    #[zbus(property)]
    fn volume(&self) -> zbus::Result<f64>;

    #[zbus(property)]
    fn set_volume(&self, volume: f64) -> zbus::Result<()>;

    #[zbus(property)]
    fn position(&self) -> zbus::Result<i64>;

    #[zbus(property)]
    fn can_play(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn can_pause(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn can_go_next(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn can_go_previous(&self) -> zbus::Result<bool>;
}

/// org.mpris.MediaPlayer2 (root) interface proxy
#[proxy(
    interface = "org.mpris.MediaPlayer2",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait MprisRoot {
    #[zbus(property)]
    fn identity(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn desktop_entry(&self) -> zbus::Result<String>;
}

// ── Domain runner ─────────────────────────────────────────────────────────────

pub async fn run(_cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    info!("crawl-media starting");

    let conn = Connection::session().await?;

    // Watch for bus name changes to detect player appear/vanish
    let dbus_proxy = zbus::fdo::DBusProxy::new(&conn).await?;
    let mut name_owner_changed = dbus_proxy.receive_name_owner_changed().await?;

    // Enumerate existing MPRIS players on startup
    let names = dbus_proxy.list_names().await?;
    for name in &names {
        if name.starts_with("org.mpris.MediaPlayer2.") {
            let bus_name = name.to_string();
            if let Ok(player) = build_player_info(&conn, &bus_name).await {
                info!(player = %bus_name, "found existing MPRIS player");
                let _ = tx.send(CrawlEvent::Media(MediaEvent::PlayerAppeared { player }));
            }
        }
    }

    // Watch for new players appearing and vanishing
    while let Some(signal) = name_owner_changed.next().await {
        let args = signal.args()?;
        let name = args.name.to_string();

        if !name.starts_with("org.mpris.MediaPlayer2.") {
            continue;
        }

        let new_owner = args.new_owner.as_deref().unwrap_or("").to_string();
        let old_owner = args.old_owner.as_deref().unwrap_or("").to_string();

        if old_owner.is_empty() && !new_owner.is_empty() {
            // Player appeared
            debug!(player = %name, "MPRIS player appeared");
            let player_result = build_player_info(&conn, &name).await;
            if let Ok(player) = player_result {
                let _ = tx.send(CrawlEvent::Media(MediaEvent::PlayerAppeared { player }));
            }
        } else if !old_owner.is_empty() && new_owner.is_empty() {
            // Player vanished
            debug!(player = %name, "MPRIS player vanished");
            let _ = tx.send(CrawlEvent::Media(MediaEvent::PlayerVanished { bus_name: name }));
        }
    }

    Ok(())
}

// ── Builder ───────────────────────────────────────────────────────────────────

async fn build_player_info(conn: &Connection, bus_name: &str) -> Result<MediaPlayer, MediaError> {
    let player_proxy = MprisPlayerProxy::builder(conn)
        .destination(bus_name)?
        .build()
        .await?;

    let root_proxy = MprisRootProxy::builder(conn)
        .destination(bus_name)?
        .build()
        .await?;

    let status_str = player_proxy.playback_status().await.unwrap_or_default();
    let status = match status_str.as_str() {
        "Playing" => PlaybackStatus::Playing,
        "Paused"  => PlaybackStatus::Paused,
        _         => PlaybackStatus::Stopped,
    };

    let metadata = player_proxy.metadata().await.unwrap_or_default();
    let title    = extract_string(&metadata, "xesam:title");
    let artist   = extract_string_list(&metadata, "xesam:artist");
    let album    = extract_string(&metadata, "xesam:album");
    let art_url  = extract_string(&metadata, "mpris:artUrl");
    let length   = extract_i64(&metadata, "mpris:length");

    let player_name = root_proxy.identity().await
        .unwrap_or_else(|_| bus_name.trim_start_matches("org.mpris.MediaPlayer2.").to_string());

    let position_us = player_proxy.position().await.ok();
    let volume = player_proxy.volume().await.ok();
    let can_play = player_proxy.can_play().await.unwrap_or(false);
    let can_pause = player_proxy.can_pause().await.unwrap_or(false);
    let can_next = player_proxy.can_go_next().await.unwrap_or(false);
    let can_prev = player_proxy.can_go_previous().await.unwrap_or(false);

    Ok(MediaPlayer {
        player_name,
        bus_name: bus_name.to_string(),
        status,
        title,
        artist,
        album,
        art_url,
        position_us,
        length_us: length,
        volume,
        can_play,
        can_pause,
        can_next,
        can_prev,
    })
}

// ── Metadata helpers ──────────────────────────────────────────────────────────

fn extract_string(meta: &HashMap<String, zbus::zvariant::OwnedValue>, key: &str) -> Option<String> {
    meta.get(key)?
        .downcast_ref::<zbus::zvariant::Str>()
        .ok()
        .map(|s| s.as_str().to_string())
}

fn extract_string_list(meta: &HashMap<String, zbus::zvariant::OwnedValue>, key: &str) -> Option<String> {
    let val = meta.get(key)?;
    if let Ok(s) = val.downcast_ref::<zbus::zvariant::Str>() {
        return Some(s.as_str().to_string());
    }
    None
}

fn extract_i64(meta: &HashMap<String, zbus::zvariant::OwnedValue>, key: &str) -> Option<i64> {
    meta.get(key)?.downcast_ref::<i64>().ok()
}

// ── Public control API ────────────────────────────────────────────────────────

pub async fn play(bus_name: &str) -> Result<(), MediaError> {
    let conn = Connection::session().await?;
    let proxy = MprisPlayerProxy::builder(&conn).destination(bus_name)?.build().await?;
    proxy.play().await?;
    Ok(())
}

pub async fn pause(bus_name: &str) -> Result<(), MediaError> {
    let conn = Connection::session().await?;
    let proxy = MprisPlayerProxy::builder(&conn).destination(bus_name)?.build().await?;
    proxy.pause().await?;
    Ok(())
}

pub async fn next(bus_name: &str) -> Result<(), MediaError> {
    let conn = Connection::session().await?;
    let proxy = MprisPlayerProxy::builder(&conn).destination(bus_name)?.build().await?;
    proxy.next().await?;
    Ok(())
}

pub async fn previous(bus_name: &str) -> Result<(), MediaError> {
    let conn = Connection::session().await?;
    let proxy = MprisPlayerProxy::builder(&conn).destination(bus_name)?.build().await?;
    proxy.previous().await?;
    Ok(())
}

pub async fn set_volume(bus_name: &str, volume: f64) -> Result<(), MediaError> {
    let conn = Connection::session().await?;
    let proxy = MprisPlayerProxy::builder(&conn).destination(bus_name)?.build().await?;
    proxy.set_volume(volume).await?;
    Ok(())
}
