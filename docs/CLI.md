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

Theme notes:
- `crawl theme --list` only shows themes from `assets/themes` that match the current variant.
- Dynamic matugen supports optional schemes via `theme.dynamic_scheme` in `crawl.toml`.
- Switching `--dark`/`--light` falls back to a default variant theme if the current theme has no matching variant.
