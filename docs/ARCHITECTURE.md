# Architecture

High-level view of how crawl is structured and how data flows through the
daemon, domains, and clients.

---

## Overview

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

## IPC transport

All communication uses HTTP/1.1 over a Unix domain socket. The wire format is
JSON. This means:

- The CLI is just an HTTP client
- Quickshell's `NetworkRequest` can talk to it natively
- You can debug with `socat` and `curl --unix-socket`
- No custom protocol to maintain

---

## Event model

Every domain task holds a `tokio::sync::broadcast::Sender<CrawlEvent>`. When
something changes (battery level, Bluetooth device connects, notification
arrives), it sends to that channel. The SSE handler fans events out to all
connected `GET /events` clients as newline-delimited JSON.

---

## Domain isolation

Each domain lives in its own crate (`crawl-bluetooth`, `crawl-audio`, etc.) with
no dependency on other domains. The only shared surface is `crawl-ipc`, which
contains the serializable types and event enum. This means:

- Domains are independently testable
- A domain crashing doesn't bring down others (each runs in its own task)
- `crawl-ipc` can be used in future QML bridge crates or other tools
