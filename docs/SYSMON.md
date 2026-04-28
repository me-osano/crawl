# Sysmon

System monitoring via sysinfo. Polls CPU, memory, disk, network, and GPU at configurable intervals. Broadcasts events to IPC subscribers.

## CLI Usage

```bash
# CPU usage (default)
crawl sysmon
crawl sysmon --cpu

# Memory usage
crawl sysmon --mem

# Disk usage per mount
crawl sysmon --disk

# Network throughput
crawl sysmon --net

# GPU info
crawl sysmon --gpu

# Live updates (press Ctrl-C to stop)
crawl sysmon --watch

# JSON output
crawl sysmon --json
crawl sysmon --cpu --json
```

## IPC Commands

| Command | Params | Returns |
|---------|--------|---------|
| `SysmonCpu` | none | `CpuStatus` (aggregate, cores, freq, load_avg) |
| `SysmonMem` | none | `MemStatus` (total, used, available, swap) |
| `SysmonDisk` | none | `Vec<DiskStatus>` (mount, total, used, available) |
| `SysmonNet` | none | `NetTraffic` (rx_bytes, tx_bytes, rx_bps, tx_bps) |
| `SysmonGpu` | none | `Option<GpuStatus>` (name, temperature_c) |

## Events

Broadcast via `CrawlEvent::Sysmon`:

| Event | Data | Trigger |
|-------|------|---------|
| `CpuUpdate` | `{ cpu: CpuStatus }` | Poll interval, CPU changed > threshold |
| `MemUpdate` | `{ mem: MemStatus }` | Poll interval, memory changed > threshold |
| `NetUpdate` | `{ traffic: NetTraffic }` | Poll interval, network changed > threshold |
| `GpuUpdate` | `{ gpu: GpuStatus }` | Poll interval, GPU changed |
| `CpuSpike` | `{ usage: f32, threshold: f32 }` | CPU > `cpu_spike_threshold` |
| `MemPressure` | `{ used_percent: f32 }` | Memory > `mem_pressure_threshold` |

## Configuration

Location: `~/.config/crawl/config.toml` (or `$XDG_CONFIG_HOME/crawl/config.toml`)

```toml
[sysmon]
poll_interval_ms = 1000          # Poll interval (ms)
cpu_spike_threshold = 90.0       # CPU spike threshold (%)
mem_pressure_threshold = 85.0    # Memory pressure threshold (%)
cpu_change_threshold = 2.0        # CPU change threshold for broadcast
mem_change_threshold = 1.0        # Memory change threshold for broadcast
net_change_threshold = 1024        # Network change threshold (bytes)
```

Environment variables (prefix `CRAWL_SYSMON__`):
```bash
CRAWL_SYSMON__POLL_INTERVAL_MS=1000
CRAWL_SYSMON__CPU_SPIKE_THRESHOLD=90.0
CRAWL_SYSMON__MEM_PRESSURE_THRESHOLD=85.0
```

## Architecture

```
crawl-sysmon/
├── lib.rs              # Public API + domain runner
├── config.rs          # Configuration
└── (uses crawl-scheduler for optional jitter-based timing)
```

## Data Types

```rust
pub struct CpuStatus {
    pub aggregate: f32,              // Overall CPU usage %
    pub cores: Vec<f32>,             // Per-core usage %
    pub frequency_mhz: Vec<u64>,    // Per-core frequency
    pub load_avg: LoadAvg,           // 1, 5, 15 min load averages
    pub temperature_c: Option<f32>, // Not yet implemented
}

pub struct MemStatus {
    pub total_kb: u64,
    pub used_kb: u64,
    pub available_kb: u64,
    pub swap_total_kb: u64,
    pub swap_used_kb: u64,
}

pub struct NetTraffic {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_bps: u64,    // Bytes per second
    pub tx_bps: u64,
}

pub struct GpuStatus {
    pub name: Option<String>,          // Driver name from DRM
    pub temperature_c: Option<f32>,  // From hwmon
}
```

## Query API (Direct)

```rust
use crawl_sysmon::{get_cpu, get_mem, get_disks, get_net, get_gpu};

let cpu = get_cpu();
let mem = get_mem();
let disks = get_disks();
let net = get_net();
let gpu = get_gpu();
```
