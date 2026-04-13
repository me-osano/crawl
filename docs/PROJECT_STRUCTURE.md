# Project Structure

```
crawl/
├── Cargo.toml                   # workspace manifest
├── config/
│   └── crawl.toml               # annotated example configuration
├── systemd/
│   └── crawl.service            # systemd user service unit
├── pkg/
│   └── PKGBUILD                 # Arch Linux package
└── crates/
    ├── crawl-ipc/               # shared types, events, error envelope
    │   └── src/
    │       ├── lib.rs
    │       ├── error.rs         # CrawlError, ErrorEnvelope
    │       ├── events.rs        # CrawlEvent enum (all domain events)
    │       └── types.rs         # BluetoothDevice, BatteryStatus, MediaPlayer, etc.
    │
    ├── crawl-daemon/            # main binary — axum server over Unix socket
    │   └── src/
    │       ├── main.rs          # startup, spawn_domains()
    │       ├── config.rs        # figment config loading
    │       ├── state.rs         # AppState (Arc<Config> + broadcast tx)
    │       ├── router.rs        # all axum routes
    │       └── sse.rs           # GET /events SSE handler
    │
    ├── crawl-cli/               # crawl binary — thin HTTP client + clap
    │   └── src/
    │       ├── main.rs          # Cli + Commands dispatch
    │       ├── client.rs        # CrawlClient (hyper over Unix socket)
    │       ├── output.rs        # terminal formatting helpers
    │       └── cmd/
    │           ├── brightness.rs
    │           ├── bluetooth.rs
    │           ├── audio.rs
    │           ├── media.rs
    │           ├── network.rs
    │           ├── notify.rs
    │           ├── clip.rs
    │           ├── proc_.rs
    │           ├── power.rs
    │           ├── disk.rs
    │           ├── sysmon.rs
    │           └── daemon.rs
    │
    ├── crawl-bluetooth/                # bluer + BlueZ D-Bus
    ├── crawl-network/               # zbus + NetworkManager
    ├── crawl-notify/            # zbus — implements org.freedesktop.Notifications
    ├── crawl-clipboard/         # wl-clipboard-rs — Wayland clipboard
    ├── crawl-sysmon/            # sysinfo — CPU, memory, disk
    ├── crawl-brightness/        # sysfs /sys/class/backlight
    ├── crawl-proc/              # sysinfo — process list/kill
    ├── crawl-media/             # zbus — MPRIS2 aggregator
    ├── crawl-power/             # zbus — UPower battery
    ├── crawl-disk/              # zbus — UDisks2 block devices
    └── crawl-audio/             # libpulse-binding — PipeWire/PA sinks
```
