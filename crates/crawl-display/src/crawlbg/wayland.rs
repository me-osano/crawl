//! Wayland backend for crawlbg using smithay-client-toolkit.
//!
//! Uses wlr-layer-shell to create fullscreen background surfaces.
//! Tracks output scale factors for high-DPI support.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::Duration;
use anyhow::{Context, Result};
use image::DynamicImage;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        wlr_layer::{
            Anchor, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{
        slot::SlotPool,
        Shm, ShmHandler,
    },
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_shm, wl_surface},
    Connection, QueueHandle,
};
use tracing::{error, info, warn};

use super::transition::{TransitionConfig, TransitionKind, generate_frames};
use super::models::WallpaperMode;
use super::outputs::logical_size;
use super::renderer::Renderer;

// ── Command channel ─────────────────────────────────────────
pub enum BackendCmd {
    DisplayImage {
        output_name: Option<String>,
        path: std::path::PathBuf,
        cfg: TransitionConfig,
        mode: WallpaperMode,
        fps: u32,
        image_data: Option<DynamicImage>,
    },
    Shutdown,
}

// ── Public handle ────────────────────────────────────────────
pub struct WaylandBackend {
    tx: SyncSender<BackendCmd>,
}

impl WaylandBackend {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel::<BackendCmd>(32);
        thread::Builder::new()
            .name("crawlbg-wayland".into())
            .spawn(move || {
                if let Err(e) = wayland_thread(rx) {
                    error!("wayland thread exited: {e:#}");
                }
            })
            .expect("spawn wayland thread");
        Self { tx }
    }

    pub fn display_image(
        &self,
        output: &str,
        path: &Path,
        cfg: &TransitionConfig,
        mode: WallpaperMode,
        fps: u32,
        image: Option<DynamicImage>,
    ) {
        info!("display_image: output={} path={} mode={:?} fps={}", output, path.display(), mode, fps);
        let result = self.tx.send(BackendCmd::DisplayImage {
            output_name: if output == "*" { None } else { Some(output.to_owned()) },
            path: path.to_owned(),
            cfg: cfg.clone(),
            mode,
            fps,
            image_data: image,
        });
        if let Err(e) = result {
            error!("failed to send DisplayImage: {}", e);
        }
    }
}

impl Drop for WaylandBackend {
    fn drop(&mut self) {
        let _ = self.tx.send(BackendCmd::Shutdown);
    }
}

// ── Wayland thread ──────────────────────────────────────────
fn wayland_thread(rx: Receiver<BackendCmd>) -> Result<()> {
    let conn = Connection::connect_to_env().context("connect to Wayland")?;
    let (globals, mut event_queue) = registry_queue_init(&conn).context("registry init")?;
    let qh = event_queue.handle();
    let compositor_state = CompositorState::bind(&globals, &qh).context("wl_compositor")?;
    let layer_shell = LayerShell::bind(&globals, &qh).context("wlr-layer-shell")?;
    let shm = Shm::bind(&globals, &qh).context("wl_shm")?;
    let output_state = OutputState::new(&globals, &qh);
    let registry_state = RegistryState::new(&globals);
    let pool = SlotPool::new(1920 * 1080 * 4, &shm).context("create shm pool")?;
    let renderer = Renderer::new();
    let mut state = WaylandState {
        registry_state,
        output_state,
        compositor_state,
        layer_shell,
        shm,
        pool,
        renderer,
        rx,
        qh,
        exit: false,
    };

    event_queue.roundtrip(&mut state).context("initial roundtrip")?;

    // Create surfaces for known outputs
    let outputs: Vec<(String, u32, u32, i32)> = state
        .output_state
        .outputs()
        .filter_map(|o| {
            let info = state.output_state.info(&o)?;
            let name = info.name.clone()?;
            let (w, h) = logical_size(&info);
            Some((name, w, h, info.scale_factor))
        })
        .collect();

    for (name, w, h, scale) in outputs {
        create_surface(&mut state, &name, w, h, scale);
    }

    event_queue.roundtrip(&mut state).context("configure roundtrip")?;
    info!("wayland backend running, {} outputs", state.renderer.surface_names().len());

    loop {
        event_queue.flush().ok();
        loop {
            match state.rx.try_recv() {
                Ok(BackendCmd::Shutdown) => { state.exit = true; break; }
                Ok(BackendCmd::DisplayImage { output_name, path, cfg, mode, fps, image_data }) => {
                    info!("received DisplayImage: output={:?} path={}", output_name, path.display());
                    let to_img = if let Some(img) = image_data {
                        img
                    } else {
                        match image::open(&path) {
                            Ok(img) => img,
                            Err(e) => { error!("load image: {e:#}"); continue; }
                        }
                    };
                    let targets: Vec<String> = match &output_name {
                        Some(n) => vec![n.clone()],
                        None => state.renderer.surface_names(),
                    };
                    for name in targets {
                        blit_transition(&mut state, &name, &to_img, &cfg, mode, fps);
                        event_queue.flush().ok();
                        event_queue.roundtrip(&mut state).ok();
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => { state.exit = true; break; }
            }
        }
        if state.exit { break; }
        event_queue.blocking_dispatch(&mut state).context("dispatch")?;
    }
    Ok(())
}

fn create_surface(state: &mut WaylandState, name: &str, width: u32, height: u32, scale: i32) {
    if state.renderer.get(name).is_some() { return; }
    let surface = state.compositor_state.create_surface(&state.qh);
    let layer = state.layer_shell.create_layer_surface(
        &state.qh, surface, Layer::Background, Some("crawlbg"), None,
    );
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(smithay_client_toolkit::shell::wlr_layer::KeyboardInteractivity::None);
    layer.set_size(0, 0);
    layer.commit();
    state.renderer.get_or_create(name, || {
        Some((layer, width, height, scale))
    });
    info!("created surface for {name} ({width}x{height} scale={scale})");
}

fn blit_transition(
    state: &mut WaylandState,
    output_name: &str,
    to_img: &DynamicImage,
    cfg: &TransitionConfig,
    mode: WallpaperMode,
    fps: u32,
) {
    info!("blit_transition: output={} mode={:?}", output_name, mode);
    let surf = match state.renderer.get_mut(output_name) {
        Some(s) => s,
        None => { warn!("unknown output {output_name}"); return; }
    };
    if !surf.configured {
        let prepared = crate::crawlbg::image::apply_wallpaper_mode(to_img, surf.width, surf.height, mode);
        surf.current_image = Some(prepared);
        return;
    }
    let w = surf.width;
    let h = surf.height;
    let use_transition = !matches!(cfg.kind, TransitionKind::None)
        && surf.current_image.is_some()
        && cfg.duration_ms > 0;
    
    let frames = if use_transition {
        let from = surf.current_image.as_ref().unwrap().clone();
        let to_prepared = crate::crawlbg::image::apply_wallpaper_mode(to_img, w, h, mode);
        generate_frames(&from, &to_prepared, w, h, cfg, false)
    } else {
        vec![crate::crawlbg::image::apply_wallpaper_mode(to_img, w, h, mode)]
    };
    
    let frame_delay_ms = if fps > 0 { 1000 / fps } else { 16 };
    
    for frame in &frames {
        let stride = w as i32 * 4;
        let buf_size = (stride as usize) * h as usize;
        if state.pool.len() < buf_size {
            if let Err(e) = state.pool.resize(buf_size) { error!("resize pool: {e}"); return; }
        }
        let (buffer, canvas) = match state.pool.create_buffer(w as i32, h as i32, stride, wl_shm::Format::Xrgb8888) {
            Ok(v) => v,
            Err(e) => { error!("create_buffer: {e}"); return; }
        };
        crate::crawlbg::renderer::Renderer::blit_frame(canvas, frame, w, h);
        let surf = match state.renderer.get(output_name) {
            Some(s) => s,
            None => return,
        };
        surf.layer.wl_surface().damage_buffer(0, 0, w as i32, h as i32);
        surf.layer.attach(Some(buffer.wl_buffer()), 0, 0);
        surf.layer.commit();
        if frames.len() > 1 {
            std::thread::sleep(Duration::from_millis(frame_delay_ms as u64));
        }
    }
    
    if let Some(surf) = state.renderer.get_mut(output_name) {
        surf.current_image = Some(crate::crawlbg::image::apply_wallpaper_mode(to_img, w, h, mode));
    }
}

struct WaylandState {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor_state: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    pool: SlotPool,
    renderer: Renderer,
    rx: Receiver<BackendCmd>,
    qh: QueueHandle<Self>,
    exit: bool,
}

impl CompositorHandler for WaylandState {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &wl_surface::WlSurface, scale: i32) {
        for name in self.renderer.surface_names() {
            if let Some(surf) = self.renderer.get(name.as_str()) {
                if surf.layer.wl_surface() == surface {
                    self.renderer.update_scale(&name, scale);
                    break;
                }
            }
        }
    }
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for WaylandState {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if let Some(info) = self.output_state.info(&output) {
            if let Some(name) = info.name.clone() {
                let (w, h) = logical_size(&info);
                info!("new output: {name} {w}x{h} scale={}", info.scale_factor);
                create_surface(self, &name, w, h, info.scale_factor);
            }
        }
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if let Some(info) = self.output_state.info(&output) {
            if let Some(name) = &info.name {
                let (w, h) = logical_size(&info);
                if let Some(surf) = self.renderer.get_mut(name) {
                    surf.width = w;
                    surf.height = h;
                    surf.scale = info.scale_factor;
                }
            }
        }
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if let Some(info) = self.output_state.info(&output) {
            if let Some(name) = &info.name {
                info!("output removed: {name}");
                self.renderer.remove(name);
            }
        }
    }
}

impl LayerShellHandler for WaylandState {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {}
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface, configure: LayerSurfaceConfigure, _serial: u32) {
        for name in self.renderer.surface_names() {
            if let Some(surf) = self.renderer.get(name.as_str()) {
                if surf.layer.wl_surface() == layer.wl_surface() {
                    let (new_w, new_h) = configure.new_size;
                    if new_w > 0 && new_h > 0 {
                        self.renderer.configure_surface(&name, new_w, new_h);
                    }
                    break;
                }
            }
        }
    }
}

impl ShmHandler for WaylandState {
    fn shm_state(&mut self) -> &mut Shm { &mut self.shm }
}

impl ProvidesRegistryState for WaylandState {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry_state }
    registry_handlers![OutputState];
}

delegate_compositor!(WaylandState);
delegate_output!(WaylandState);
delegate_layer!(WaylandState);
delegate_shm!(WaylandState);
delegate_registry!(WaylandState);
