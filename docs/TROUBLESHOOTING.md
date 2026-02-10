# Troubleshooting Guide

This guide covers the most common issues when running OpenNiri-Windows and how to resolve them.

---

## 1. Checking if the Daemon is Running

Use the CLI `status` command to query the running daemon:

```
openniri-cli status
```

If the daemon is running you will see output like:

```
OpenNiri Daemon Status:
  Version: 0.1.0
  Monitors: 2
  Total windows: 7
  Uptime: 1h 23m 45s
```

If the daemon is **not** running you will see:

```
Error: Failed to connect to daemon. Is openniri running?
```

To start the daemon (and apply the layout once it is ready):

```
openniri-cli status   # if this fails, no daemon is running
openniri-cli run
```

For restarts (including switching to safe mode), always stop first and confirm stop completion before rerunning:

```
openniri-cli stop
openniri-cli status   # must fail with "Failed to connect..." before any new `run`
```

---

## 2. Log Location

**When launched via `openniri-cli run`**, the CLI redirects daemon output to temporary log files:

| Stream | File |
|--------|------|
| stdout | `%TEMP%\openniri-daemon.log` |
| stderr | `%TEMP%\openniri-daemon.err.log` |

On most Windows installations `%TEMP%` resolves to `C:\Users\<you>\AppData\Local\Temp`.

**When launched directly** (e.g. `openniri.exe` from a terminal), logs go to stderr in the current terminal window.

### Configuring the log level

In your `config.toml`, the `[behavior]` section accepts a `log_level` key:

```toml
[behavior]
log_level = "debug"   # trace | debug | info | warn | error
```

The default level is `info`. Set it to `debug` or `trace` for more detailed output when diagnosing issues. After changing the value, reload the configuration (`openniri-cli reload`) or restart the daemon.

### Redirecting logs manually

If you launch the daemon binary directly and want to save logs to a file:

```powershell
openniri.exe 2> openniri.log
```

---

## 3. Config File Location

OpenNiri looks for configuration in the following locations, in priority order:

| Priority | Path |
|----------|------|
| 1 | `%APPDATA%\openniri\config\config.toml` |
| 2 | `~\.config\openniri\config.toml` |
| 3 | `.\config.toml` (current working directory) |

On a typical Windows install, priority 1 resolves to:

```
C:\Users\<you>\AppData\Roaming\openniri\config\config.toml
```

If no config file is found in any of these locations, the daemon starts with built-in defaults.

### Generating a default config

```
openniri-cli init
```

This writes a fully commented default `config.toml` to the standard location. Use `--output <path>` to write it elsewhere, or `--force` to overwrite an existing file.

---

## 4. Common Issues

### Desktop feels stuck after apply/reload

1. **Run panic revert first.**

   ```
   openniri-cli panic-revert
   ```

   This sends `panic_revert` to restore/uncloak managed windows and exit the daemon cleanly.

2. **Confirm shutdown before any rerun.**

   ```
   openniri-cli status
   ```

   `status` must fail with "Failed to connect..." before any new `run`.

3. **If terminal input is blocked but tray is reachable, use tray emergency action.** Right-click the OpenNiri tray icon, click **Emergency: Uncloak All Windows**, then click **Exit**.

4. **Attempt focus recovery keys.** Try `Win+Ctrl+Shift+B`, `Alt+Esc`, then `Win+Tab` and click the target terminal/editor window.

5. **Use force-kill/sign-out/reboot only as last resort.** If tray + CLI recovery are unavailable, end `openniri.exe` from Task Manager (`Ctrl+Shift+Esc`). Use sign-out/reboot only if the desktop remains unrecoverable. Warn users that these steps can interrupt active shell/file operations.

### Hotkeys don't work

1. **Check for conflicts.** Other applications (PowerToys, AutoHotkey, display drivers, etc.) may have registered the same key combinations. The default bindings use `Win+H/J/K/L` and several `Win+Shift` / `Win+Ctrl` chords. Try temporarily disabling other hotkey tools to isolate the conflict.

2. **Verify config syntax.** Open your `config.toml` and confirm the `[hotkeys]` section uses the correct format:

   ```toml
   [hotkeys]
   "Win+H" = "focus_left"
   "Win+Shift+Q" = "close_window"
   ```

   Key names are case-insensitive. Valid modifiers are `Win`, `Ctrl`, `Alt`, and `Shift`. They are joined with `+`.

3. **Check the log.** Set `log_level = "debug"` and look for lines about hotkey registration failures at startup.

4. **Reload after editing.** Config changes are not picked up automatically. Run `openniri-cli reload` or use the tray menu "Reload Config" entry.

### Windows aren't being tiled

1. **Check if tiling is paused.** Right-click the OpenNiri system tray icon. If the menu says "Resume Tiling", tiling is currently paused. Click it to resume. You can also confirm via:

   ```
   openniri-cli status
   ```

2. **Check window rules.** If you have `[[window_rules]]` entries in your config, a rule with `action = "float"` or `action = "ignore"` may be matching the window. Temporarily remove your window rules and reload to test.

3. **Check for elevated windows.** Windows launched as Administrator (e.g. Task Manager, an elevated terminal) cannot be managed by a non-elevated OpenNiri process. See the "Known Limitations" section below for details.

4. **Verify the window is not minimized.** Minimized windows remain tracked but are excluded from placement calculations until they are restored.

### Second daemon won't start

If you see:

```
Error: Another openniri-daemon instance is already running.
Use 'openniri-cli status' to check the running instance.
```

Another instance of `openniri.exe` is already listening on the IPC named pipe (`\\.\pipe\openniri`). Only one daemon can run at a time. Either use the existing instance or stop it first:

```
openniri-cli stop
openniri-cli status   # must fail before rerun
```

Then start a new one:

```
openniri-cli run
```

### Config changes not applied

The daemon does **not** watch the config file for changes. After editing `config.toml` you must explicitly reload:

- **CLI:** `openniri-cli reload`
- **Tray menu:** Right-click the system tray icon and select "Reload Config"

The daemon will re-read the config file, recompile window rules, and re-register hotkeys. Check the log for any validation warnings after reloading.

### Windows flash or flicker on startup

When the daemon first tiles windows it may briefly show and reposition them, causing a visible flash lasting roughly 100 ms. This is expected behavior related to DWM cloaking timing. If `use_cloaking = true` (the default), off-screen windows are hidden via DWM cloak rather than moved off-screen, which minimizes the effect but does not eliminate it entirely.

---

## 5. Resetting to Defaults

To return to the default configuration:

1. Stop the daemon:

   ```
   openniri-cli stop
   ```

2. Confirm stop completed:

   ```
   openniri-cli status   # must fail before rerun
   ```

3. Delete or rename the config file:

   ```powershell
   Rename-Item "$env:APPDATA\openniri\config\config.toml" "config.toml.bak"
   ```

4. Optionally regenerate a fresh default config:

   ```
   openniri-cli init
   ```

5. Start the daemon again:

   ```
   openniri-cli run
   ```

---

## 6. Antivirus and Security Software

OpenNiri uses Win32 APIs that some antivirus or endpoint detection (EDR) products may flag:

| API | Purpose | Why it's flagged |
|-----|---------|------------------|
| `SetWindowsHookEx` (WH_MOUSE_LL) | Touchpad gesture detection | Low-level mouse hooks are also used by keyloggers |
| `RegisterHotKey` | Global keyboard shortcuts | Global input interception |
| `DwmSetWindowAttribute` | Active window border color | DWM manipulation |
| `SetWindowPos` / `DeferWindowPos` | Tiling layout | Repositioning other process windows |
| Named pipe (`\\.\pipe\openniri`) | CLI-to-daemon IPC | Local IPC (no network) |

**OpenNiri has no network access, telemetry, or data collection.** All communication is local via named pipes. The daemon only interacts with windows on the local desktop.

### Adding exclusions

If your antivirus blocks OpenNiri:

1. Add `openniri.exe` and `openniri-cli.exe` to the exclusion list
2. For Windows Defender: Settings > Virus & threat protection > Exclusions > Add folder exclusion for the OpenNiri install directory
3. For enterprise EDR (CrowdStrike, SentinelOne, etc.): ask your IT team to whitelist the executables by path or code-signing hash

### Verifying the binary

Binaries published on GitHub Releases are built by GitHub Actions CI in a clean environment. You can verify by:

1. Checking the CI workflow run linked to each release tag
2. Building from source yourself: `cargo build --release`
3. Comparing SHA-256 hashes of release artifacts

---

## 7. Known Limitations

- **Elevated (admin) windows.** OpenNiri cannot reposition or tile windows that belong to processes running with administrator privileges unless the daemon itself is also running elevated. This is a Windows security boundary. To manage elevated windows, launch `openniri.exe` from an Administrator terminal or shortcut.

- **Virtual desktop integration.** Windows virtual desktops (Win+Ctrl+D) are not currently supported. OpenNiri manages windows on the active desktop only. Switching desktops may cause unexpected layout behavior.

- **Fullscreen games.** Exclusive-fullscreen applications may conflict with tiling. If a game steals focus or resizes unexpectedly, consider adding a window rule to float or ignore it:

  ```toml
  [[window_rules]]
  match_executable = "game.exe"
  action = "ignore"
  ```

- **UWP / Microsoft Store apps.** Some UWP applications use non-standard window classes and may not respond consistently to repositioning. If a Store app misbehaves, try adding a window rule to float it.

- **Single-monitor multi-monitor commands.** Commands like `focus-monitor left/right` and `move-to-monitor left/right` are no-ops when only one monitor is connected.

---

## 8. Fallback Behavior

OpenNiri is designed to degrade gracefully. Here's what happens when operations fail:

| Situation | What happens | Log message |
|-----------|-------------|-------------|
| `SetWindowPos` fails | Warning logged, window stays at previous position | `"Failed to apply layout..."` |
| DWM cloak fails | Warning logged, window stays visible | `"Failed to cloak/uncloak..."` |
| Access denied (elevated window) | Window is skipped during enumeration | `"Ignoring window..."` |
| Hotkey registration fails | Warning logged, hotkey count reflects partial | `"Failed to register hotkey..."` |
| Config file invalid TOML | Previous config retained, error logged | `"Failed to load configuration..."` |
| Config value out of range | Value clamped to valid range, warning logged | `"Config: ... clamped to ..."` |
| Monitor disconnected | Windows migrate to primary, workspace removed | `"Monitor removed..."` |
| Named pipe busy | CLI retries for up to 5 seconds | `"Failed to connect..."` |
| Panic (crash) | All windows uncloaked, crash report written to `%TEMP%` | `"PANIC detected..."` |
| IPC read timeout | Connection dropped, client gets timeout error | `"Timed out..."` |

### Crash Reports

If the daemon panics, a crash report is written to `%TEMP%\openniri-crash-<timestamp>.txt` containing:
- Panic message and source location
- Stack backtrace (if `RUST_BACKTRACE=1` is set)
- Daemon version

All managed windows are uncloaked (made visible) before the crash report is written.

---

## 9. Getting Help

If your issue is not covered above, please open an issue on GitHub:

<https://github.com/AdEx-Partners-DE/OpenNiri-Windows/issues>

When filing an issue, include:

1. The output of `openniri-cli status` (or the connection error if the daemon is not running).
2. Your `config.toml` (redact any paths you consider private).
3. Relevant log output (set `log_level = "debug"` to capture detail).
4. Your Windows version (`winver` output) and whether you are running as a standard user or administrator.
