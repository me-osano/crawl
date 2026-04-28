//! Image loading, decoding, and resizing utilities.
//!
//! Provides high-quality image operations using Lanczos3 resampling
//! for optimal wallpaper quality.

pub use image::DynamicImage;
use image::{GenericImageView, Rgba, RgbaImage};
use std::path::Path;
use anyhow::Context;
use tracing::debug;

/// Load an image from disk.
pub fn load_image(path: &Path) -> anyhow::Result<DynamicImage> {
    let img = image::open(path)
        .with_context(|| format!("open image {}", path.display()))?;
    debug!("loaded image: {} ({}x{})", path.display(), img.width(), img.height());
    Ok(img)
}

/// Resize image to fit target dimensions using Lanczos3 filter.
///
/// This provides higher quality than bilinear or nearest-neighbor,
/// preserving sharpness and reducing artifacts on wallpaper scaling.
pub fn resize_lanczos3(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
) -> RgbaImage {
    let (src_width, src_height) = img.dimensions();
    
    if src_width == target_width && src_height == target_height {
        return img.to_rgba8();
    }

    debug!(
        "resizing {}x{} -> {}x{} (Lanczos3)",
        src_width, src_height, target_width, target_height
    );

    img.resize_exact(target_width, target_height, image::imageops::FilterType::Lanczos3)
        .to_rgba8()
}

/// Apply wallpaper mode to fit image into target dimensions.
///
/// Returns a new `RgbaImage` sized to `(width, height)` with the source
/// image arranged according to `mode`. Uses Lanczos3 for high-quality scaling.
pub fn apply_wallpaper_mode(
    img: &DynamicImage,
    width: u32,
    height: u32,
    mode: super::models::WallpaperMode,
) -> RgbaImage {
    match mode {
        super::models::WallpaperMode::Fill => {
            // Fill entire area, crop edges to preserve aspect ratio
            let (w, h) = img.dimensions();
            let src_aspect = w as f32 / h as f32;
            let dst_aspect = width as f32 / height as f32;
            
            if (src_aspect - dst_aspect).abs() < 0.01 {
                // Aspects match, just resize
                resize_lanczos3(img, width, height)
            } else {
                // Need to resize then crop to fit
                let (new_w, new_h) = if src_aspect > dst_aspect {
                    // Image is wider, crop sides
                    let new_h = (width as f32 / src_aspect) as u32;
                    (width, new_h)
                } else {
                    // Image is taller, crop top/bottom
                    let new_w = (height as f32 * src_aspect) as u32;
                    (new_w, height)
                };
                let resized = resize_lanczos3(img, new_w, new_h);
                // Crop to exact dimensions
                let x = ((new_w - width) / 2) as i64;
                let y = ((new_h - height) / 2) as i64;
                image::imageops::crop_imm(&resized, x.max(0) as u32, y.max(0) as u32, width, height).to_image()
            }
        }
        super::models::WallpaperMode::Fit => {
            // Fit within bounds, preserve aspect ratio, pad with black
            let (w, h) = img.dimensions();
            let scale = (width as f32 / w as f32).min(height as f32 / h as f32);
            let new_w = (w as f32 * scale) as u32;
            let new_h = (h as f32 * scale) as u32;
            let resized = resize_lanczos3(img, new_w, new_h);
            let mut canvas = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255]));
            let x = (width - new_w) / 2;
            let y = (height - new_h) / 2;
            image::imageops::replace(&mut canvas, &resized, x as i64, y as i64);
            canvas
        }
        super::models::WallpaperMode::Stretch => {
            // Stretch to fill, ignore aspect ratio
            resize_lanczos3(img, width, height)
        }
        super::models::WallpaperMode::Center => {
            // Center at original size, pad with black
            let (w, h) = img.dimensions();
            let mut canvas = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255]));
            let x = ((width as i32 - w as i32) / 2).max(0) as i64;
            let y = ((height as i32 - h as i32) / 2).max(0) as i64;
            let (src_w, src_h) = (w.min(width), h.min(height));
            let cropped = img.crop_imm(0, 0, src_w, src_h);
            image::imageops::replace(&mut canvas, &cropped.to_rgba8(), x, y);
            canvas
        }
        super::models::WallpaperMode::Tile => {
            // Tile the image to fill the entire area
            let (w, h) = img.dimensions();
            if w == 0 || h == 0 {
                return RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255]));
            }
            let rgba = img.to_rgba8();
            let mut canvas = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255]));
            for y in (0..height).step_by(h as usize) {
                for x in (0..width).step_by(w as usize) {
                    let tx = (x + w - 1).min(width - 1);
                    let ty = (y + h - 1).min(height - 1);
                    let cw = tx - x + 1;
                    let ch = ty - y + 1;
                    for py in 0..ch {
                        for px in 0..cw {
                            let src_x = px % w;
                            let src_y = py % h;
                            let pixel = rgba.get_pixel(src_x, src_y);
                            canvas.put_pixel(x + px, y + py, *pixel);
                        }
                    }
                }
            }
            canvas
        }
    }
}
