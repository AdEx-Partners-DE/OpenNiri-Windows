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
- Safe mode for troubleshooting (`--safe-mode`)
- Built-in diagnostics (`openniri-cli doctor`)

## Product Status

OpenNiri-Windows is **alpha** and under active development.

What this means in practice:

- Core behavior is implemented and tested in CI.
- UX is currently keyboard/config-first (no full GUI setup flow yet).
- Some Windows-managed/system windows can reject movement or styling operations.
- Current release state (as of 2026-02-08): no tagged public release yet, so source build is the primary install path.

## Quick Start

### Option A: Download (When Available)

1. Open [GitHub Releases](https://github.com/AdEx-Partners-DE/OpenNiri-Windows/releases)
2. If a tagged release is available, download the latest `.zip` archive
3. Extract `openniri.exe` and `openniri-cli.exe` to a folder
4. (Optional) Add the folder to your `PATH`
5. Generate a default config:
   ```
   openniri-cli init
   ```
6. Start the daemon:
   ```
   openniri-cli run
   ```

### Option B: Build from Source (Recommended for Current Alpha)

Prerequisites: [Rust](https://rustup.rs) GNU toolchain (`stable-x86_64-pc-windows-gnu`) and MinGW linker

```bash
rustup toolchain install stable-x86_64-pc-windows-gnu
git clone https://github.com/AdEx-Partners-DE/OpenNiri-Windows.git
cd OpenNiri-Windows
cargo +stable-x86_64-pc-windows-gnu build --release
cargo +stable-x86_64-pc-windows-gnu run -p openniri-cli -- init
cargo +stable-x86_64-pc-windows-gnu run -p openniri-cli -- run
```

The workspace is configured for GNU (`x86_64-pc-windows-gnu`) in `.cargo/config.toml`, and binaries are placed in `target/release/`.

## First Steps After Install

| Step | Command | What It Does |
|------|---------|--------------|
| Create config | `openniri-cli init` | Writes a default `config.toml` with comments |
| Start daemon | `openniri-cli run` | Launches the window manager in the background |
| Check status | `openniri-cli status` | Shows version, monitors, windows, uptime |
| Toggle pause | `openniri-cli toggle-pause` (alias: `pause`) | Pauses/resumes tiling without stopping the daemon |
| Run diagnostics | `openniri-cli doctor` | Checks binary, config, daemon, and system state |
| Stop daemon | `openniri-cli stop` | Requests shutdown, waits for teardown confirmation, and runs local emergency restore if confirmation fails |
| Emergency local restore | `openniri-cli emergency-uncloak` (alias: `restore-windows`) | Best-effort local visibility restore without daemon IPC |
| Reload config | `openniri-cli reload` | Applies config changes without restarting |

If the desktop becomes unresponsive or focus gets trapped, use non-destructive recovery first:

```
openniri-cli panic-revert
openniri-cli status   # must fail (timeout OR "Daemon is not running...") before rerun
```

If `panic-revert` cannot confirm daemon response, it now also executes a local visibility restore path automatically.  
`openniri-cli stop` now uses the same local restore safety net when shutdown confirmation is ambiguous.
You can also use `openniri-cli recover` as a short alias for `panic-revert`.
If no terminal is reachable, use tray **Emergency: Uncloak All Windows** and then **Exit**.

Start safe mode only after shutdown is confirmed:

```
openniri-cli run --safe-mode
```

For a full walkthrough, see [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md).
For incident recovery details, see [docs/SUPPORT_PLAYBOOK.md](docs/SUPPORT_PLAYBOOK.md).

## Default Hotkeys

| Key | Action |
|---|---|
| `Win+H / Win+L` | Focus left / right |
| `Win+J / Win+K` | Focus down / up |
| `Win+Shift+H / Win+Shift+L` | Move column left / right |
| `Win+Ctrl+H / Win+Ctrl+L` | Shrink / grow column |
| `Win+Ctrl+Escape` | Emergency visibility restore + daemon panic-revert |
| `Win+Alt+H / Win+Alt+L` | Focus monitor left / right |
| `Win+Alt+Shift+H / Win+Alt+Shift+L` | Move window to monitor left / right |
| `Win+Shift+Q` | Close focused window |
| `Win+F` | Toggle floating |
| `Win+Shift+F` | Toggle fullscreen |
| `Win+1 / Win+2 / Win+3` | Set width to 1/3, 1/2, 2/3 |
| `Win+0` | Equalize all column widths |
| `Win+R` | Refresh (re-enumerate windows) |

## Start Automatically with Windows

```bash
openniri-cli autostart enable
```

This writes a Registry entry under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` that launches the daemon on login. To disable:

```bash
openniri-cli autostart disable
```

## Config and Runtime Paths

Config file:

- `%APPDATA%\openniri\config\config.toml`

State data:

- `%APPDATA%\openniri\data\workspace-state.json`

Daemon logs:

- `%TEMP%\openniri-daemon.log`
- `%TEMP%\openniri-daemon.err.log`

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

- [Getting Started](docs/GETTING_STARTED.md) - Step-by-step walkthrough
- [Configuration](docs/CONFIGURATION.md) - Full config reference with examples
- [Troubleshooting](docs/TROUBLESHOOTING.md) - Common issues and debugging guide
- [Specification](docs/SPEC.md) - Design specification
- [Architecture](docs/ARCHITECTURE.md) - Architecture overview
- [Windows Constraints](docs/WINDOWS_CONSTRAINTS.md) - Win32 platform constraints

## Platform Constraints

OpenNiri-Windows is a **window controller**, not a compositor.

- DWM remains the compositor.
- Elevated or protected windows may reject placement/styling changes.
- Behavior can vary across app frameworks (Win32/WPF/Electron/UWP).

## Contributing

See `CONTRIBUTING.md`.

## License

GPL-3.0. See `LICENSE`.
