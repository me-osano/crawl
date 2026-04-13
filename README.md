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
- [Documentation](#documentation)
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

## Domains

| Domain | Crate | Backend | What it owns |
|---|---|---|---|
| Bluetooth | `crawl-bluetooth` | `bluer` / BlueZ D-Bus | Discovery, pair, trust, connect/disconnect, alias, Bluetooth battery |
| Network | `crawl-network` | `zbus` / NetworkManager | WiFi scan, connect, interface status, master power |
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

### Arch Linux (PKGBUILD) — recommended

Install:
```bash
curl -fsSL https://raw.githubusercontent.com/me-osano/crawl/master/pkg/install.sh | sh
```

Update:
```bash
curl -fsSL https://raw.githubusercontent.com/me-osano/crawl/master/pkg/update.sh | sh
```

Or use the CLI:
```bash
crawl update
```

Check the latest release tag without installing:
```bash
crawl update --dry-run
```

Uninstall:
```bash
curl -fsSL https://raw.githubusercontent.com/me-osano/crawl/master/pkg/uninstall.sh | sh
```
OR (purge crawl and its files)
```bash
curl -fsSL https://raw.githubusercontent.com/me-osano/crawl/master/pkg/uninstall.sh | sh -s -- --purge
```

Manual PKGBUILD:
```bash
git clone https://github.com/me-osano/crawl
cd crawl/pkg
makepkg -si
```

The PKGBUILD installs:
- `/usr/bin/crawl-daemon` and `/usr/bin/crawl`
- `/usr/lib/systemd/user/crawl.service`
- `/usr/lib/udev/rules.d/90-crawl-backlight.rules`
- `/etc/crawl/crawl.toml`
- `/usr/share/crawl/themes/*.toml`

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

[theme]
active = "catppuccin-mocha"   # or "dynamic" for matugen
variant = "dark"              # "dark" or "light"
wallpaper_cmd = "swww img {path}"
write_gtk = true
write_ghostty = true
write_shell = true
write_json = true
```


## Documentation

- `docs/ARCHITECTURE.md` — daemon, IPC, events, domain isolation
- `docs/CLI.md` — full CLI reference
- `docs/CRAWL_IPC.md` — HTTP API + SSE event stream
- `docs/QUICKSHELL.md` — QML integration examples
- `docs/PROJECT_STRUCTURE.md` — workspace layout
- `docs/DEVELOPMENT.md` — build/run/debugging
- `docs/CRAWL_THEME.md` — theming system

# CLI Reference

All commands accept `--json` / `-j` for raw JSON output (useful in scripts).
All commands accept `--socket <path>` to override the daemon socket.

---

## brightness

```bash
crawl brightness                    # get current
crawl brightness --set=80           # set to 80%
crawl brightness --inc=5            # increase by 5%
crawl brightness --dec=10           # decrease by 10%
```

## sysmon

```bash
crawl sysmon --cpu                  # CPU usage + load averages
crawl sysmon --mem                  # memory usage
crawl sysmon --disk                 # disk usage per mount
crawl sysmon --watch                # live CPU/memory updates (SSE)
crawl sysmon --cpu --json           # raw JSON
```

## bluetooth

```bash
crawl bluetooth                            # status + device list
crawl bluetooth --scan                     # start discovery
crawl bluetooth --connect=AA:BB:CC:DD:EE:FF
crawl bluetooth --disconnect=AA:BB:CC:DD:EE:FF
crawl bluetooth --power=on
crawl bluetooth --power=off
```

## network

```bash
crawl network                                # connectivity status
crawl network --power=on                     # enable networking
crawl network --power=off                    # disable networking

crawl network --wifi --list                  # list nearby WiFi networks
crawl network --wifi --scan                  # trigger WiFi scan
crawl network --wifi --connect --ssid=MySSID --password=hunter2
crawl network --wifi --disconnect

crawl network --eth --list                   # list wired interfaces
crawl network --eth --connect                # connect first wired interface
crawl network --eth --connect --iface=enp3s0 # connect specific wired interface
crawl network --eth --disconnect             # disconnect active wired interface
crawl network --eth --disconnect --iface=enp3s0
```

## audio

```bash
crawl audio                              # list sinks with volume
crawl audio --output --volume=70         # set output volume to 70%
crawl audio --output --mute              # toggle output mute
crawl audio --input --volume=70          # set input volume to 70%
crawl audio --input --mute               # toggle input mute
crawl audio --input --list               # list microphones / sources
```

## media

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

## power

```bash
crawl power                         # battery percent, state, time estimates
crawl power --json
```

## notify

```bash
crawl notify --list                 # all active notifications
crawl notify --title="Build done" --body="cargo build succeeded"
crawl notify --title="Alert" --body="Disk full" --urgency=critical
crawl notify --dismiss=42           # dismiss notification by ID
```

## clip

```bash
crawl clip --get                    # current clipboard content
crawl clip --set="some text"        # write to clipboard
crawl clip --history                # clipboard history (JSON)
```

## proc

```bash
crawl proc                          # top 20 processes by CPU
crawl proc --sort=mem --top=10      # top 10 by memory
crawl proc --find=firefox           # find by name
crawl proc --kill=1234              # SIGTERM
crawl proc --kill=1234 --force      # SIGKILL
crawl proc --watch=1234             # wait for PID to exit
```

## disk

```bash
crawl disk                          # list block devices
crawl disk --mount=/dev/sdb1        # mount device
crawl disk --unmount=/dev/sdb1
crawl disk --eject=/dev/sdb         # eject drive
```

## daemon

```bash
crawl daemon                        # status + version
crawl daemon --restart
crawl daemon --stop
```

## theme

```bash
crawl theme --status
crawl theme --list=dark
crawl theme --list=light
crawl theme --dark --set-custom=rose-pine
crawl theme --light --set-custom=catppuccin-latte
crawl theme --dark --set-dynamic=tonalspot
crawl theme --wallpaper=~/Pictures/wall.jpg
crawl theme --wallpaper=~/Pictures/wall.jpg --no-generate
crawl theme --dark
crawl theme --light
crawl theme --regenerate
```

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
