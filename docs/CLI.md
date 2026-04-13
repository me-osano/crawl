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
crawl network                           # connectivity status
crawl network --wifi                    # list nearby WiFi networks
crawl network --connect=MySSID --password=hunter2
crawl network --eth                     # list wired interfaces
crawl network --eth-connect             # connect first wired interface
crawl network --eth-connect=enp3s0      # connect specific wired interface
crawl network --eth-disconnect          # disconnect active wired interface
crawl network --eth-disconnect=enp3s0   # disconnect specific wired interface
```

## audio

```bash
crawl audio                         # list sinks with volume
crawl audio --volume=70             # set default sink to 70%
crawl audio --mute                  # toggle mute
crawl audio --sources               # list microphones / sources
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
crawl theme --list
crawl theme --set=rose-pine
crawl theme --wallpaper=~/Pictures/wall.jpg
crawl theme --wallpaper=~/Pictures/wall.jpg --no-generate
crawl theme --dark
crawl theme --light
crawl theme --regenerate
```
