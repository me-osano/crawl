# Crawl IPC / HTTP API

This document describes the HTTP-over-Unix-socket API exposed by the daemon and
the SSE event stream used for live updates.

---

## Overview

The daemon exposes a standard HTTP/1.1 API over a Unix socket. You can talk to
it with any HTTP client that supports Unix sockets.

Socket path: `$XDG_RUNTIME_DIR/crawl.sock`

### Quick examples

```bash
curl --unix-socket $XDG_RUNTIME_DIR/crawl.sock http://localhost/health
curl --unix-socket $XDG_RUNTIME_DIR/crawl.sock http://localhost/sysmon/cpu
curl --unix-socket $XDG_RUNTIME_DIR/crawl.sock \
     -X POST -H 'Content-Type: application/json' \
     -d '{"value":80}' http://localhost/brightness/set
```

With `socat`:

```bash
echo -e 'GET /sysmon/cpu HTTP/1.0\r\n\r\n' | \
    socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/crawl.sock
```

---

## Request/response

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

### Health

```
GET  /health
→ { "status": "ok", "version": "0.1.4" }
```

### Bluetooth

```
GET  /bluetooth/status          → BluetoothStatus
GET  /bluetooth/devices         → [BluetoothDevice]
POST /bluetooth/scan            → {}
POST /bluetooth/connect         ← { "address": "AA:BB:CC:DD:EE:FF" }
POST /bluetooth/disconnect      ← { "address": "AA:BB:CC:DD:EE:FF" }
POST /bluetooth/power           ← { "on": true }
POST /bluetooth/pair            ← { "address": "AA:BB:CC:DD:EE:FF" }
POST /bluetooth/trust           ← { "address": "AA:BB:CC:DD:EE:FF", "trusted": true }
POST /bluetooth/remove          ← { "address": "AA:BB:CC:DD:EE:FF" }
POST /bluetooth/alias           ← { "address": "AA:BB:CC:DD:EE:FF", "alias": "Headphones" }
POST /bluetooth/discoverable    ← { "on": true }
POST /bluetooth/pairable        ← { "on": true }
```

### Network

```
GET  /network/status              → NetStatus (includes `mode`)
GET  /network/wifi                → [WifiNetwork]
POST /network/wifi/scan           ← {}
POST /network/wifi/connect        ← { "ssid": "MyWifi", "password": "..." }
POST /network/wifi/disconnect     ← {}
POST /network/power               ← { "on": true }
POST /network/eth/connect         ← { "interface": "enp3s0" }  # optional: auto-select if omitted
POST /network/eth/disconnect      ← { "interface": "enp3s0" }  # optional: auto-select active if omitted
```

### Notifications

```
GET    /notify/list      → [Notification]
POST   /notify/send      ← { "title": "...", "body": "...", "urgency": "normal" }
DELETE /notify/:id       → {}
```

### Clipboard

```
GET  /clipboard          → ClipEntry
POST /clipboard          ← { "content": "text" }
GET  /clipboard/history  → [ClipEntry]
```

### Sysmon

```
GET  /sysmon/cpu         → CpuStatus
GET  /sysmon/mem         → MemStatus
GET  /sysmon/disk        → [DiskStatus]
```

### Brightness

```
GET  /brightness         → BrightnessStatus
POST /brightness/set     ← { "value": 80 }
POST /brightness/inc     ← { "value": 5 }
POST /brightness/dec     ← { "value": 5 }
```

### Processes

```
GET  /proc/list          → [ProcessInfo]   (?sort=cpu&top=20)
GET  /proc/find          → [ProcessInfo]   (?name=firefox)
GET  /proc/watch/:pid    → { "pid": 1234, "name": "bash", "exit_code": null }
POST /proc/:pid/kill     ← { "force": false }
```

### Media

```
GET  /media/players      → [MediaPlayer]
GET  /media/active       → MediaPlayer
POST /media/play         ← { "player": null }
POST /media/pause        ← { "player": null }
POST /media/next         ← { "player": null }
POST /media/prev         ← { "player": null }
POST /media/volume       ← { "volume": 0.8, "player": null }
```

### Power

```
GET  /power/battery      → BatteryStatus
```

### Disk

```
GET  /disk/list          → [BlockDevice]
POST /disk/mount         ← { "device": "/org/freedesktop/UDisks2/block_devices/sdb1" }
POST /disk/unmount       ← { "device": "..." }
POST /disk/eject         ← { "device": "..." }
```

### Audio

```
GET  /audio/sinks        → [AudioDevice]
GET  /audio/sources      → [AudioDevice]
POST /audio/volume       ← { "percent": 70, "device": "input|output" }
POST /audio/mute         ← { "device": "input|output" }
```

### Theme

```
GET  /theme/list                 → { "themes": ["..."] }
GET  /theme/list?variant=dark    → { "themes": ["..."] }
GET  /theme/list?variant=light   → { "themes": ["..."] }
POST /theme/custom               ← { "name": "rose-pine", "variant": "dark" }
POST /theme/dynamic              ← { "scheme": "tonalspot", "variant": "dark" }
POST /theme/variant              ← { "variant": "dark" }
POST /theme/wallpaper            ← { "path": "...", "no_generate": false }
POST /theme/regenerate           ← {}
```

---

## SSE event stream

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
| `network` | `connected`, `disconnected`, `ip_changed`, `wifi_enabled`, `wifi_disabled`, `connectivity_changed`, `mode_changed` |
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

Consume with curl:

```bash
curl --no-buffer --unix-socket $XDG_RUNTIME_DIR/crawl.sock \
     http://localhost/events
```

Filter a single domain:

```bash
curl --no-buffer --unix-socket $XDG_RUNTIME_DIR/crawl.sock \
     http://localhost/events | grep '"domain":"power"'
```
