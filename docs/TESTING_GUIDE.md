# Manual Testing Guide

This guide covers what to test when running OpenNiri-Windows on real hardware. Work through the scenarios in order — earlier tests validate prerequisites for later ones.

## Prerequisites

- Windows 10 (1903+) or Windows 11
- Rust GNU toolchain installed: `rustup toolchain install stable-x86_64-pc-windows-gnu`
- Release binary: `cargo +stable-x86_64-pc-windows-gnu build --release` (binaries in `target/release/`)
- At least 3 windows open (e.g., terminal, browser, editor)
- Optional: second monitor for multi-monitor tests

## Safety Checklist (Before You Test)

- Confirm no daemon is already running: `openniri-cli status` should fail with "Daemon is not running..."
- Keep one terminal visible and reachable for recovery commands
- Pin the tray icon (open hidden icons and pin OpenNiri)
- Avoid testing while elevated/admin-only windows are in active use
- `openniri-cli stop` and `openniri-cli panic-revert` target `openniri.exe` only; they do not close your other terminals, browser tabs, or editor windows by themselves
- Do not use Task Manager force-kill, sign-out, or reboot unless non-destructive recovery steps fail

## If You Lose Control During Testing

Use non-destructive recovery first:

1. Do not close other apps or terminals while first-line recovery is running.
2. `openniri-cli panic-revert`
3. `openniri-cli status` should fail before any rerun (this confirms OpenNiri is down)
4. If daemon IPC is unavailable, run `openniri-cli emergency-uncloak` (or rely on panic-revert auto local restore)
5. If no terminal is reachable, use tray **Emergency: Uncloak All Windows** and then **Exit**
6. Start safe mode only after shutdown is confirmed: `openniri-cli run --safe-mode`

## Debug Logging

Set debug logging in your `config.toml` before testing:

```toml
[behavior]
log_level = "debug"
```

Logs go to `%TEMP%\openniri-daemon.log` when launched via `openniri-cli run`.

---

## Test Scenarios

### 1. Cold Start

**Steps**: Close any running daemon. Open 3+ windows. Run `openniri-cli run`.

**Expected**:
- Tray icon appears in the system tray
- Startup banner prints to log with version, monitor count, window count, hotkey count
- All visible windows are tiled in columns on the horizontal strip
- Windows that were partially off-screen are repositioned

**Watch for**: Flicker during initial tiling (brief, expected). Windows left in wrong positions.

---

### 2. Hotkey Navigation

**Steps**: Press `Win+H` (focus left), `Win+L` (focus right), `Win+J` (focus down), `Win+K` (focus up).

**Expected**:
- Focus moves between windows/columns
- Active window border color changes (if `active_border = true` in config)
- Viewport scrolls smoothly when focus reaches edge columns
- No focus movement if already at the boundary

**Watch for**: Focus not moving. Border color not updating. Scroll animation jank.

---

### 3. Window Open/Close Lifecycle

**Steps**: Open a new window (e.g., Notepad). Then close it.

**Expected**:
- New window appears as a new column, tiled into the layout
- Closing the window removes its column
- Remaining windows re-tile to fill the space

**Watch for**: New window not being tiled. Gap left after closing.

---

### 4. Drag Resistance (Snap-Back)

**Steps**: Try to drag a tiled window by its title bar.

**Expected**:
- The window snaps back to its tiled position
- No visible jitter or flicker (the A1 feedback loop fix prevents this)
- Log shows "Suppressing MovedOrResized during apply_layout" messages

**Watch for**: Jitter, flicker, rapid log spam. If you see these, the feedback loop fix may not be working.

---

### 5. Floating Toggle

**Steps**: Focus a window. Press `Win+F` to float it. Press `Win+F` again to unfloat.

**Expected**:
- Floating: window is freed from tiling, can be moved/resized freely
- Unfloating: window returns to the tiled layout

**Watch for**: Window disappearing. Float state not persisting across focus changes.

---

### 6. Fullscreen Toggle

**Steps**: Focus a window. Press `Win+Shift+F` to enter fullscreen. Press again to exit.

**Expected**:
- Fullscreen: window fills the entire monitor work area, other windows hidden
- Exit fullscreen: normal tiling layout restores

**Watch for**: Other windows not hiding. Fullscreen window not filling the screen.

---

### 7. Config Edit + Reload

**Steps**: Change `gap = 20` in your `config.toml`. Run `openniri-cli reload`.

**Expected**:
- Layout re-applies with new gap value
- Log shows "Configuration applied to all N workspaces"

**Watch for**: Old gap persisting. Error messages in log.

---

### 8. Bad Config Recovery

**Steps**: Intentionally break your `config.toml` (e.g., remove a closing bracket). Run `openniri-cli reload`.

**Expected**:
- Daemon keeps running with the previous valid config
- Log shows a parse error message
- Fix the config and reload again — new config applies

**Watch for**: Daemon crashing on bad config.

---

### 9. Multi-Monitor (if available)

**Steps**: With 2+ monitors, press `Win+Alt+H` / `Win+Alt+L` (focus monitor left/right). Press `Win+Alt+Shift+H` / `Win+Alt+Shift+L` (move window to monitor).

**Expected**:
- Focus switches between monitors
- Window moves to the other monitor's workspace

**Watch for**: Commands being no-ops. Window disappearing during cross-monitor move.

---

### 10. Minimize / Restore

**Steps**: Minimize a tiled window from the taskbar or title bar. Restore it.

**Expected**:
- Minimize: window removed from visible layout, focus moves to next window
- Restore: window returns to layout, focus moves to restored window

**Watch for**: Focus not shifting on minimize. Restored window not appearing.

---

### 11. Fullscreen + Minimize

**Steps**: Enter fullscreen (`Win+Shift+F`). Minimize the fullscreen window.

**Expected**:
- Fullscreen mode exits
- Other windows become visible again
- Focus moves to another window

**Watch for**: Other windows staying hidden. Fullscreen state not clearing.

---

### 12. Pause / Resume via Tray

**Steps**: Right-click the tray icon. Click "Pause Tiling". Open/resize windows. Click "Resume Tiling".

**Expected**:
- Paused: windows can be freely moved/resized, tray shows "Resume Tiling"
- Resumed: all windows snap back to tiled positions, tray shows "Pause Tiling"

**Watch for**: Tray text not updating. Windows not snapping back on resume.

---

### 13. Column Resize

**Steps**: Press `Win+Ctrl+H` (decrease width) or `Win+Ctrl+L` (increase width). Also try presets: `Win+1` (1/3), `Win+2` (1/2), `Win+3` (2/3), `Win+0` (equalize).

**Expected**:
- Column width changes with snap hint overlay briefly visible
- Presets set the column to the exact viewport fraction

**Watch for**: Snap hint not appearing. Width not changing. Columns overlapping.

---

### 14. Stop and Recover

**Steps**:
1. Run `openniri-cli stop`.
2. Run `openniri-cli status` and confirm it fails with "Daemon is not running..." before any rerun.
3. If `stop` reports unconfirmed shutdown, verify that local emergency visibility restore output is present and windows are visible before proceeding.

**Expected**:
- Daemon shuts down cleanly
- All managed windows are uncloaked (made visible) — no invisible windows left behind
- Tray icon disappears
- Windows remain at their last tiled positions
- Other applications continue running (terminal sessions, browser tabs, file copies)
- `status` confirms no daemon is running before restart/safe-mode attempts

**Watch for**: Windows left invisible (cloaked). Daemon not responding to stop confirmation.

---

### 15. Safe Mode

**Steps**:
1. If daemon is already running, run `openniri-cli stop`.
2. Run `openniri-cli status` and confirm it fails with "Daemon is not running..." before rerun.
3. Run `openniri.exe --safe-mode` (or `openniri-cli run --safe-mode`).

**Expected**:
- Daemon starts without registering hotkeys and without DWM cloaking
- Startup banner shows 0 hotkeys registered
- Windows are still tiled via IPC commands (`openniri-cli focus left`, etc.)

**Watch for**: Hotkeys still working in safe mode. Cloaking still active.

---

### 16. Focus-Lockout Recovery (INC-49)

**Steps**:
1. Start OpenNiri normally: `openniri-cli run`.
2. Reproduce the focus-lockout symptom path used in incident testing (for example: local `apply` workflow that previously trapped focus).
3. Run the recommended non-destructive recovery sequence:
   - `openniri-cli panic-revert`
   - `openniri-cli status` (must fail before rerun)
   - `Win+Ctrl+Shift+B`
   - `Alt+Esc`
   - `Win+Tab` and select the target terminal/editor window
4. Explicitly verify recovery state:
   - `panic-revert` returns `OK`
   - tray icon disappears after daemon exit
   - managed windows are visible (uncloaked)
5. Start safe mode only after shutdown is confirmed: `openniri-cli run --safe-mode`.

**Pass**:
- Terminal/editor focus is recoverable without reboot, sign-out, or Explorer restart.
- `panic-revert` + `status` confirms daemon shutdown before safe-mode rerun.
- User regains normal keyboard/mouse control of previously trapped windows.
- No force-kill is needed in the primary recovery path.

**Fail**:
- Focus remains trapped after the documented sequence.
- `panic-revert` is skipped as first-line recovery action.
- Safe mode is started while prior daemon instance is still running.
- Recovery requires reboot/sign-out/Explorer restart to regain control.

---

### 17. Tray Emergency Uncloak Path (Terminal Unavailable)

**Steps**:
1. Start OpenNiri normally: `openniri-cli run`.
2. Reproduce a state where terminal interaction is unavailable or blocked.
3. Use tray emergency path exactly in this order:
   - Right-click tray icon -> **Emergency: Uncloak All Windows**
   - Right-click tray icon -> **Exit**
4. Recover any terminal and run:
   - `openniri-cli status` (must fail before rerun)
5. Optionally restart in safe mode after status confirms shutdown: `openniri-cli run --safe-mode`.

**Pass**:
- Emergency tray action immediately restores/uncloaks managed windows.
- Tray **Exit** shuts down daemon cleanly and tray icon disappears.
- `status` confirms no daemon is running.
- No Task Manager force-kill, sign-out, or reboot is required.

**Fail**:
- Emergency tray action does not restore window visibility.
- Tray exit leaves daemon running.
- Recovery requires force-kill/sign-out/reboot before tray path is attempted.

---

### 18. Long-Running File Operation Continuity During Recovery

**Steps**:
1. Start a long file operation from terminal (example: large copy via `robocopy`).
2. While the operation is still running, execute the same recommended non-destructive recovery sequence:
   - `openniri-cli panic-revert`
   - `openniri-cli status` (must fail before rerun)
   - focus recovery keys (`Win+Ctrl+Shift+B`, `Alt+Esc`, `Win+Tab`)
3. Do not use Task Manager force-kill/sign-out/reboot unless step 2 fails.
4. Optionally restart in safe mode after shutdown confirmation: `openniri-cli run --safe-mode`.

**Pass**:
- The long-running file operation keeps running or completes normally through recovery steps.
- No shell process is force-killed as part of recommended recovery.
- Explorer restart is not required for standard recovery.
- `panic-revert` is the first recovery action taken.

**Fail**:
- File operation is interrupted, canceled, or left in a broken state by recommended recovery steps.
- Recovery guidance requires Explorer restart before non-destructive steps are exhausted.
- Recovery sequence relies on force-kill before `panic-revert`.

---

### 19. Doctor Command

**Steps**: With the daemon running, run `openniri-cli doctor`.

**Expected**:
- Prints diagnostic checks: daemon reachable, config valid, hotkeys registered, etc.
- All checks pass on a healthy setup

**Watch for**: False failures. Missing checks.

---

## Known Limitations

- Elevated (admin) windows cannot be managed by a non-elevated daemon
- Virtual desktops (Win+Ctrl+D) are not integrated
- Exclusive-fullscreen games may conflict — use window rules to ignore them
- Some UWP/Store apps use non-standard window classes
- Brief flicker (~100ms) may occur during initial window tiling

---

## Host Smoke Test

A quick end-to-end verification you can run on any Windows desktop. Completes in under 2 minutes.

### Prerequisites

- Release binaries built: `cargo +stable-x86_64-pc-windows-gnu build --release`
- No existing daemon running (check with `openniri-cli status`)

### Steps

```powershell
# 1. Start the daemon
openniri-cli run
# Expected: "Started openniri daemon (PID ...)" and tray icon appears

# 2. Verify daemon is healthy
openniri-cli doctor
# Expected: All checks [PASS]

# 3. Open a test window
Start-Process notepad
Start-Sleep -Seconds 1

# 4. Verify the window was tiled
openniri-cli query all
# Expected: Notepad appears in the managed window list

# 5. Toggle floating
openniri-cli toggle-floating
# Expected: Notepad becomes freely movable

# 6. Toggle floating back
openniri-cli toggle-floating
# Expected: Notepad snaps back into tiled layout

# 7. Close the test window
openniri-cli close-window
Start-Sleep -Milliseconds 500

# 8. Verify window removed
openniri-cli query all
# Expected: Notepad no longer in the list

# 9. Stop the daemon
openniri-cli stop
# Expected: "OK", tray icon disappears, all windows uncloaked
```

### What to look for

- **Tray icon**: Appears on start, disappears on stop
- **No invisible windows**: After stop, all windows should be visible
- **No crashes**: Daemon log at `%TEMP%\openniri-daemon.log` should have no PANIC lines
- **Clean exit**: `openniri-cli status` should fail after stop (daemon gone)

### Running manually (without PowerShell)

If you prefer manual testing: start the daemon, open Notepad, press `Win+H` / `Win+L` to navigate, press `Win+F` to toggle floating, then run `openniri-cli stop` from a terminal.

---

## Reporting Issues

1. Set `log_level = "debug"` and reproduce the issue
2. Collect logs from `%TEMP%\openniri-daemon.log`
3. Run `openniri-cli status` and note the output
4. Open an issue at https://github.com/AdEx-Partners-DE/OpenNiri-Windows/issues using the bug report template
