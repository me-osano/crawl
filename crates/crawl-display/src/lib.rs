//! crawl-display: Display control (brightness, wallpaper)
//
pub mod brightness;
pub mod config;
pub mod crawlbg;
pub mod wallpaper;

pub use brightness::{Backlight, BrightnessError};
pub use config::DisplayConfig;
pub use crawlbg::{CrawlbgBackend, WallpaperMode, SetWallpaperRequest};
pub use wallpaper::WallpaperService;

use tokio::sync::broadcast;

pub use brightness::run as run_brightness;

pub async fn run(cfg: DisplayConfig, tx: broadcast::Sender<crawl_ipc::CrawlEvent>) -> anyhow::Result<()> {
    brightness::run(cfg, tx).await
}