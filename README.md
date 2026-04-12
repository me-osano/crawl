# Crawl
```
  ____ ____      ___        ___     
 / ___|  _ \    / \ \      / / |    
| |   | |_) |  / _ \ \ /\ / /| |    
| |___|  _ <  / ___ \ V  V / | |___ 
 \____|_| \_\/_/   \_\_/\_/  |_____|
 
 ```

A fast, modular system services daemon and CLI for Linux Wayland desktops —
built in Rust for use with [niri](https://github.com/YaLTeR/niri),
[Quickshell](https://quickshell.outfoxxed.me/), and any compositor that
doesn't hand you a desktop environment.

```
crawl brightness --set=80
crawl sysmon --cpu
crawl bluetooth --connect=AA:BB:CC:DD:EE:FF
crawl media --next
crawl power --battery
crawl audio --volume=60
```

Designed as the backend for [CrawlDesktopShell](https://github.com/me-osano/crawldesktopshell) —
a custom Wayland Desktop shell — but fully usable standalone.

---

## Table of Contents

- [What crawl is](#what-crawl-is)
- [Architecture](#architecture)
- [Domains](#domains)
- [Installation](#installation)
  - [From source](#from-source)
  - [Arch Linux (PKGBUILD)](#arch-linux-pkgbuild)
- [Setup](#setup)
  - [Brightness permissions](#brightness-permissions)
  - [Bluetooth](#bluetooth)
  - [Notifications](#notifications)
  - [systemd user service](#systemd-user-service)
- [Configuration](#configuration)
- [CLI reference](#cli-reference)
- [IPC / HTTP API](#ipc--http-api)
  - [Request/response](#requestresponse)
  - [SSE event stream](#sse-event-stream)
- [Quickshell integration](#quickshell-integration)
- [Project structure](#project-structure)
- [Development](#development)
  - [Building](#building)
  - [Running locally](#running-locally)
  - [Adding a domain](#adding-a-domain)
  - [Debugging the socket](#debugging-the-socket)
- [Roadmap](#roadmap)

---

## What crawl is

`crawl` is two things:

**`crawl-daemon`** — a long-running Rust process that owns system-level
concerns and exposes them over a Unix socket at
`$XDG_RUNTIME_DIR/crawl.sock`. Each domain (Bluetooth, audio, brightness,
etc.) runs as an independent `tokio` task. Events from all domains are
broadcast over a single SSE stream that any number of clients can subscribe
to simultaneously.

**`crawl`** (the CLI) — a thin client that sends HTTP-over-socket requests
to the daemon and formats the response for the terminal. It's just a
pretty-printer on top of the JSON API; anything the CLI can do, you can do
with `curl` or `socat`.

```
crawl CLI  ──────────────────────────────┐
                                         │  HTTP over Unix socket
crawl-notify ──┐                         │  $XDG_RUNTIME_DIR/crawl.sock
crawl-bluetooth ──────┤                         │
crawl-network ─────┤   broadcast channel   ──┤──► GET /events  (SSE stream)
crawl-audio ───┤   (CrawlEvent)          │
crawl-sysmon ──┤                         │
crawl-power ───┤                         │
...            │                         │
               └── crawl-daemon (axum) ──┘
                          │
                   Quickshell QML
                   DataStream / NetworkRequest
```

---

## Architecture

### IPC transport

All communication uses HTTP/1.1 over a Unix domain socket. The wire format
is JSON. This means:

- The CLI is just an HTTP client
- Quickshell's `NetworkRequest` can talk to it natively
- You can debug with `socat` and `curl --unix-socket`
- No custom protocol to maintain

### Event model

Every domain task holds a `tokio::sync::broadcast::Sender<CrawlEvent>`. When
something changes (battery level, Bluetooth device connects, notification arrives),
it sends to that channel. The SSE handler fans events out to all connected
`GET /events` clients as newline-delimited JSON.

### Domain isolation

Each domain lives in its own crate (`crawl-bluetooth`, `crawl-audio`, etc.) with no
dependency on other domains. The only shared surface is `crawl-ipc`, which
contains the serializable types and event enum. This means:

- Domains are independently testable
- A domain crashing doesn't bring down others (each runs in its own task)
- `crawl-ipc` can be used in future QML bridge crates or other tools

---

## Domains

| Domain | Crate | Backend | What it owns |
|---|---|---|---|
| Bluetooth | `crawl-bluetooth` | `bluer` / BlueZ D-Bus | Discovery, pair, connect/disconnect, Bluetooth battery |
| Network | `crawl-network` | `zbus` / NetworkManager | WiFi scan, connect, interface status |
| Notifications | `crawl-notify` | `zbus` / `org.freedesktop.Notifications` | Full notification daemon (replaces mako/dunst) |
| Clipboard | `crawl-clipboard` | `wl-clipboard-rs` | Clipboard + primary selection, history |
| System monitor | `crawl-sysmon` | `sysinfo` | CPU, memory, disk, load averages |
| Brightness | `crawl-brightness` | sysfs `/sys/class/backlight` | Backlight read/write |
| Processes | `crawl-proc` | `sysinfo` | Process list, search, kill, watch |
| Media | `crawl-media` | `zbus` / MPRIS2 | Multi-player aggregator, track info, control |
| Power | `crawl-power` | `zbus` / UPower | Battery percent, state, time estimates |
| Disk | `crawl-disk` | `zbus` / UDisks2 | Block device list, mount, unmount, eject |
| Audio | `crawl-audio` | `libpulse-binding` | Sink/source list, volume, mute |

---

## Installation

### Dependencies

**Runtime:**
```
bluez  networkmanager  udisks2  upower  pipewire  pipewire-pulse  libpulse
```

**Build:**
```
rust (stable)  cargo  pkg-config  clang  (for bluer/libpulse bindgen)
```

On Arch:
```bash
sudo pacman -S bluez bluez-libs networkmanager udisks2 upower \
               pipewire pipewire-pulse libpulse \
               rust pkg-config clang
```

### From source

```bash
git clone https://github.com/me-osano/crawl
cd crawl

cargo build --release --workspace --bins

# Install binaries
sudo install -Dm755 target/release/crawl-daemon /usr/local/bin/crawl-daemon
sudo install -Dm755 target/release/crawl        /usr/local/bin/crawl

# Install service and config
mkdir -p ~/.config/systemd/user
cp systemd/crawl.service ~/.config/systemd/user/

mkdir -p ~/.config/crawl
cp config/crawl.toml ~/.config/crawl/crawl.toml
```

### Arch Linux (PKGBUILD)

```bash
cd pkg
makepkg -si
```

The PKGBUILD installs:
- `/usr/bin/crawl-daemon` and `/usr/bin/crawl`
- `/usr/lib/systemd/user/crawl.service`
- `/usr/lib/udev/rules.d/90-crawl-backlight.rules`
- `/etc/crawl/crawl.toml`

---

## Setup

### Brightness permissions

crawl reads and writes `/sys/class/backlight/<device>/brightness` directly.
No subprocess, no sudo. You need write permission on that file.

**Option A — udev rule (recommended, installed by PKGBUILD automatically):**

```
# /etc/udev/rules.d/90-crawl-backlight.rules
ACTION=="add", SUBSYSTEM=="backlight", \
    RUN+="/usr/bin/chgrp video /sys/class/backlight/%k/brightness", \
    RUN+="/usr/bin/chmod g+w /sys/class/backlight/%k/brightness"
```

Then add your user to the `video` group and re-login:
```bash
sudo usermod -aG video $USER
```

**Verify:**
```bash
ls -la /sys/class/backlight/*/brightness
# should show group=video, mode=rw-rw-r--
```

**Option B — polkit rule:**

```
# /etc/polkit-1/rules.d/90-crawl-brightness.rules
polkit.addRule(function(action, subject) {
    if (action.id == "org.freedesktop.login1.set-brightness" &&
        subject.isInGroup("video")) {
        return polkit.Result.YES;
    }
});
```

### Bluetooth

Ensure `bluetoothd` is running:
```bash
sudo systemctl enable --now bluetooth
```

crawl talks to BlueZ over D-Bus via `bluer` — no extra configuration needed.

### Notifications

By default, `crawl-notify` registers `org.freedesktop.Notifications` on the
session bus, making crawl-daemon your notification daemon. **Remove or
disable mako/dunst** before starting crawl, or set `replace_daemon = false`
in your config if you want to keep them.

```bash
# If you were using mako:
systemctl --user disable --now mako

# Or set in crawl.toml:
# [notifications]
# replace_daemon = false
```

Quickshell then reads notifications from the SSE stream (`domain: "notify"`)
instead of spawning a separate notification daemon.

### systemd user service

```bash
# Enable and start
systemctl --user enable --now crawl

# Check status
systemctl --user status crawl

# Follow logs
journalctl --user -u crawl -f

# Restart after config change
systemctl --user restart crawl
```

The service starts automatically with your graphical session
(`WantedBy=graphical-session.target`).

Note: the provided unit expects `/usr/local/bin/crawl-daemon`. The Arch
package installs to `/usr/bin/crawl-daemon`, so override the unit or use a
drop-in to update `ExecStart` when using the packaged service.

---

## Configuration

Config file location: `$XDG_CONFIG_HOME/crawl/crawl.toml`
(usually `~/.config/crawl/crawl.toml`)

An annotated example is at `config/crawl.toml` in this repo and installed
to `/etc/crawl/crawl.toml` by the Arch package.

**Environment variable overrides** use double-underscore separators:
```bash
CRAWL__DAEMON__LOG_LEVEL=debug crawl-daemon
CRAWL__SYSMON__POLL_INTERVAL_MS=2000 crawl-daemon
CRAWL__BRIGHTNESS__DEVICE=amdgpu_bl0 crawl-daemon
```

**Key settings:**

```toml
[daemon]
log_level = "info"          # trace | debug | info | warn | error

[notifications]
replace_daemon = true       # set false to keep mako/dunst

[brightness]
device = ""                 # auto-detect; or set e.g. "intel_backlight"

[sysmon]
poll_interval_ms = 1000

[power]
low_battery_threshold  = 20.0
critical_threshold     = 5.0

[clipboard]
history_size     = 50
watch_primary    = false
```

---

## CLI reference

All commands accept `--json` / `-j` for raw JSON output (useful in scripts).
All commands accept `--socket <path>` to override the daemon socket.

### brightness

```bash
crawl brightness                    # get current
crawl brightness --set=80           # set to 80%
crawl brightness --inc=5            # increase by 5%
crawl brightness --dec=10           # decrease by 10%
```

### sysmon

```bash
crawl sysmon --cpu                  # CPU usage + load averages
crawl sysmon --mem                  # memory usage
crawl sysmon --disk                 # disk usage per mount
crawl sysmon --watch                # live CPU/memory updates (SSE)
crawl sysmon --cpu --json           # raw JSON
```

### bluetooth

```bash
crawl bluetooth                            # status + device list
crawl bluetooth --scan                     # start discovery
crawl bluetooth --connect=AA:BB:CC:DD:EE:FF
crawl bluetooth --disconnect=AA:BB:CC:DD:EE:FF
crawl bluetooth --power=on
crawl bluetooth --power=off
```

### network

```bash
crawl network                           # connectivity status
crawl network --wifi                    # list nearby WiFi networks
crawl network --connect=MySSID --password=hunter2
```

### audio

```bash
crawl audio                         # list sinks with volume
crawl audio --volume=70             # set default sink to 70%
crawl audio --mute                  # toggle mute
crawl audio --sources               # list microphones / sources
```

### media

```bash
crawl media                         # active player + track info
crawl media --play
crawl media --pause
crawl media --next
crawl media --prev
crawl media --volume=0.8            # 0.0–1.0
crawl media --list                  # all MPRIS players
crawl media --player=spotify --next # target specific player
```

### power

```bash
crawl power                         # battery percent, state, time estimates
crawl power --json
```

### notify

```bash
crawl notify --list                 # all active notifications
crawl notify --title="Build done" --body="cargo build succeeded"
crawl notify --title="Alert" --body="Disk full" --urgency=critical
crawl notify --dismiss=42           # dismiss notification by ID
```

### clip

```bash
crawl clip --get                    # current clipboard content
crawl clip --set="some text"        # write to clipboard
crawl clip --history                # clipboard history (JSON)
```

### proc

```bash
crawl proc                          # top 20 processes by CPU
crawl proc --sort=mem --top=10      # top 10 by memory
crawl proc --find=firefox           # find by name
crawl proc --kill=1234              # SIGTERM
crawl proc --kill=1234 --force      # SIGKILL
```

### disk

```bash
crawl disk                          # list block devices
crawl disk --mount=/dev/sdb1        # mount device
crawl disk --unmount=/dev/sdb1
crawl disk --eject=/dev/sdb         # eject drive
```

### daemon

```bash
crawl daemon                        # status + version
crawl daemon --restart
crawl daemon --stop
```

---

## IPC / HTTP API

The daemon exposes a standard HTTP/1.1 API over a Unix socket. You can talk
to it with any HTTP client that supports Unix sockets.

Note: sysmon and brightness endpoints are now wired to real data.

**With curl:**
```bash
curl --unix-socket $XDG_RUNTIME_DIR/crawl.sock http://localhost/health
curl --unix-socket $XDG_RUNTIME_DIR/crawl.sock http://localhost/sysmon/cpu
curl --unix-socket $XDG_RUNTIME_DIR/crawl.sock \
     -X POST -H 'Content-Type: application/json' \
     -d '{"value":80}' http://localhost/brightness/set
```

**With socat:**
```bash
echo -e 'GET /sysmon/cpu HTTP/1.0\r\n\r\n' | \
    socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/crawl.sock
```

### Request/response

All responses are JSON. Errors use the standard envelope:
```json
{
  "error": {
    "domain": "bluetooth",
    "code":   "not_powered",
    "message": "Bluetooth adapter is not powered"
  }
}
```

#### Health
```
GET  /health
→ { "status": "ok", "version": "0.1.1" }
```

#### Bluetooth
```
GET  /bluetooth/status          → BluetoothStatus
GET  /bluetooth/devices         → [BluetoothDevice]
POST /bluetooth/scan            → {}
POST /bluetooth/connect         ← { "address": "AA:BB:CC:DD:EE:FF" }
POST /bluetooth/disconnect      ← { "address": "AA:BB:CC:DD:EE:FF" }
POST /bluetooth/power           ← { "on": true }
```

#### Network
```
GET  /network/status         → NetStatus
GET  /network/wifi           → [WifiNetwork]
POST /network/connect        ← { "ssid": "MyWifi", "password": "..." }
```

#### Notifications
```
GET    /notify/list      → [Notification]
POST   /notify/send      ← { "title": "...", "body": "...", "urgency": "normal" }
DELETE /notify/:id       → {}
```

#### Clipboard
```
GET  /clipboard          → ClipEntry
POST /clipboard          ← { "content": "text" }
GET  /clipboard/history  → [ClipEntry]
```

#### Sysmon
```
GET  /sysmon/cpu         → CpuStatus
GET  /sysmon/mem         → MemStatus
GET  /sysmon/disk        → [DiskStatus]
```

#### Brightness
```
GET  /brightness         → BrightnessStatus
POST /brightness/set     ← { "value": 80 }
POST /brightness/inc     ← { "value": 5 }
POST /brightness/dec     ← { "value": 5 }
```

#### Processes
```
GET  /proc/list          → [ProcessInfo]   (?sort=cpu&top=20)
GET  /proc/find          → [ProcessInfo]   (?name=firefox)
POST /proc/:pid/kill     ← { "force": false }
```

#### Media
```
GET  /media/players      → [MediaPlayer]
GET  /media/active       → MediaPlayer
POST /media/play         ← { "player": null }
POST /media/pause        ← { "player": null }
POST /media/next         ← { "player": null }
POST /media/prev         ← { "player": null }
POST /media/volume       ← { "volume": 0.8, "player": null }
```

#### Power
```
GET  /power/battery      → BatteryStatus
```

#### Disk
```
GET  /disk/list          → [BlockDevice]
POST /disk/mount         ← { "device": "/org/freedesktop/UDisks2/block_devices/sdb1" }
POST /disk/unmount       ← { "device": "..." }
POST /disk/eject         ← { "device": "..." }
```

#### Audio
```
GET  /audio/sinks        → [AudioDevice]
GET  /audio/sources      → [AudioDevice]
POST /audio/volume       ← { "percent": 70 }
POST /audio/mute         ← {}
```

### SSE event stream

```
GET /events
Content-Type: text/event-stream
```

Each event is a JSON object with `domain` and `data` fields:

```
data: {"domain":"sysmon","data":{"event":"cpu_update","cpu":{"aggregate":34.2,...}}}

data: {"domain":"bluetooth","data":{"event":"device_connected","device":{...}}}

data: {"domain":"notify","data":{"event":"new","notification":{...}}}

data: {"domain":"power","data":{"event":"battery_update","status":{...}}}

: keep-alive
```

**All domains and their events:**

| Domain | Events |
|---|---|
| `bluetooth` | `device_discovered`, `device_connected`, `device_disconnected`, `device_removed`, `adapter_powered`, `scan_started`, `scan_stopped` |
| `network` | `connected`, `disconnected`, `ip_changed`, `wifi_enabled`, `wifi_disabled`, `connectivity_changed` |
| `notify` | `new`, `closed`, `action_invoked`, `replaced` |
| `clipboard` | `changed`, `primary_changed` |
| `sysmon` | `cpu_update`, `mem_update`, `cpu_spike`, `mem_pressure` |
| `brightness` | `changed` |
| `proc` | `spawned`, `exited` |
| `media` | `player_appeared`, `player_vanished`, `track_changed`, `playback_changed`, `volume_changed` |
| `power` | `battery_update`, `ac_connected`, `ac_disconnected`, `low_battery`, `critical` |
| `disk` | `device_mounted`, `device_unmounted`, `device_added`, `device_removed` |
| `audio` | `volume_changed`, `mute_toggled`, `default_sink_changed`, `default_source_changed`, `device_added`, `device_removed` |
| `daemon` | `started`, `stopping`, `domain_error` |

**Consume with curl:**
```bash
curl --no-buffer --unix-socket $XDG_RUNTIME_DIR/crawl.sock \
     http://localhost/events
```

**Filter a single domain:**
```bash
curl --no-buffer --unix-socket $XDG_RUNTIME_DIR/crawl.sock \
     http://localhost/events | grep '"domain":"power"'
```

---

## Quickshell integration

crawl is designed to be the backend for a Quickshell QML shell.

### Consuming the SSE stream

```qml
// In your Quickshell root or a dedicated Service component
import Quickshell
import Quickshell.Io

pragma Singleton

Singleton {
    id: root

    property real cpuUsage: 0
    property real batteryPercent: 0
    property string batteryState: "unknown"
    property bool onAc: true
    property var notifications: []

    // Active media player
    property string mediaTitle: ""
    property string mediaArtist: ""
    property string mediaStatus: "stopped"

    Process {
        id: eventStream
        command: ["curl", "--no-buffer",
                  "--unix-socket", Quickshell.env("XDG_RUNTIME_DIR") + "/crawl.sock",
                  "http://localhost/events"]
        running: true

        stdout: SplitParser {
            onRead: (line) => {
                if (!line.startsWith("data: ")) return
                try {
                    const evt = JSON.parse(line.slice(6))
                    root.handleEvent(evt)
                } catch (_) {}
            }
        }
    }

    function handleEvent(evt) {
        switch (evt.domain) {
        case "sysmon":
            if (evt.data.event === "cpu_update")
                root.cpuUsage = evt.data.cpu.aggregate
            break
        case "power":
            if (evt.data.event === "battery_update") {
                root.batteryPercent = evt.data.status.percent
                root.batteryState   = evt.data.status.state
                root.onAc           = evt.data.status.on_ac
            }
            break
        case "notify":
            if (evt.data.event === "new")
                root.notifications.push(evt.data.notification)
            else if (evt.data.event === "closed")
                root.notifications = root.notifications
                    .filter(n => n.id !== evt.data.id)
            break
        case "media":
            if (evt.data.event === "track_changed") {
                root.mediaTitle  = evt.data.player.title  ?? ""
                root.mediaArtist = evt.data.player.artist ?? ""
                root.mediaStatus = evt.data.player.status
            }
            break
        }
    }

    // One-shot HTTP requests to the daemon
    function setBrightness(percent) {
        crawlRequest("POST", "/brightness/set", { value: percent })
    }
    function setVolume(percent) {
        crawlRequest("POST", "/audio/volume", { percent: percent })
    }
    function mediaNext() {
        crawlRequest("POST", "/media/next", {})
    }
    function dismissNotification(id) {
        crawlRequest("DELETE", "/notify/" + id, null)
    }

    function crawlRequest(method, path, body) {
        // TODO: wire up via Quickshell NetworkRequest or a Process curl call
        // NetworkRequest doesn't support Unix sockets natively yet;
        // use Process + curl as a bridge or the CrawlDesktopShell axum bridge crate.
    }
}
```

### Bar widget examples

```qml
// Battery widget reading from CrawlService
Text {
    text: {
        const pct = CrawlService.batteryPercent.toFixed(0)
        const icon = CrawlService.onAc ? "󰂄" : "󰁹"
        return icon + " " + pct + "%"
    }
    color: CrawlService.batteryPercent < 20 ? "#f38ba8" : "#cdd6f4"
}

// CPU widget
Text {
    text: "󰘚 " + CrawlService.cpuUsage.toFixed(1) + "%"
}

// Media widget
Row {
    Text { text: CrawlService.mediaArtist + " — " + CrawlService.mediaTitle }
    MouseArea {
        onClicked: CrawlService.mediaNext()
    }
}
```

---

## Project structure

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

---

## Development

### Building

```bash
# Full workspace
cargo build --workspace

# Release binaries only
cargo build --release --bins

# Single crate
cargo build -p crawl-sysmon

# Run daemon directly (no install)
CRAWL_LOG=debug cargo run -p crawl-daemon
```

### Running locally

```bash
# Terminal 1 — run daemon
CRAWL_LOG=debug cargo run -p crawl-daemon

# Terminal 2 — use CLI against it
cargo run -p crawl-cli -- sysmon --cpu
cargo run -p crawl-cli -- brightness --set=75
cargo run -p crawl-cli -- media --status

# Or use curl directly
SOCK=$XDG_RUNTIME_DIR/crawl.sock
curl --unix-socket $SOCK http://localhost/health
curl --unix-socket $SOCK http://localhost/sysmon/cpu | jq .
curl --unix-socket $SOCK http://localhost/power/battery | jq .

# Watch the SSE stream
curl --no-buffer --unix-socket $SOCK http://localhost/events
```

### Adding a domain

1. **Create the crate:**
   ```bash
   cargo new --lib crates/crawl-newdomain
   ```

2. **Add to workspace** in root `Cargo.toml`:
   ```toml
   members = [ ..., "crates/crawl-newdomain" ]
   ```

3. **Implement the interface** — every domain crate exposes:
   ```rust
   // Config struct with Default impl (used by figment)
   pub struct Config { ... }

   // Entry point — called by crawl-daemon, runs indefinitely
   pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()>
   ```

4. **Add events** to `crawl-ipc/src/events.rs`:
   ```rust
   pub enum CrawlEvent {
       ...
       NewDomain(NewDomainEvent),
   }
   pub enum NewDomainEvent { ... }
   ```

5. **Add types** to `crawl-ipc/src/types.rs` if needed.

6. **Wire into daemon:**
   - Add dep in `crawl-daemon/Cargo.toml`
   - Add `Config` field in `crawl-daemon/src/config.rs`
   - Spawn in `spawn_domains()` in `main.rs`
   - Add routes in `router.rs`

7. **Add CLI subcommand** in `crawl-cli/src/cmd/`.

### Debugging the socket

```bash
SOCK=$XDG_RUNTIME_DIR/crawl.sock

# List all routes (health check)
curl -s --unix-socket $SOCK http://localhost/health | jq

# Watch all events, pretty-print JSON
curl --no-buffer --unix-socket $SOCK http://localhost/events \
    | while IFS= read -r line; do
        [[ "$line" == data:* ]] && echo "${line#data: }" | jq --tab .
      done

# Test posting to an endpoint
curl -s --unix-socket $SOCK \
     -X POST -H 'Content-Type: application/json' \
     -d '{"value": 70}' \
     http://localhost/brightness/set | jq

# Check if daemon is running
systemctl --user is-active crawl
```

### Logging

Set `CRAWL_LOG` or `RUST_LOG` to control log verbosity:

```bash
CRAWL_LOG=debug                       # everything
CRAWL_LOG=crawl_bluetooth=trace,info         # Bluetooth domain verbose, others info
CRAWL_LOG=warn,crawl_notify=debug     # only warnings except notify domain
```

Logs go to the systemd journal when running as a service:
```bash
journalctl --user -u crawl -f
journalctl --user -u crawl -f --output=cat   # no metadata prefix
```

---

## Roadmap

**0.1 — Foundation (current)**
- [x] Workspace structure and crawl-ipc types
- [x] axum daemon over Unix socket
- [x] SSE broadcast event stream
- [x] clap CLI skeleton with all domain subcommands
- [x] All 11 domain crates scaffolded with real crate deps

**0.2 — Core domains working**
- [ ] `crawl-sysmon` — CPU/mem/disk fully implemented
- [ ] `crawl-brightness` — sysfs read/write working end-to-end
- [ ] `crawl-power` — UPower battery reading working
- [ ] `crawl-proc` — process list and kill working
- [ ] HTTP router handlers wired to domain query functions

**0.3 — D-Bus domains**
- [ ] `crawl-notify` — notification daemon fully working
- [ ] `crawl-media` — MPRIS aggregator with track changes
- [ ] `crawl-bluetooth` — scan, connect, disconnect working
- [ ] `crawl-network` — NM status and WiFi list working
- [ ] `crawl-audio` — sink volume and mute working

**0.4 — Polish**
- [ ] `crawl-disk` — mount/unmount working
- [ ] `crawl-clipboard` — clipboard watch and history
- [ ] `--watch` mode in CLI (streaming sysmon/events)
- [ ] Shell completions (bash, zsh)
- [ ] Man pages

**0.5 — CrawlDesktopShell integration**
- [ ] Quickshell DataStream bridge
- [ ] `org.freedesktop.ScreenSaver` inhibit implementation
- [ ] Idle detection via `ext-idle-notify-v1`
- [ ] Color temperature control
- [ ] AUR package

---

## License

MIT — see [LICENSE](LICENSE).

---

## Related projects

- [CrawlDesktopShell](https://github.com/me-osano/crawldesktopshell) — the Quickshell shell that crawl is built for
- [RUDE](https://github.com/me-osano/rude) — Rust Unified Download Engine, sister project
- [bluer](https://github.com/bluez/bluer) — async BlueZ D-Bus bindings
- [zbus](https://github.com/dbus2/zbus) — native Rust D-Bus implementation
- [sysinfo](https://github.com/GuillaumeGomez/sysinfo) — cross-platform system info
- [playerctl](https://github.com/altdesktop/playerctl) — MPRIS CLI (what crawl-media replaces)
