# Crawl CLI

## Global Options
```bash
-j, --json     # Output raw JSON instead of formatted
```

## Commands

### System Information
```bash
crawl sysinfo                            # Full info with logo
crawl sysinfo -j                         # JSON output
crawl sysinfo --no-logo                  # No ASCII logo
crawl sysinfo -f compositor              # Specific field only
```

### Audio
```bash
# Output (speakers)
crawl audio                              # List output devices
crawl audio -l                         # List output devices
crawl audio -v 70                      # Set output volume to 70%
crawl audio --mute                       # Mute output
crawl audio --unmute                     # Unmute output

# Input (microphones)
crawl audio --input                       # List input devices
crawl audio --input -l                    # List microphones
crawl audio --input -v 70              # Set input volume to 70%
crawl audio --input --mute               # Mute input
crawl audio --input --unmute             # Unmute input
```

### Display
```bash
# Brightness
crawl display brightness               # Get brightness status
crawl display brightness-set 80        # Set to 80%
crawl display brightness-inc 5         # Increase by 5%
crawl display brightness-dec 10        # Decrease by 10%

# Wallpaper
crawl display wallpaper                                # Get wallpaper status
crawl display wallpaper-set ~/Pictures/wall.png        # Set wallpaper
crawl display wallpaper-set ~/wall.png HDMI-1          # Set on specific monitor
crawl display wallpaper-set ~/wall.png --transition random --mode fill
crawl display wallpaper-get                            # Get current wallpaper
crawl display wallpaper-get HDMI-1                     # Get specific monitor
```

### System Monitor
```bash
# CPU (default)
crawl sysmon                         # CPU + load
crawl sysmon --cpu                   # CPU + load
crawl sysmon --cpu --watch          # Live CPU updates (Ctrl-C to stop)

# Memory
crawl sysmon --mem                   # Memory usage

# Disk
crawl sysmon --disk                 # Disk per mount

# Network throughput
crawl sysmon --net                  # Network RX/TX

# GPU
crawl sysmon --gpu                  # GPU name + temperature

# JSON output
crawl sysmon --json
crawl sysmon --cpu --json
```

### Processes
```bash
# List processes (default: by CPU, top 20)
crawl proc
crawl proc --list

# Sort options
crawl proc --sort cpu              # Sort by CPU usage
crawl proc --sort mem              # Sort by memory usage
crawl proc --sort pid              # Sort by PID
crawl proc --sort name             # Sort by name

# Top N
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

### Daemon
```bash
crawl daemon                        # Status + version
crawl daemon --restart
crawl daemon --stop
crawl daemon --json               # JSON output
```