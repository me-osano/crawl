# Sysinfo

Provides system information aggregation for the Crawl desktop stack.

## CLI Usage

```bash
# Full output with logo
crawl sysinfo

# JSON output
crawl sysinfo -j

# Specific field only
crawl sysinfo -f compositor

# No logo
crawl sysinfo --no-logo

# Custom logo
crawl sysinfo --logo arch
```

## Fields

| Field | Source | Description |
|-------|--------|-------------|
| OS | `/etc/os-release` | OS name, kernel, hostname |
| Hardware | `/proc/cpuinfo`, `/proc/meminfo` | CPU model, cores, memory |
| Session | Environment vars | User, shell, terminal, uptime |
| Compositor | Environment vars | DE/WM detection |
| Display | DRM, wlr-randr | Monitors, resolution, scale |
| Disk | `df` | Root partition usage |

## Compositor Detection

Detected via environment variables:

| Compositor | Variable |
|------------|----------|
| Hyprland | `HYPRLAND_INSTANCE_SIGNATURE` |
| Sway | `SWAYSOCK` |
| Niri | `NIRI_SOCKET` |
| Labwc | `LABWC_PID` |
| Mango | `XDG_CURRENT_DESKTOP` |

## Architecture

```
crawl-sysinfo/
├── lib.rs          # Public API
├── service.rs      # SystemService aggregator
├── models.rs       # Data types
├── compositor.rs  # Compositor detection
├── os.rs          # OS info
├── session.rs     # Session info
├── hardware.rs    # Hardware info
└── display.rs    # Display/monitor info
```

## Output Format
```
                 -`                    ┌───Hardware────────────
                .o+`                   | Kernel  |   6.19.14-zen1-1-zen
               `ooo/                   | Host    |   misarch
              `+oooo:                  | Uptime  |   1 hours, 37 mins
             `+oooooo:                 | Shell   |   bash
             -+oooooo+:                ┌───Software────────────
           `/:-:++oooo+:               | OS      |   Arch Linux
          `/++++/+++++++:              | Resolution            1920x1080 @ 1.0x
         `/++++++++++++++:             | CPU                   AMD Ryzen 5 PRO 3500U w/ Radeon Vega Mobile Gfx (8 cores)
        `/+++ooooooooooooo/`           Memory                13.5 GB
       ./ooosssso++osssssso+`          GPU                   04:00.0 VGA compatible controller: Advanced Micro Devices, Inc. [Radeon Vega Mobile Series] (rev d2)
      .oossssso-````/ossssss+`         Disk                  160G / 78G
     -osssssso.      :ssssssso.
    :osssssss/        osssso+++.
   /ossssssss/        +ssssooo/-
 `/ossssso+/:-        -:/+osssso+-
`+sso+:-`                 `.-/+oso:
`++:.                           `-/+/
.`                                 `/
```