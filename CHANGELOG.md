# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project adheres to Semantic Versioning.

## [0.1.3] - 2026-04-13

### Added
- NetworkManager domain now maintains a persistent connection with periodic status refresh and event publishing.
- Network master power endpoint and status field for global NM enable/disable.
- Bluetooth pairing features: pair, trust, remove, alias rename, discoverable, pairable, and auth agent.
- Bluetooth Battery1 support for device battery percentage.

### Changed
- Network status now reports mode (station/ap/unknown) and a connected AP always wins WiFi dedupe.
- Network power now uses master networking switch (removed per-wifi power control).
- Network events now include `mode_changed`.

### Documentation
- Updated IPC docs with new endpoints and network mode info.

## [0.1.2] - 2026-04-13

### Added
- Theme domain (static templates + dynamic matugen generation) with CLI + API endpoints.
- Theme asset templates packaged under `/usr/share/crawl/themes`.
- PKGBUILD install/update/uninstall helper scripts and `crawl update` CLI command.

### Changed
- PKGBUILD now installs theme templates and includes updated archive checksum.
- Theme configuration defaults expanded (assets_dirs, writers).

### Documentation
- Added theme config and CLI usage to README.

## [0.1.1] - 2026-04-12

### Added
- Implemented sysmon `--watch` in the CLI by consuming the SSE event stream.
- Wired sysmon and brightness HTTP endpoints in the daemon.

### Changed
- Improved SSE serialization handling by logging failures instead of emitting empty events.
- Aligned the systemd unit with the `/usr/local/bin/crawl-daemon` install path.
- Updated `wl-clipboard-rs` to 0.9.3 to address future Rust incompatibilities.

### Documentation
- Documented sysmon `--watch` usage and clarified the systemd unit path.
- Noted that sysmon and brightness endpoints are wired to real data.
