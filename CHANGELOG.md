# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and this project adheres to Semantic Versioning.

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
