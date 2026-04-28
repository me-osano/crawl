# Display Subsystem

The crawl-display crate handles display-related functionality including brightness control and wallpaper management.

## Configuration

```toml
[display]
wallpaper = ""                           # empty for default wallpaper, or specify path
wallpaper_transition = "fade"            # fade, wipe, wave, center, outer, random, none
wallpaper_transition_duration_ms = 500   # transition duration in milliseconds
brightness_min = 1.0
brightness_max = 100.0
brightness_device = ""                   # empty for auto-detect
```

## Architecture

```
crawl-display/
├── src/
│   ├── lib.rs           # Public API re-exports
│   ├── brightness.rs    # Brightness control via sysfs
│   ├── config.rs       # Unified DisplayConfig
│   ├── wallpaper.rs    # Wallpaper service (state management, IPC handling)
│   └── crawlbg/
│       ├── mod.rs       # CrawlbgBackend - native Wayland backend entry
│       ├── models.rs    # Domain types (SetWallpaperRequest, WallpaperMode, WallpaperState, CrawlbgError)
│       ├── cache.rs     # LRU image cache with eviction
│       ├── image.rs     # Image loading, Lanczos3 resizing via fast_image_resize
│       ├── outputs.rs   # Monitor/output management with scale tracking
│       ├── renderer.rs  # Per-output surface management, blit operations
│       ├── transition.rs # Transition animations (SIMD-accelerated blending)
│       └── wayland.rs   # Wayland backend using wlr-layer-shell + smithay-client-toolkit
├── assets/
│   └── wallpaper.png   # Default wallpaper
└── Cargo.toml
```

## Brightness Control

Control display brightness via sysfs backlight interface.

### API

```rust
use crawl_display::{Backlight, DisplayConfig};

let config = DisplayConfig {
    brightness_min: 5.0,
    brightness_max: 100.0,
    brightness_device: "intel_backlight".into(),
    ..Default::default()
};

let backlight = Backlight::open(&config)?;

// Get current status
let status = backlight.status()?;

// Set brightness by percent
let status = backlight.set_percent(50.0)?;

// Adjust relative to current
let status = backlight.adjust_percent(10.0)?;  // +10%
```

### Events

Emits `BrightnessEvent::Changed` with the new status when brightness changes.

## Wallpaper Management

Manage wallpapers using the native Wayland backend (crawlbg) with animated transitions.

### Backends

#### crawlbg (Native Wayland Backend)

Native Wayland wallpaper backend using wlr-layer-shell protocol with smithay-client-toolkit. Supports animated transitions and multiple wallpaper modes.

**Features:**
- Animated transitions (fade, wipe, wave, center, outer, random)
- Multiple wallpaper modes (fill, fit, stretch, center, tile)
- Per-monitor wallpaper support
- Lanczos3 high-quality image scaling via `fast_image_resize`
- LRU image cache with eviction (bounded memory usage)
- Output scale tracking for high-DPI support
- Monitor hotplug handling with proper cleanup
- State persistence (restores wallpaper on restart)
- SIMD-accelerated pixel blending (AVX2) with scalar fallback

```rust
use crawl_display::{CrawlbgBackend, WallpaperService, SetWallpaperRequest, WallpaperMode};

// Create service with config (wires up transition settings automatically)
let service = WallpaperService::new(event_tx, config);

// Set wallpaper with transition
let request = SetWallpaperRequest {
    path: "/path/to/wallpaper.png".into(),
    monitor: None,  // None = all monitors, or Some("DP-1".into()) for specific
    mode: WallpaperMode::Fill,
    wallpaper_transition: "fade".into(),
    wallpaper_transition_duration_ms: 500,
};
service.set_wallpaper(request).await?;
```

### Wallpaper Modes

```rust
pub enum WallpaperMode {
    Fill,    // Fill entire screen (may crop edges to preserve aspect ratio)
    Fit,     // Fit within bounds (preserve aspect ratio, pad with black bars)
    Stretch, // Stretch to fill (ignores aspect ratio)
    Center,  // Center at original size (pad with black)
    Tile,    // Tile the image to fill screen
}
```

### Transition Types

```rust
pub enum TransitionKind {
    Fade,    // Simple crossfade (default)
    Wipe,    // Left-to-right wipe
    Wave,    // Sinusoidal wave wipe with vertical ripple
    Center,  // Expand from center outward
    Outer,   // Contract from edges inward
    Random,  // Randomly select one of the above
    None,    // No animation
}
```

### State Persistence

Wallpaper state is automatically saved to disk and restored on daemon restart:

**State file location:** `~/.local/state/crawl/wallpaper_state.json`

The state includes:
- Global wallpaper path
- Per-monitor wallpaper assignments
- Per-monitor mode flag

### Image Cache

The backend uses an LRU (Least Recently Used) cache with eviction to bound memory usage. Default capacity is 20 images. Use `preload()` to warm the cache:

```rust
service.preload("/path/to/wallpaper.png").await?;
```

### IPC Commands

```json
{"action": "get_state"}
{"action": "set_wallpaper", "params": {"path": "/path/to/image.png", "mode": "fit", "monitor": "DP-1"}}
{"action": "get_wallpaper", "params": {"monitor": "DP-1"}}
{"action": "preload", "params": {"path": "/path/to/image.png"}}
{"action": "brightness_get"}
{"action": "brightness_set", "params": {"value": 50}}
{"action": "brightness_inc", "params": {"value": 5}}
{"action": "brightness_dec", "params": {"value": 5}}
```

### Events

```rust
pub enum WallpaperEvent {
    Changed { screen: String, path: String },      // Wallpaper changed
    BackendChanged { backend: String },            // Backend switched
    BackendNotAvailable { backend: String },       // Backend unavailable
    Error { message: String },                    // Error occurred
}
```

## CLI Usage

```bash
# Wallpaper commands
crawl display wallpaper
crawl display wallpaper-set /path/to/image.png
crawl display wallpaper-set /path/to/image.png --monitor DP-1
crawl display wallpaper-get
crawl display wallpaper-get --monitor DP-1

# Brightness commands
crawl display brightness
crawl display brightness-set 75
crawl display brightness-inc 10
crawl display brightness-dec 5
```

## Default Wallpaper

Place the default wallpaper at:

```
crates/crawl-display/assets/wallpaper.png
```

The path `assets/wallpaper.png` is resolved relative to the crate's manifest directory at runtime. You can also configure a custom default in `crawl.toml`:

```toml
[display]
wallpaper = "/path/to/your/wallpaper.png"
```

The wallpaper is set automatically on daemon startup. If a previously saved wallpaper exists, it will be restored instead of using the default.

## Requirements

### Brightness

- Write access to `/sys/class/backlight/*/brightness`
- On Arch: add user to `video` group with udev rule

### Wallpaper
- **Wayland compositor** with wlr-layer-shell support (sway, river, Hyprland, etc.)
- `WAYLAND_DISPLAY` environment variable set
- **Dependencies:** `fast_image_resize` (Lanczos3), `lru` (cache eviction), `smithay-client-toolkit` 0.19
- Image files readable by daemon process
- Supported image formats: PNG, JPEG, GIF, BMP, ICO, TIFF, WebP, AVIF, PNM