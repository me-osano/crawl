//! Software transition compositor for wallpaper changes.
//!
//! Produces a sequence of blended frames between `from` and `to` images.
//! The backend feeds these frames to the Wayland layer surface at the
//! configured FPS. No GPU required — pure CPU blending with SIMD-friendly
//! layout (row-major RGBA u8).
//!
//! SIMD acceleration uses AVX2 (256-bit vectors) for parallel pixel blending.
//! Falls back to scalar blending on non-SIMD architectures.
//!
//! Transition types taken from wpaperd / gl-transitions:
//!   Fade, Wipe (directional), Wave, Center-expand, Outer-contract, Random.

use image::{ImageBuffer, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

/// SIMD support - import wide crate types when building with AVX2.
#[cfg(target_feature = "avx2")]
use wide::f32x8;

/// Transition types for wallpaper changes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    #[default]
    Fade,
    Wipe,
    Wave,
    Center,
    Outer,
    Random,
    None,
}

/// Parameters controlling the animation.
#[derive(Debug, Clone)]
pub struct TransitionConfig {
    pub kind: TransitionKind,
    /// Frames per second.
    pub fps: u32,
    /// Total duration in milliseconds.
    pub duration_ms: u32,
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            kind: TransitionKind::Fade,
            fps: 30,
            duration_ms: 400,
        }
    }
}

impl TransitionConfig {
    /// Create a new TransitionConfig with custom FPS.
    pub fn with_fps(kind: TransitionKind, duration_ms: u32, fps: u32) -> Self {
        Self { kind, fps, duration_ms }
    }
}

/// One frame from a transition sequence — RGBA pixels, same dimensions as inputs.
pub type Frame = RgbaImage;

/// Returns every frame of the transition as an owned `Vec<Frame>`.
///
/// Both images should be pre-processed (wallpaper mode applied, resized)
/// before calling this function. The `width` and `height` should match
/// the dimensions of `from` and `to`.
///
/// If `half_res` is true, transitions render at half resolution for performance
/// (default: false for best quality).
pub fn generate_frames(
    from: &RgbaImage,
    to: &RgbaImage,
    width: u32,
    height: u32,
    cfg: &TransitionConfig,
    half_res: bool,
) -> Vec<Frame> {
    let total_frames = ((cfg.fps as f64 * cfg.duration_ms as f64) / 1000.0).round() as u32;
    let total_frames = total_frames.max(1);

    // Determine rendering resolution
    let (render_w, render_h) = if half_res {
        ((width / 2).max(1), (height / 2).max(1))
    } else {
        (width, height)
    };

    let actual_kind = match cfg.kind {
        TransitionKind::Random => {
            use rand::Rng;
            let n: u8 = rand::thread_rng().gen_range(0..5);
            match n {
                0 => TransitionKind::Fade,
                1 => TransitionKind::Wipe,
                2 => TransitionKind::Wave,
                3 => TransitionKind::Center,
                _ => TransitionKind::Outer,
            }
        }
        ref k => k.clone(),
    };

    (0..total_frames)
        .map(|i| {
            let t = i as f32 / (total_frames - 1).max(1) as f32; // 0.0 → 1.0
            blend_frame(from, to, render_w, render_h, t, &actual_kind)
        })
        .collect()
}

/// Blend a single frame at progress `t` (0.0 = fully `from`, 1.0 = fully `to`).
fn blend_frame(
    from: &RgbaImage,
    to: &RgbaImage,
    width: u32,
    height: u32,
    t: f32,
    kind: &TransitionKind,
) -> Frame {
    // Use SIMD path when AVX2 is available, otherwise fall back to scalar
    #[cfg(target_feature = "avx2")]
    {
        blend_frame_simd(from, to, width, height, t, kind)
    }
    #[cfg(not(target_feature = "avx2"))]
    {
        blend_frame_scalar(from, to, width, height, t, kind)
    }
}

/// Scalar fallback for non-SIMD architectures.
fn blend_frame_scalar(
    from: &RgbaImage,
    to: &RgbaImage,
    width: u32,
    height: u32,
    t: f32,
    kind: &TransitionKind,
) -> Frame {
    ImageBuffer::from_fn(width, height, |x, y| {
        let px_from = from.get_pixel(x, y);
        let px_to = to.get_pixel(x, y);
        let use_to = pixel_is_to(x, y, width, height, t, kind);
        if use_to {
            lerp_pixel(px_from, px_to, t.min(1.0))
        } else {
            *px_from
        }
    })
}

/// SIMD-accelerated blending using AVX2 (256-bit vectors = 8 pixels at once).
///
/// # Safety
/// Assumes pixel data is contiguous RGBA u8 with at least 4 bytes per pixel.
/// Processes pixels in chunks of 8 (AVX2 256-bit width).
#[cfg(target_feature = "avx2")]
fn blend_frame_simd(
    from: &RgbaImage,
    to: &RgbaImage,
    width: u32,
    height: u32,
    t: f32,
    kind: &TransitionKind,
) -> Frame {
    let mut result = ImageBuffer::new(width, height);
    let total_pixels = (width * height) as usize;
    let t_vec = f32x8::splat(t.min(1.0));
    const SIMD_CHUNK: usize = 8; // AVX2 processes 8 f32s at once

    // Process 8 pixels at a time
    for chunk_start in (0..total_pixels).step_by(SIMD_CHUNK) {
        let chunk_end = (chunk_start + SIMD_CHUNK).min(total_pixels);
        let len = chunk_end - chunk_start;

        // Load 8 pixels from `from` and `to` (RGBA channels)
        let mut from_r = [0f32; 8];
        let mut from_g = [0f32; 8];
        let mut from_b = [0f32; 8];
        let mut from_a = [0f32; 8];
        let mut to_r = [0f32; 8];
        let mut to_g = [0f32; 8];
        let mut to_b = [0f32; 8];
        let mut to_a = [0f32; 8];

        for i in 0..len {
            let pixel_idx = (chunk_start + i) * 4;
            from_r[i] = from.as_raw()[pixel_idx] as f32;
            from_g[i] = from.as_raw()[pixel_idx + 1] as f32;
            from_b[i] = from.as_raw()[pixel_idx + 2] as f32;
            from_a[i] = from.as_raw()[pixel_idx + 3] as f32;
            to_r[i] = to.as_raw()[pixel_idx] as f32;
            to_g[i] = to.as_raw()[pixel_idx + 1] as f32;
            to_b[i] = to.as_raw()[pixel_idx + 2] as f32;
            to_a[i] = to.as_raw()[pixel_idx + 3] as f32;
        }

        // SIMD lerp: result = from + (to - from) * t
        let from_r_vec = f32x8::from(from_r);
        let from_g_vec = f32x8::from(from_g);
        let from_b_vec = f32x8::from(from_b);
        let from_a_vec = f32x8::from(from_a);
        let to_r_vec = f32x8::from(to_r);
        let to_g_vec = f32x8::from(to_g);
        let to_b_vec = f32x8::from(to_b);
        let to_a_vec = f32x8::from(to_a);

        let result_r = from_r_vec + (to_r_vec - from_r_vec) * t_vec;
        let result_g = from_g_vec + (to_g_vec - from_g_vec) * t_vec;
        let result_b = from_b_vec + (to_b_vec - from_b_vec) * t_vec;
        let result_a = from_a_vec + (to_a_vec - from_a_vec) * t_vec;

        // Convert back to u8 and store
        let result_r_arr: [f32; 8] = result_r.into();
        let result_g_arr: [f32; 8] = result_g.into();
        let result_b_arr: [f32; 8] = result_b.into();
        let result_a_arr: [f32; 8] = result_a.into();

        for i in 0..len {
            let pixel_idx = (chunk_start + i) * 4;
            let x = (chunk_start + i) as u32 % width;
            let y = (chunk_start + i) as u32 / width;

            // Check if this pixel should use `to` based on transition kind
            let use_to = pixel_is_to(x, y, width, height, t, kind);

            if use_to {
                result.put_pixel(
                    x, y,
                    Rgba([
                        result_r_arr[i].round() as u8,
                        result_g_arr[i].round() as u8,
                        result_b_arr[i].round() as u8,
                        result_a_arr[i].round() as u8,
                    ])
                );
            } else {
                result.put_pixel(x, y, *from.get_pixel(x, y));
            }
        }
    }
    result
}

/// Returns true when this pixel should be primarily sourced from `to`.
/// The threshold creates the transition shape.
#[inline]
fn pixel_is_to(x: u32, y: u32, w: u32, h: u32, t: f32, kind: &TransitionKind) -> bool {
    match kind {
        // Pure alpha blend: every pixel blends simultaneously
        TransitionKind::Fade | TransitionKind::None => true,
        // Left-to-right wipe
        TransitionKind::Wipe => (x as f32 / w as f32) < t,
        // Sinusoidal wave wipe (left-to-right with vertical ripple)
        TransitionKind::Wave => {
            let wave_offset = (y as f32 / h as f32 * std::f32::consts::TAU * 2.0).sin() * 0.05;
            (x as f32 / w as f32) < (t + wave_offset)
        }
        // Expand from center outward
        TransitionKind::Center => {
            let cx = (x as f32 / w as f32) - 0.5;
            let cy = (y as f32 / h as f32) - 0.5;
            let dist = (cx * cx + cy * cy).sqrt() / (0.5f32.sqrt());
            dist < t
        }
        // Contract from edges inward (reverse of center)
        TransitionKind::Outer => {
            let cx = (x as f32 / w as f32) - 0.5;
            let cy = (y as f32 / h as f32) - 0.5;
            let dist = (cx * cx + cy * cy).sqrt() / (0.5f32.sqrt());
            dist > (1.0 - t)
        }
        // Already resolved above
        TransitionKind::Random => true,
    }
}

/// Linear interpolation between two RGBA pixels.
#[inline]
fn lerp_pixel(a: &Rgba<u8>, b: &Rgba<u8>, t: f32) -> Rgba<u8> {
    Rgba([
        lerp_u8(a[0], b[0], t),
        lerp_u8(a[1], b[1], t),
        lerp_u8(a[2], b[2], t),
        lerp_u8(a[3], b[3], t),
    ])
}

#[inline]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}
