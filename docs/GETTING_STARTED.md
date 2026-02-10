# Getting Started with OpenNiri-Windows

This guide walks you through installing, running, and customizing OpenNiri-Windows.

## Prerequisites

- **Windows 10** (version 1903 or later) or **Windows 11**
- [Rust](https://rustup.rs) GNU toolchain (`stable-x86_64-pc-windows-gnu`)
- MinGW linker (`gcc`) available on `PATH`

## Installation

Current release state (as of 2026-02-08): no tagged public release yet. Use **Build from Source** unless release artifacts are published.

### Download (when tagged releases are available)

1. Go to [GitHub Releases](https://github.com/AdEx-Partners-DE/OpenNiri-Windows/releases)
2. Download the latest `.zip` archive
3. Extract `openniri.exe` (daemon) and `openniri-cli.exe` (CLI) to a folder
4. Optionally, add that folder to your system `PATH` for convenience

### Build from Source (recommended for current alpha)

If you prefer to build from source:

```bash
rustup toolchain install stable-x86_64-pc-windows-gnu
git clone https://github.com/AdEx-Partners-DE/OpenNiri-Windows.git
cd OpenNiri-Windows
cargo +stable-x86_64-pc-windows-gnu build --release
```

The workspace default target is GNU (`x86_64-pc-windows-gnu`) via `.cargo/config.toml`. Binaries are in `target/release/`.

## First Run

### 1. Generate a Config File

```
openniri-cli init
```

This creates `config.toml` at `%APPDATA%\openniri\config\config.toml` with sensible defaults and comments explaining every option.

### 2. Start the Daemon

```
openniri-cli run
```

This launches the daemon in the background. You should see:

- A system tray icon appear near the clock
- Your open windows rearrange into tiled columns
- A startup banner in the log file confirming monitor/window/hotkey counts

### 3. Verify It Works

Try pressing `Win+H` and `Win+L` to move focus between windows, or run:

```
openniri-cli status
```

You should see output like:

```
OpenNiri Daemon Status:
  Version: 0.1.0
  Monitors: 2
  Total windows: 8
  Uptime: 0h 1m 30s
```

### 4. Run Diagnostics (Optional)

If something doesn't look right:

```
openniri-cli doctor
```

This checks your binary, config, daemon status, admin privileges, and Windows version.

## Default Hotkeys

All hotkeys use the `Win` key as the modifier:

| Key | Action |
|-----|--------|
| `Win+H` | Focus column to the left |
| `Win+L` | Focus column to the right |
| `Win+J` | Focus window below (stacked columns) |
| `Win+K` | Focus window above (stacked columns) |
| `Win+Shift+H` | Move column left |
| `Win+Shift+L` | Move column right |
| `Win+Ctrl+H` | Shrink column width |
| `Win+Ctrl+L` | Grow column width |
| `Win+Alt+H` | Focus monitor to the left |
| `Win+Alt+L` | Focus monitor to the right |
| `Win+Alt+Shift+H` | Move window to monitor left |
| `Win+Alt+Shift+L` | Move window to monitor right |
| `Win+Shift+Q` | Close focused window |
| `Win+F` | Toggle floating mode |
| `Win+Shift+F` | Toggle fullscreen |
| `Win+1` | Set column width to 1/3 viewport |
| `Win+2` | Set column width to 1/2 viewport |
| `Win+3` | Set column width to 2/3 viewport |
| `Win+0` | Equalize all column widths |
| `Win+R` | Refresh (re-enumerate windows) |

All hotkeys can be customized in `config.toml` under the `[hotkeys]` section.

## Customizing Your Setup

Edit the config file:

- **Windows**: `%APPDATA%\openniri\config\config.toml`
- Open it with: right-click tray icon > "Open Config"

Common customizations:

```toml
[layout]
gap = 10              # Space between columns (pixels)
outer_gap = 10        # Space at screen edges (pixels)
default_column_width = 800

[behavior]
focus_follows_mouse = true    # Hover to focus
focus_new_windows = true      # Auto-focus new windows

[hotkeys]
"Win+H" = "focus_left"
"Win+L" = "focus_right"
# Add your own bindings here
```

After editing, apply changes without restarting:

```
openniri-cli reload
```

For the complete configuration reference, see [CONFIGURATION.md](CONFIGURATION.md).

## Stopping the Daemon

```
openniri-cli stop
```

Or right-click the system tray icon and select "Exit".

## Auto-Start on Login

To start OpenNiri automatically when you log in:

```
openniri-cli autostart enable
```

To disable:

```
openniri-cli autostart disable
```

## Safe Mode

If hotkeys conflict with another app or cloaking causes visual glitches, use a stop-first safe-mode restart:

```
openniri-cli stop
openniri-cli status   # should fail with "Failed to connect..." before rerun
openniri-cli run --safe-mode
```

This disables:
- Global hotkey registration (you can still control via CLI commands)
- DWM cloaking (uses move-off-screen instead)

You can also disable these individually:

```
openniri.exe --no-hotkeys    # Disable hotkeys only
openniri.exe --no-cloak      # Disable cloaking only
```

## Troubleshooting

Common issues and solutions are documented in [TROUBLESHOOTING.md](TROUBLESHOOTING.md).

Quick checks:

1. **Windows don't tile**: Run `openniri-cli doctor` to check if the daemon is running
2. **Hotkeys don't work**: Check if another app is using the same keys; stop first, confirm stop via `openniri-cli status`, then try `--safe-mode`
3. **Visual glitches**: Try `--no-cloak` to use move-off-screen instead of DWM cloaking
4. **Need to recover from a stuck desktop**: Run `openniri-cli panic-revert` first, confirm shutdown with `openniri-cli status` (must fail), then retry with `openniri-cli run --safe-mode` if needed. If no terminal is reachable, use tray **Emergency: Uncloak All Windows** then **Exit**. Use Task Manager kill/sign-out/reboot only as last resort (can interrupt active work).

## Uninstalling

1. Stop the daemon: `openniri-cli stop`
2. Disable auto-start: `openniri-cli autostart disable`
3. Delete the binary files (`openniri.exe`, `openniri-cli.exe`)
Note: the next steps permanently delete your config and saved workspace state. Back up `%APPDATA%\\openniri\\` first if you want to keep settings.
4. Optionally, delete the config directory: `%APPDATA%\openniri\`
5. Optionally, delete saved state: `%APPDATA%\openniri\data\`

## Getting Help

If you run into issues:

1. **Run diagnostics**: `openniri-cli doctor` checks your binary, config, daemon status, and system info
2. **Collect logs**: `openniri-cli collect-logs` gathers diagnostic information in one place
3. **Check the troubleshooting guide**: [TROUBLESHOOTING.md](TROUBLESHOOTING.md) covers common problems and solutions
4. **Search existing issues**: <https://github.com/AdEx-Partners-DE/OpenNiri-Windows/issues>
5. **File a new issue**: Include your `doctor` output, relevant logs, and reproduction steps

For common failure scenarios and exact resolution steps, see the [Support Playbook](SUPPORT_PLAYBOOK.md).
