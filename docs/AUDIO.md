# Audio

Controls audio devices via PipeWire/PulseAudio using libpulse-binding.

## Architecture

```
crawl-audio/
├── lib.rs              # Public API + domain runner
├── Config             # Configuration
└── helpers.rs         # (inline) Volume conversion, device mapping
```

## CLI Usage

### Output Devices (Speakers)
```bash
# List devices
crawl audio

# Set volume (0-100)
crawl audio -v 70

# Mute/unmute
crawl audio --mute
crawl audio --unmute

# List all output devices
crawl audio -l
```

### Input Devices (Microphones)
```bash
# List input devices
crawl audio --input

# Set input volume
crawl audio --input -v 50

# Mute/unmute input
crawl audio --input --mute
crawl audio --input --unmute
```

## Features

- **Volume Control**: Set 0-100% on default device
- **Mute Toggle**: Mute/unmute default device  
- **Device Listing**: Shows all sinks/sources with volume bars
- **Default Detection**: Marks default device with ●
- **Status Display**: Shows muted/playing state

## Output Format

```
Output Devices (Speakers)
────────────────────────
  alsa_output.pci-xxx  (default)
     ● [██████████░░░░] 75%
     🔊 Playing
```

## Daemon Integration

The audio domain runs as a tokio task, connected to PulseAudio via libpulse-binding. It subscribes to sink events and emits:

- `DeviceAdded`
- `DeviceRemoved`
- `VolumeChanged` (TODO)
- `MuteToggled` (TODO)

## IPC Types

```rust
pub enum AudioDeviceKind {
    Sink,    // Output (speakers)
    Source,  // Input (microphones)
}

pub struct AudioDevice {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
    pub kind: AudioDeviceKind,
    pub volume_percent: u32,
    pub muted: bool,
    pub is_default: bool,
}
```