//! Renderer for wallpaper surfaces.
//!
//! Manages per-output layer surfaces, handles high-quality scaling
//! with Lanczos3, and uploads pixel data to Wayland shared memory buffers.
//! Handles fractional scaling by tracking output scale factors.

use super::models::WallpaperMode;
use crate::crawlbg::image::apply_wallpaper_mode;
use image::{DynamicImage, RgbaImage};
use smithay_client_toolkit::{
    shell::{
        wlr_layer::{LayerSurface},
    },
};
use std::collections::HashMap;
// wayland_client::protocol::wl_shm is used via smithay_client_toolkit
use tracing::{debug, warn};
// OutputInfoExt is defined in wayland.rs

/// Per-output rendering state.
pub struct OutputSurface {
    pub layer: LayerSurface,
    pub width: u32,
    pub height: u32,
    pub scale: i32,
    pub configured: bool,
    /// The last image shown (for transition `from` frame).
    pub current_image: Option<RgbaImage>,
}

impl OutputSurface {
    /// Create a new output surface.
    pub fn new(layer: LayerSurface, width: u32, height: u32, scale: i32) -> Self {
        Self {
            layer,
            width,
            height,
            scale,
            configured: false,
            current_image: None,
        }
    }

    /// Update dimensions from configure event.
    pub fn update_size(&mut self, width: u32, height: u32) {
        if self.width != width || self.height != height {
            debug!("surface size changed: {}x{} -> {}x{}", self.width, self.height, width, height);
            self.width = width;
            self.height = height;
        }
    }

    /// Update scale factor.
    pub fn update_scale(&mut self, scale: i32) {
        if self.scale != scale {
            debug!("surface scale changed: {} -> {}", self.scale, scale);
            self.scale = scale;
        }
    }

    /// Get effective dimensions accounting for scale.
    pub fn effective_size(&self) -> (u32, u32) {
        let w = (self.width as f32 * self.scale as f32).round() as u32;
        let h = (self.height as f32 * self.scale as f32).round() as u32;
        (w, h)
    }
}

/// Manages all output surfaces and rendering.
pub struct Renderer {
    surfaces: HashMap<String, OutputSurface>,
}

impl Renderer {
    /// Create a new Renderer.
    pub fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
        }
    }

    /// Get or create a surface for an output.
    pub fn get_or_create<F>(&mut self, name: &str, create_fn: F) -> Option<&mut OutputSurface>
    where
        F: FnOnce() -> Option<(LayerSurface, u32, u32, i32)>,
    {
        if !self.surfaces.contains_key(name) {
            if let Some((layer, width, height, scale)) = create_fn() {
                self.surfaces.insert(
                    name.to_string(),
                    OutputSurface::new(layer, width, height, scale),
                );
                debug!("created surface for output: {}", name);
            } else {
                warn!("failed to create surface for output: {}", name);
                return None;
            }
        }
        self.surfaces.get_mut(name)
    }

    /// Get a surface by name.
    pub fn get(&self, name: &str) -> Option<&OutputSurface> {
        self.surfaces.get(name)
    }

    /// Get a mutable surface by name.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut OutputSurface> {
        self.surfaces.get_mut(name)
    }

    /// Remove a surface by name.
    pub fn remove(&mut self, name: &str) -> bool {
        self.surfaces.remove(name).is_some()
    }

    /// Update a surface's size from configure event.
    pub fn configure_surface(&mut self, name: &str, width: u32, height: u32) -> bool {
        if let Some(surf) = self.surfaces.get_mut(name) {
            if width > 0 && height > 0 {
                surf.update_size(width, height);
            }
            surf.configured = true;
            true
        } else {
            false
        }
    }

    /// Update surface scale from output info.
    pub fn update_scale(&mut self, name: &str, scale: i32) -> bool {
        if let Some(surf) = self.surfaces.get_mut(name) {
            surf.update_scale(scale);
            true
        } else {
            false
        }
    }

    /// Remove all surfaces.
    pub fn clear(&mut self) {
        self.surfaces.clear();
    }

    /// Get all surface names.
    pub fn surface_names(&self) -> Vec<String> {
        self.surfaces.keys().cloned().collect()
    }

    /// Prepare wallpaper image for an output.
    ///
    /// Applies wallpaper mode and scales to match output resolution exactly.
    /// Uses Lanczos3 for high-quality scaling.
    pub fn prepare_image(
        &self,
        output_name: &str,
        image: &DynamicImage,
        mode: WallpaperMode,
    ) -> Option<RgbaImage> {
        let surf = self.surfaces.get(output_name)?;
        
        // Use effective size (accounting for scale) for high-DPI outputs
        let (target_w, target_h) = surf.effective_size();
        
        debug!(
            "preparing image for {}: {}x{} (effective: {}x{}, scale: {})",
            output_name, surf.width, surf.height, target_w, target_h, surf.scale
        );

        // Apply wallpaper mode with Lanczos3 scaling
        let prepared = apply_wallpaper_mode(image, target_w, target_h, mode);
        Some(prepared)
    }

    /// Blit a single frame to the output's layer surface.
    ///
    /// Converts RGBA image to XRGB8888 format for Wayland shared memory.
    pub fn blit_frame(
        canvas: &mut [u8],
        image: &RgbaImage,
        width: u32,
        height: u32,
    ) {
        let pixels = image.as_raw();
        let chunk_count = (width * height) as usize;

        for i in 0..chunk_count {
            let base = i * 4;
            let canvas_offset = i * 4;
            
            if base + 2 >= pixels.len() || canvas_offset + 3 >= canvas.len() {
                break;
            }

            // XRGB8888: B G R X (little-endian)
            canvas[canvas_offset] = pixels[base + 2];     // B
            canvas[canvas_offset + 1] = pixels[base + 1]; // G
            canvas[canvas_offset + 2] = pixels[base];     // R
            canvas[canvas_offset + 3] = 0;                // X (ignored)
        }
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}
