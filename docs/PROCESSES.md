# Processes

Process listing and management via sysinfo. Uses caching with incremental updates for efficiency. Broadcasts process events to IPC subscribers.

## CLI Usage

```bash
# List processes (default: by CPU, top 20)
crawl proc
crawl proc --list

# Sort by CPU, memory, PID, or name
crawl proc --sort cpu
crawl proc --sort mem
crawl proc --sort pid
crawl proc --sort name

# Top N processes
crawl proc --top 10

# Find process by name
crawl proc --find firefox

# Kill process
crawl proc --kill 1234
crawl proc --kill 1234 --force    # SIGKILL instead of SIGTERM

# Watch process (wait for exit)
crawl proc --watch 1234

# JSON output
crawl proc --json
crawl proc --find firefox --json
```

## IPC Commands

| Command | Params | Returns |
|---------|--------|---------|
| `ProcList` | `{ "sort": "cpu", "top": 20 }` | `Vec<ProcessInfo>` |
| `ProcTop` | `{ "limit": 10 }` | `{ "top_by_cpu": [...], "top_by_mem": [...] }` |
| `ProcFind` | `{ "name": "firefox" }` | `Vec<ProcessInfo>` |
| `ProcKill` | `{ "pid": 1234, "force": false }` | `{ "ok": true }` |
| `ProcWatch` | `{ "pid": 1234 }` | `{ "pid": 1234, "name": "...", "exit_code": null }` |

## Events

Broadcast via `CrawlEvent::Proc`:

| Event | Data | Trigger |
|-------|------|---------|
| `TopUpdate` | `{ top_by_cpu: Vec<ProcessInfo>, top_by_mem: Vec<ProcessInfo> }` | Every `top_interval_ms` |

Note: `Spawned` and `Exited` events are defined but not yet implemented.

## Configuration

Location: `~/.config/crawl/config.toml` (or `$XDG_CONFIG_HOME/crawl/config.toml`)

```toml
[processes]
sort_by = "cpu"        # Default sort field: cpu | mem | pid | name
top = 20                # Default number of processes to return
include_cmd = false     # Include command line in process info (expensive)
top_interval_ms = 1000  # Interval for top-N tracking (ms)
full_interval_ms = 5000 # Interval for full process scan (ms)
```

Environment variables (prefix `CRAWL_PROC__`):
```bash
CRAWL_PROC__SORT_BY=cpu
CRAWL_PROC__TOP=20
CRAWL_PROC__INCLUDE_CMD=false
CRAWL_PROC__TOP_INTERVAL_MS=1000
CRAWL_PROC__FULL_INTERVAL_MS=5000
```

## Architecture

```
crawl-proc/
├── lib.rs              # Public API + domain runner
├── config.rs          # Configuration
└── cache/             # Process cache with incremental updates
    └── mod.rs        # Caching logic, top-N tracking
```

## Data Types

```rust
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: Option<u32>,         // Parent PID
    pub name: String,
    pub exe_path: Option<String>,
    pub cpu_percent: f32,
    pub cpu_ticks: Option<f64>,
    pub mem_rss_kb: u64,           // Resident set size in KB
    pub status: String,             // Running, Sleep, etc.
    pub user: Option<String>,
    pub cmd: Vec<String>,           // Command line (if include_cmd=true)
}
```

## Query API (Direct)

```rust
use crawl_proc::{list_processes, find_processes, list_processes_fresh, kill_process, watch_pid};

// List top processes
let procs = list_processes("cpu", 20);

// Find by name
let found = find_processes("firefox");

// Force fresh scan + list
let fresh = list_processes_fresh("mem", 10);

// Kill process
kill_process(1234, false);  // SIGTERM
kill_process(1234, true);   // SIGKILL

// Watch process (async)
let name = watch_pid(1234).await?;
```
