# OpenNiri-Windows

[![CI](https://github.com/AdEx-Partners-DE/OpenNiri-Windows/actions/workflows/ci.yml/badge.svg)](https://github.com/AdEx-Partners-DE/OpenNiri-Windows/actions/workflows/ci.yml)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)](LICENSE)

OpenNiri-Windows is a scrollable tiling window manager for Windows 10/11, built in Rust.

It brings the Niri-style horizontal workspace model to native Windows without replacing DWM.

## Product Positioning

Most Windows tilers are tree/BSP-driven. OpenNiri-Windows is scroll-first:

- Windows are arranged on a horizontal strip.
- Your monitor acts as a viewport over that strip.
- Navigation remains spatially consistent as windows are added.
- You move through workspace context instead of constantly rebuilding split trees.

## Who This Is For

- Keyboard-driven users who manage many windows every day.
- Engineers, analysts, operators, and creators on single or multi-monitor setups.
- Teams that want an open-source Windows tiler with transparent architecture and Rust codebase.

## Capability Snapshot

Implemented now:

- Multi-monitor workspaces with monitor-aware focus and move commands
- Global hotkeys with live config reload
- Floating and fullscreen toggles
- Width presets (`Win+1/2/3`) and equalize (`Win+0`)
- Smooth scroll animations, snap hints, and touchpad gestures
- Optional focus-follows-mouse
- System tray actions (pause/reload/open config/open logs/exit)
- Workspace persistence and safer shutdown/recovery behavior

## Product Status

OpenNiri-Windows is **alpha** and under active development.

What this means in practice:

- Core behavior is implemented and tested in CI.
- UX is currently keyboard/config-first (no full GUI setup flow yet).
- Some Windows-managed/system windows can reject movement or styling operations.

## Quick Start (3 Minutes)

### Prerequisites

- Rust (stable)
- GNU Windows target (`x86_64-pc-windows-gnu`)

### Install and Run

```bash
git clone https://github.com/AdEx-Partners-DE/OpenNiri-Windows.git
cd OpenNiri-Windows
cargo build --release
cargo run -p openniri-cli -- init
cargo run -p openniri-cli -- run
```

### Verify / Stop

```bash
cargo run -p openniri-cli -- status
cargo run -p openniri-cli -- stop
```

### Daily Start

```bash
cargo run -p openniri-cli -- run
```

## Default Hotkeys

| Key | Action |
|---|---|
| `Win+H / Win+L` | Focus left / right |
| `Win+J / Win+K` | Focus down / up |
| `Win+Shift+H / Win+Shift+L` | Move column left / right |
| `Win+Ctrl+H / Win+Ctrl+L` | Shrink / grow column |
| `Win+Alt+H / Win+Alt+L` | Focus monitor left / right |
| `Win+Alt+Shift+H / Win+Alt+Shift+L` | Move window to monitor left / right |
| `Win+Shift+Q` | Close focused window |
| `Win+F` | Toggle floating |
| `Win+Shift+F` | Toggle fullscreen |
| `Win+1 / Win+2 / Win+3` | Set width to 1/3, 1/2, 2/3 |
| `Win+0` | Equalize all column widths |
| `Win+R` | Refresh (re-enumerate windows) |

## Config and Runtime Paths

Config file:

- `%APPDATA%\\openniri\\config\\config.toml`

State data:

- `%APPDATA%\\openniri\\data\\workspace-state.json`

Daemon logs:

- `%TEMP%\\openniri-daemon.log`
- `%TEMP%\\openniri-daemon.err.log`

## Architecture

OpenNiri-Windows is a Rust workspace:

| Crate | Responsibility |
|---|---|
| `openniri-core-layout` | Platform-agnostic layout engine |
| `openniri-platform-win32` | Win32 integration and window operations |
| `openniri-ipc` | Named-pipe command/response protocol |
| `openniri-daemon` | Runtime event loop and state management |
| `openniri-cli` | User-facing command line interface |

Technical docs:

- `docs/SPEC.md`
- `docs/ARCHITECTURE.md`
- `docs/WINDOWS_CONSTRAINTS.md`
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `docs/PUBLIC_READINESS_CHECKLIST.md`

## Platform Constraints

OpenNiri-Windows is a **window controller**, not a compositor.

- DWM remains the compositor.
- Elevated or protected windows may reject placement/styling changes.
- Behavior can vary across app frameworks (Win32/WPF/Electron/UWP).

## Public Readiness Plan

The execution checklist for shipping this as a polished public product is here:

- `docs/PUBLIC_READINESS_CHECKLIST.md`

## Contributing

See `CONTRIBUTING.md`.

## License

GPL-3.0. See `LICENSE`.
