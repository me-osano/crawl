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
        introspect::{SinkInfo, SourceInfo},
        subscribe::{Facility, InterestMaskSet, Operation},
        Context, FlagSet as ContextFlagSet, State,
    },
    mainloop::{standard::Mainloop, threaded::Mainloop as ThreadedMainloop},
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
    #[error("source not found: {0}")]
    SourceNotFound(String),
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

pub fn source_to_device(source: &SourceInfo) -> AudioDevice {
    AudioDevice {
        id:          source.index,
        name:        source.name.as_deref().unwrap_or("unknown").to_string(),
        description: source.description.as_deref().map(str::to_string),
        kind:        AudioDeviceKind::Source,
        volume_percent: volumes_to_percent(&source.volume),
        muted:       source.mute,
        is_default:  false,
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
    let mut mainloop = ThreadedMainloop::new().ok_or_else(|| AudioError::Connection("failed to create mainloop".into()))?;
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

pub async fn set_volume(cfg: &Config, percent: u32) -> Result<(), AudioError> {
    set_output_volume(cfg, percent).await
}

pub async fn toggle_mute(cfg: &Config) -> Result<bool, AudioError> {
    toggle_output_mute(cfg).await
}

pub async fn set_output_volume(cfg: &Config, percent: u32) -> Result<(), AudioError> {
    let cfg = cfg.clone();
    tokio::task::spawn_blocking(move || set_volume_impl(&cfg, percent, AudioDeviceKind::Sink))
        .await
        .map_err(|e| AudioError::OperationFailed(format!("{e}")))?
}

pub async fn set_input_volume(cfg: &Config, percent: u32) -> Result<(), AudioError> {
    let cfg = cfg.clone();
    tokio::task::spawn_blocking(move || set_volume_impl(&cfg, percent, AudioDeviceKind::Source))
        .await
        .map_err(|e| AudioError::OperationFailed(format!("{e}")))?
}

pub async fn toggle_output_mute(cfg: &Config) -> Result<bool, AudioError> {
    let cfg = cfg.clone();
    tokio::task::spawn_blocking(move || toggle_mute_impl(&cfg, AudioDeviceKind::Sink))
        .await
        .map_err(|e| AudioError::OperationFailed(format!("{e}")))?
}

pub async fn toggle_input_mute(cfg: &Config) -> Result<bool, AudioError> {
    let cfg = cfg.clone();
    tokio::task::spawn_blocking(move || toggle_mute_impl(&cfg, AudioDeviceKind::Source))
        .await
        .map_err(|e| AudioError::OperationFailed(format!("{e}")))?
}

pub async fn list_sinks(cfg: &Config) -> Result<Vec<AudioDevice>, AudioError> {
    let cfg = cfg.clone();
    tokio::task::spawn_blocking(move || list_devices_impl(&cfg, AudioDeviceKind::Sink))
        .await
        .map_err(|e| AudioError::OperationFailed(format!("{e}")))?
}

pub async fn list_sources(cfg: &Config) -> Result<Vec<AudioDevice>, AudioError> {
    let cfg = cfg.clone();
    tokio::task::spawn_blocking(move || list_devices_impl(&cfg, AudioDeviceKind::Source))
        .await
        .map_err(|e| AudioError::OperationFailed(format!("{e}")))?
}

fn connect_mainloop(cfg: &Config) -> Result<(Mainloop, Context), AudioError> {
    let mut mainloop = Mainloop::new()
        .ok_or_else(|| AudioError::Connection("failed to create mainloop".into()))?;
    let mut context = Context::new(&mainloop, &cfg.app_name)
        .ok_or_else(|| AudioError::Connection("failed to create context".into()))?;

    let server = if cfg.server.is_empty() { None } else { Some(cfg.server.as_str()) };
    context
        .connect(server, ContextFlagSet::NOFLAGS, None)
        .map_err(|e| AudioError::Connection(format!("{e:?}")))?;

    loop {
        mainloop.iterate(true);
        match context.get_state() {
            State::Ready => break,
            State::Failed | State::Terminated => {
                return Err(AudioError::Connection("context failed to connect".into()))
            }
            _ => {}
        }
    }

    Ok((mainloop, context))
}

fn wait_op<T: ?Sized>(mainloop: &mut Mainloop, op: &pulse::operation::Operation<T>) -> Result<(), AudioError> {
    use pulse::operation::State as OpState;
    while op.get_state() == OpState::Running {
        mainloop.iterate(true);
    }
    match op.get_state() {
        OpState::Done => Ok(()),
        _ => Err(AudioError::OperationFailed("operation cancelled".into())),
    }
}

fn get_default_names(mainloop: &mut Mainloop, context: &Context) -> Result<(Option<String>, Option<String>), AudioError> {
    let introspect = context.introspect();
    let default_names = std::rc::Rc::new(std::cell::RefCell::new((None, None)));
    let default_names_ref = default_names.clone();
    let op = introspect.get_server_info(move |info| {
        let sink = info.default_sink_name.as_deref().map(str::to_string);
        let source = info.default_source_name.as_deref().map(str::to_string);
        *default_names_ref.borrow_mut() = (sink, source);
    });
    wait_op(mainloop, &op)?;
    Ok(default_names.borrow().clone())
}

fn list_devices_impl(cfg: &Config, kind: AudioDeviceKind) -> Result<Vec<AudioDevice>, AudioError> {
    let (mut mainloop, context) = connect_mainloop(cfg)?;
    let (default_sink, default_source) = get_default_names(&mut mainloop, &context)?;
    let introspect = context.introspect();

    let devices: std::rc::Rc<std::cell::RefCell<Vec<AudioDevice>>> =
        std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let devices_ref = devices.clone();

    match kind {
        AudioDeviceKind::Sink => {
            let op = introspect.get_sink_info_list(move |result| {
                if let ListResult::Item(sink) = result {
                    let mut dev = sink_to_device(sink);
                    if default_sink.as_deref() == sink.name.as_deref() {
                        dev.is_default = true;
                    }
                    devices_ref.borrow_mut().push(dev);
                }
            });
            wait_op(&mut mainloop, &op)?;
        }
        AudioDeviceKind::Source => {
            let op = introspect.get_source_info_list(move |result| {
                if let ListResult::Item(source) = result {
                    let mut dev = source_to_device(source);
                    if default_source.as_deref() == source.name.as_deref() {
                        dev.is_default = true;
                    }
                    devices_ref.borrow_mut().push(dev);
                }
            });
            wait_op(&mut mainloop, &op)?;
        }
    }

    Ok(devices.borrow().clone())
}

fn get_default_device_name(
    mainloop: &mut Mainloop,
    context: &Context,
    kind: AudioDeviceKind,
) -> Result<String, AudioError> {
    let (default_sink, default_source) = get_default_names(mainloop, context)?;
    match kind {
        AudioDeviceKind::Sink => default_sink.ok_or_else(|| AudioError::SinkNotFound("default sink".into())),
        AudioDeviceKind::Source => default_source.ok_or_else(|| AudioError::SourceNotFound("default source".into())),
    }
}

fn get_device_info(
    mainloop: &mut Mainloop,
    context: &Context,
    name: &str,
    kind: AudioDeviceKind,
) -> Result<(u8, bool), AudioError> {
    match kind {
        AudioDeviceKind::Sink => get_sink_info(mainloop, context, name),
        AudioDeviceKind::Source => get_source_info(mainloop, context, name),
    }
}

fn get_sink_info(
    mainloop: &mut Mainloop,
    context: &Context,
    name: &str,
) -> Result<(u8, bool), AudioError> {
    let introspect = context.introspect();
    let info_ref = std::rc::Rc::new(std::cell::RefCell::new(None));
    let info_ref_cloned = info_ref.clone();
    let op = introspect.get_sink_info_by_name(name, move |result| {
        if let ListResult::Item(sink) = result {
            let channels = sink.channel_map.len();
            *info_ref_cloned.borrow_mut() = Some((channels, sink.mute));
        }
    });
    wait_op(mainloop, &op)?;
    info_ref.borrow().clone().ok_or_else(|| AudioError::SinkNotFound(name.into()))
}

fn get_source_info(
    mainloop: &mut Mainloop,
    context: &Context,
    name: &str,
) -> Result<(u8, bool), AudioError> {
    let introspect = context.introspect();
    let info_ref = std::rc::Rc::new(std::cell::RefCell::new(None));
    let info_ref_cloned = info_ref.clone();
    let op = introspect.get_source_info_by_name(name, move |result| {
        if let ListResult::Item(source) = result {
            let channels = source.channel_map.len();
            *info_ref_cloned.borrow_mut() = Some((channels, source.mute));
        }
    });
    wait_op(mainloop, &op)?;
    info_ref.borrow().clone().ok_or_else(|| AudioError::SourceNotFound(name.into()))
}

fn set_volume_impl(cfg: &Config, percent: u32, kind: AudioDeviceKind) -> Result<(), AudioError> {
    let (mut mainloop, context) = connect_mainloop(cfg)?;
    let name = get_default_device_name(&mut mainloop, &context, kind.clone())?;
    let (channels, _) = get_device_info(&mut mainloop, &context, &name, kind.clone())?;
    let volumes = percent_to_volumes(channels, percent);
    let mut introspect = context.introspect();
    let op = match kind {
        AudioDeviceKind::Sink => introspect.set_sink_volume_by_name(&name, &volumes, None),
        AudioDeviceKind::Source => introspect.set_source_volume_by_name(&name, &volumes, None),
    };
    wait_op(&mut mainloop, &op)
}

fn toggle_mute_impl(cfg: &Config, kind: AudioDeviceKind) -> Result<bool, AudioError> {
    let (mut mainloop, context) = connect_mainloop(cfg)?;
    let name = get_default_device_name(&mut mainloop, &context, kind.clone())?;
    let (_, muted) = get_device_info(&mut mainloop, &context, &name, kind.clone())?;
    let new_state = !muted;
    let mut introspect = context.introspect();
    let op = match kind {
        AudioDeviceKind::Sink => introspect.set_sink_mute_by_name(&name, new_state, None),
        AudioDeviceKind::Source => introspect.set_source_mute_by_name(&name, new_state, None),
    };
    wait_op(&mut mainloop, &op)?;
    Ok(new_state)
}
