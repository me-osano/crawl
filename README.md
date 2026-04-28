# Crawl

Modular, event-driven system service stack for Linux. Provides a background daemon managing system components and a CLI for interaction via Unix socket IPC.

```
## Description

Crawl centralizes desktop/system service management including audio, display, network, bluetooth, system monitoring, and process control. It runs as `crawl-daemon` exposing a JSON-RPC 2.0 API over Unix sockets, with `crawl` CLI for end-user interaction.

## Features

- **System Information**: OS, hardware, compositor, display, and disk info with ASCII logo
- **System Monitoring**: CPU, memory, disk, network, and GPU tracking with live watch mode
- **Process Management**: List, sort, find, kill, and watch processes
- **Audio Control**: PulseAudio/PipeWire volume, mute, and device management
- **Display Management**: Brightness control (sysfs backlight) and Wayland wallpaper setting with animated transitions
- **Network Management**: NetworkManager-based WiFi, ethernet, and hotspot control
- **Bluetooth**: Bluetooth device management
- **IPC**: Unix socket-based JSON-RPC 2.0 with pub/sub event subscription

## Architecture

Cargo workspace with modular crates:
- `crawl-daemon`: Core daemon orchestrating services and IPC
- `crawl-cli`: Command-line client for the daemon
- `crawl-ipc`: Shared IPC types, Unix socket transport, JSON-RPC 2.0 protocol
- `crawl-sysinfo`: System information aggregation
- `crawl-sysmon`: System resource monitoring
- `crawl-proc`: Process management
- `crawl-audio`: Audio device control
- `crawl-display`: Brightness and wallpaper management
- `crawl-network`: NetworkManager integration
- `crawl-bluetooth`: Bluetooth service
- `crawl-scheduler`: Central task scheduler with jitter-based timing

## Prerequisites

- Linux (Wayland compositor with wlr-layer-shell support for wallpaper features)
- Rust 1.94.0+ (for building from source)
- PulseAudio/PipeWire (audio)
- NetworkManager (network features)
- Membership in `video` group (brightness control)

## Installation

### Quick Install
```bash
curl -fsSL https://github.com/me-osano/crawl/raw/main/install.sh | bash
```

### Manual Build
```bash
git clone https://github.com/me-osano/crawl.git
cd crawl
cargo build --release --workspace --bins
sudo install -Dm755 target/release/crawl-daemon /usr/local/bin/crawl-daemon
sudo install -Dm755 target/release/crawl /usr/local/bin/crawl
```

The installer sets up:
- Systemd service (`crawl.service`)
- Default config at `~/.config/crawl/config.toml` and `/etc/crawl/config.toml`
- Udev rules for backlight access
- Adds user to `video` group

## Configuration

Config locations (priority order):
1. `~/.config/crawl/config.toml`
2. `/etc/crawl/config.toml`

Environment variables (prefix `CRAWL_<CRATE>__<KEY>`):
```bash
CRAWL_NETWORK__AUTO_ENABLE=true
CRAWL_SYSMON__POLL_INTERVAL_MS=1000
```

See `docs/` for per-component config details.

## Usage

Global flag for JSON output:
```bash
crawl -j  # Raw JSON instead of formatted text
```

Common commands:
```bash
crawl sysinfo                          # System information
crawl sysmon --cpu --watch             # Live CPU monitoring
crawl proc --sort mem --top 10         # Top 10 processes by memory
crawl audio -v 70                      # Set audio volume to 70%
crawl display wallpaper-set ~/wall.png # Set wallpaper
crawl network wifi-connect --ssid "MyNet" # Connect to WiFi
crawl daemon --restart                 # Restart daemon
```

Full CLI reference: `docs/CLI.md`

## Project Structure

```
crawl/
├── crates/           # Workspace member crates
├── docs/             # Component documentation
├── assets/           # Default config and wallpaper
├── Cargo.toml        # Workspace manifest
├── install.sh        # Installation script
└── LICENSE           # MIT License
```

## License

MIT License – see [LICENSE](LICENSE)

## Repository

https://github.com/me-osano/crawl
