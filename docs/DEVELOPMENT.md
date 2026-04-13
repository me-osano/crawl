# Development

---

## Building

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

---

## Running locally

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

---

## Adding a domain

1. Create the crate:
   ```bash
   cargo new --lib crates/crawl-newdomain
   ```

2. Add to workspace in `Cargo.toml`:
   ```toml
   members = [ ..., "crates/crawl-newdomain" ]
   ```

3. Implement the interface — every domain crate exposes:
   ```rust
   // Config struct with Default impl (used by figment)
   pub struct Config { ... }

   // Entry point — called by crawl-daemon, runs indefinitely
   pub async fn run(cfg: Config, tx: broadcast::Sender<CrawlEvent>) -> anyhow::Result<()>
   ```

4. Add events to `crawl-ipc/src/events.rs`:
   ```rust
   pub enum CrawlEvent {
       ...
       NewDomain(NewDomainEvent),
   }
   pub enum NewDomainEvent { ... }
   ```

5. Add types to `crawl-ipc/src/types.rs` if needed.

6. Wire into daemon:
   - Add dep in `crawl-daemon/Cargo.toml`
   - Add `Config` field in `crawl-daemon/src/config.rs`
   - Spawn in `spawn_domains()` in `main.rs`
   - Add routes in `router.rs`

7. Add CLI subcommand in `crawl-cli/src/cmd/`.

---

## Debugging the socket

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

---

## Logging

Set `CRAWL_LOG` or `RUST_LOG` to control log verbosity:

```bash
CRAWL_LOG=debug                       # everything
CRAWL_LOG=crawl_bluetooth=trace,info  # Bluetooth domain verbose, others info
CRAWL_LOG=warn,crawl_notify=debug     # only warnings except notify domain
```

Logs go to the systemd journal when running as a service:

```bash
journalctl --user -u crawl -f
journalctl --user -u crawl -f --output=cat   # no metadata prefix
```
