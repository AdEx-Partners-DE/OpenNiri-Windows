# Support Playbook

Quick-reference for diagnosing and resolving the most common OpenNiri-Windows issues.

---

## Critical: Desktop Becomes Unusable After `apply`

**Symptoms**: User cannot reliably return to terminal/app windows (even if thumbnails are visible), Alt+Tab focus recovery fails, shell feels stuck.

**Non-destructive recovery first**:
1. Run panic revert first:
   - `openniri-cli panic-revert` (alias: `openniri-cli recover`; sends `panic_revert` to restore/uncloak managed windows and exit cleanly)
   - If terminal control still works and user wants a temporary freeze first, run `openniri-cli toggle-pause` (alias: `openniri-cli pause`).
   - `panic-revert` and `stop` are scoped to OpenNiri (`openniri.exe`); they are not intended to close unrelated user apps.
2. Verify daemon is down before any rerun:
   - `openniri-cli status` must fail (timeout OR "Daemon is not running...")
   - If `openniri-cli stop` reports "shutdown was not confirmed", local emergency visibility restore has already been triggered; do not rerun yet, treat it as unconfirmed, and continue recovery checks.
3. If CLI is blocked but the tray is reachable, use tray action **Emergency: Uncloak All Windows**.
   - CLI equivalent alias: `openniri-cli restore-windows`.
   - If CLI does not return within ~10 seconds, switch to tray recovery.
   - If tray icon is missing, open the Windows "hidden icons" panel and pin OpenNiri.
4. Attempt focus recovery keys:
   - `Win+Ctrl+Escape` (emergency visibility restore + panic-revert)
   - `Win+Ctrl+Shift+B`
   - `Alt+Esc`
   - `Win+Tab` then click target window
5. Start safe mode only after step 2 succeeds:
   - `openniri-cli run --safe-mode`

**Expected recovery outcomes**:
- `panic-revert` is always attempted first in stuck-state incidents.
- After step 2, `status` fails (timeout OR "Daemon is not running...") before any rerun.
- Terminal/editor focus is restored without Explorer restart in the normal recovery path.
- If safe mode is retried, it is started only after step 2 succeeds.

**Emergency fallback (terminal inaccessible)**:
1. If the tray is reachable, click **Emergency: Uncloak All Windows** first, then click **Exit**.
2. Re-run `openniri-cli status` from any recovered terminal; it must fail (timeout OR "Daemon is not running...").

**Last resort only (risk of interrupting active work)**:
1. If tray + CLI recovery paths are unavailable, open Task Manager (`Ctrl+Shift+Esc`) and end `openniri.exe`.
2. If no terminal can be recovered at all, use `Ctrl+Alt+Del` and sign out or reboot.

**Important warning**:
- Do **not** use force-kill/sign-out/reboot until non-destructive steps above are exhausted.
- Force-kill, sign-out, and reboot can interrupt shell-bound workflows and long-running file operations.

---

## Hotkeys Don't Work

**Symptoms**: Pressing Win+H/L/J/K does nothing.

**Steps**:
1. Check if the daemon is running: `openniri-cli status`
2. Check for hotkey registration failures in the log — look for "Failed to register hotkey"
3. Check for conflicts with other apps (PowerToys, AutoHotkey, display drivers, game overlays). Temporarily close them and reload: `openniri-cli reload`
4. If running as a non-admin user and the focused app is elevated (Admin), hotkeys may not reach the daemon. Try launching the daemon elevated (not recommended for daily use).
5. Safe mode fallback: stop the daemon, confirm shutdown with `openniri-cli status` (must fail: timeout OR "Daemon is not running..."), then restart in safe mode via `openniri-cli run --safe-mode` and control via CLI commands.

---

## Windows Are Invisible / Missing

**Symptoms**: Windows disappear after starting the daemon, or after a crash.

**Steps**:
1. If tray is reachable, run **Emergency: Uncloak All Windows** immediately.
2. Check if cloaking is active: `openniri-cli status` — look for window count.
3. Stop the daemon: `openniri-cli stop`.
4. Confirm stop completion: `openniri-cli status` must fail (timeout OR "Daemon is not running...").
5. If the daemon crashed, run Alt+Tab to check if windows are still there but cloaked.
6. Disable cloaking in config:
   ```toml
   [appearance]
   use_cloaking = false
   ```
7. Restart: `openniri-cli run`
8. If windows are still missing, they may have been moved off-screen. Use Win+Shift+Left/Right (Windows snap) to bring them back.

---

## Daemon Won't Start

**Symptoms**: `openniri-cli run` fails or the daemon exits immediately.

**Steps**:
1. Run diagnostics: `openniri-cli doctor`
2. Check if another instance is running: `openniri-cli status`
3. If the pipe is stale (daemon crashed without cleanup), try connecting: `openniri-cli status`. If it times out, the pipe will be released.
4. Check for a crash report in `%TEMP%\openniri-crash-*.txt`
5. Check the error log: `%TEMP%\openniri-daemon.err.log`
6. Try launching directly in a terminal to see stderr: `openniri.exe`

---

## Config Not Loading

**Symptoms**: Config changes have no effect after reload.

**Steps**:
1. Validate TOML syntax: open the file in a TOML-aware editor or run `openniri-cli doctor`
2. Verify you're editing the right file: check `openniri-cli doctor` output for the config path being used
3. Reload after editing: `openniri-cli reload`
4. Check the log for validation warnings: "Config: ... clamped to ..." or "Failed to load configuration"
5. To reset to defaults: `openniri-cli config reset`

---

## Windows Not Tiling

**Symptoms**: Some or all windows are not being arranged into columns.

**Steps**:
1. Check if tiling is paused: right-click tray icon, or `openniri-cli status`
2. Check window rules: a rule with `action = "float"` or `action = "ignore"` may match. Temporarily remove `[[window_rules]]` entries and reload.
3. Refresh the window list: `openniri-cli refresh` — this re-enumerates all windows.
4. Check if the window is elevated (Admin). Non-elevated daemon cannot manage admin windows.
5. Check if the window's class is in the exclude list (UWP, shell windows). See COMPATIBILITY.md.

---

## High CPU Usage

**Symptoms**: `openniri.exe` consuming excessive CPU.

**Steps**:
1. Check if scroll animations are stuck: try `openniri-cli refresh` to reset state.
2. Disable gesture detection if not needed:
   ```toml
   [gestures]
   enabled = false
   ```
3. Disable focus-follows-mouse if not needed:
   ```toml
   [behavior]
   focus_follows_mouse = false
   ```
4. Check the log for rapid event loops (debug level): look for repeated "MovedOrResized" or "Focused" messages.
5. Restart the daemon with stop check:
   - `openniri-cli stop`
   - `openniri-cli status` must fail (timeout OR "Daemon is not running...")
   - `openniri-cli run`

---

## Second Monitor Not Detected

**Symptoms**: Windows only tile on one monitor.

**Steps**:
1. Check `openniri-cli status` for monitor count.
2. Run `openniri-cli refresh` to re-enumerate monitors.
3. If using a DisplayLink adapter, wait a few seconds after connecting for the driver to register the display.
4. Check Windows display settings to confirm the monitor is detected by the OS.
5. Restart the daemon — monitor detection runs at startup.

---

## Crash Recovery

**Symptoms**: The daemon crashed unexpectedly.

**Steps**:
1. Check for crash report: look in `%TEMP%` for `openniri-crash-*.txt`
2. All windows should be uncloaked automatically on crash. If not, use Alt+Tab or Win+D.
3. Review the crash report for the panic message and backtrace.
4. Restart: `openniri-cli run`
5. If the crash is reproducible, file a bug report with the crash report attached.

---

## Filing a Bug Report

Before filing:

1. Run `openniri-cli doctor` and copy the output
2. Run `openniri-cli collect-logs` to gather diagnostic info
3. Search [existing issues](https://github.com/AdEx-Partners-DE/OpenNiri-Windows/issues) for duplicates
4. File a new issue with: doctor output, collect-logs output, reproduction steps, and your config (redact private paths)
