//! crawl-audio: Audio control via PipeWire/PulseAudio.
//!
//! Uses libpulse-binding (the safe Rust wrapper around libpulse).
//! Works transparently with PipeWire's PulseAudio compatibility layer.
//!
//! The domain runner bridges libpulse's callback-based API into tokio
//! using a dedicated thread + channel pattern.

use crawl_ipc::{
    events::{AudioEvent, CrawlEvent},
    types::{AudioDevice, AudioDeviceKind},
};
use libpulse_binding as pulse;
use pulse::{
    callbacks::ListResult,
    context::{
        introspect::SinkInfo,
        subscribe::{Facility, InterestMaskSet, Operation},
        Context, FlagSet as ContextFlagSet, State,
    },
    mainloop::threaded::Mainloop,
    volume::{ChannelVolumes, Volume},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::{error, info};

// ── Config ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// PulseAudio server address. Empty = default (respects PULSE_SERVER env)
    pub server: String,
    /// Application name reported to PulseAudio
    pub app_name: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: String::new(),
            app_name: "crawl".into(),
        }
    }
}

// ── Error ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("PulseAudio connection failed: {0}")]
    Connection(String),
    #[error("sink not found: {0}")]
    SinkNotFound(String),
    #[error("operation failed: {0}")]
    OperationFailed(String),
}

// ── Volume helpers ────────────────────────────────────────────────────────────

/// Convert a PA ChannelVolumes to a 0–100 percent integer.
pub fn volumes_to_percent(vol: &ChannelVolumes) -> u32 {
    let avg = vol.avg();
    ((avg.0 as f64 / Volume::NORMAL.0 as f64) * 100.0).round() as u32
}

/// Build a uniform ChannelVolumes from a percent value.
pub fn percent_to_volumes(channels: u8, percent: u32) -> ChannelVolumes {
    let raw = ((percent as f64 / 100.0) * Volume::NORMAL.0 as f64).round() as u32;
    let vol = Volume(raw.min(Volume::MAX.0));
    let mut cv = ChannelVolumes::default();
    cv.set(channels, vol);
    cv
}

/// Convert a PA SinkInfo into our shared AudioDevice type.
pub fn sink_to_device(sink: &SinkInfo) -> AudioDevice {
    AudioDevice {
        id:          sink.index,
        name:        sink.name.as_deref().unwrap_or("unknown").to_string(),
        description: sink.description.as_deref().map(str::to_string),
        kind:        AudioDeviceKind::Sink,
        volume_percent: volumes_to_percent(&sink.volume),
        muted:       sink.mute,
        is_default:  false, // caller sets this based on default sink name
    }
}

// ── Domain runner ─────────────────────────────────────────────────────────────

pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()> {
    info!("crawl-audio starting");

    // libpulse is synchronous and callback-driven; run it on a dedicated thread.
    let tx_clone = tx.clone();
    let cfg_clone = cfg.clone();

    tokio::task::spawn_blocking(move || {
        if let Err(e) = pulse_thread(cfg_clone, tx_clone) {
            error!("audio thread failed: {e}");
        }
    }).await?;

    Ok(())
}

fn pulse_thread(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> Result<(), AudioError> {
    let mut mainloop = Mainloop::new().ok_or_else(|| AudioError::Connection("failed to create mainloop".into()))?;
    let mut context  = Context::new(&mainloop, &cfg.app_name)
        .ok_or_else(|| AudioError::Connection("failed to create context".into()))?;

    let server = if cfg.server.is_empty() { None } else { Some(cfg.server.as_str()) };
    context.connect(server, ContextFlagSet::NOFLAGS, None)
        .map_err(|e| AudioError::Connection(format!("{e:?}")))?;

    mainloop.start().map_err(|e| AudioError::Connection(format!("{e:?}")))?;

    // Wait for context to become ready
    loop {
        match context.get_state() {
            State::Ready    => break,
            State::Failed | State::Terminated => {
                return Err(AudioError::Connection("context failed to connect".into()));
            }
            _ => mainloop.wait(),
        }
    }

    info!("connected to PulseAudio/PipeWire");

    // Subscribe to sink and sink-input events
    let tx2 = tx.clone();
    context.set_subscribe_callback(Some(Box::new(move |facility, op, index| {
        if let (Some(Facility::Sink), Some(op)) = (facility, op) {
            match op {
                Operation::Changed => {
                    // TODO: re-query the specific sink and emit VolumeChanged / MuteToggled
                    // Requires re-entering the context from this callback — best done with
                    // a channel to a separate query coroutine.
                }
                Operation::New => {
                    // TODO: emit DeviceAdded
                }
                Operation::Removed => {
                    let _ = tx2.send(CrawlEvent::Audio(AudioEvent::DeviceRemoved { id: index }));
                }
            }
        }
    })));

    context.subscribe(InterestMaskSet::SINK | InterestMaskSet::SINK_INPUT, |_| {});

    // Enumerate initial sinks
    let introspect = context.introspect();
    let tx3 = tx.clone();

    introspect.get_sink_info_list(move |result| {
        if let ListResult::Item(sink) = result {
            let dev = sink_to_device(sink);
            let _ = tx3.send(CrawlEvent::Audio(AudioEvent::DeviceAdded { device: dev }));
        }
    });

    // Run event loop - mainloop runs in its own thread
    // Use a simple blocking loop to keep the thread alive
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}

// ── Public query / control API ────────────────────────────────────────────────

/// Set the default sink volume (percent 0–100).
/// Note: libpulse operations are inherently async-via-callback.
/// For simplicity this uses a oneshot channel to bridge back to async.
pub async fn set_volume(_percent: u32) -> Result<(), AudioError> {
    // TODO: implement via spawn_blocking + PA introspect.set_sink_volume_by_name
    // Pattern:
    //   1. connect to PA
    //   2. get default sink name
    //   3. build ChannelVolumes via percent_to_volumes()
    //   4. introspect.set_sink_volume_by_name(name, &cv, callback)
    //   5. signal completion via oneshot
    Ok(())
}

pub async fn toggle_mute() -> Result<bool, AudioError> {
    // TODO: introspect.set_sink_mute_by_name
    Ok(false)
}

pub async fn list_sinks() -> Result<Vec<AudioDevice>, AudioError> {
    // TODO: introspect.get_sink_info_list in spawn_blocking
    Ok(vec![])
}

pub async fn list_sources() -> Result<Vec<AudioDevice>, AudioError> {
    // TODO: introspect.get_source_info_list in spawn_blocking
    Ok(vec![])
}
