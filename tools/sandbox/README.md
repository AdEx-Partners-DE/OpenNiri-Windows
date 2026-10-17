# Sandbox end-to-end test

Runs the real daemon against real windows inside a disposable Windows Sandbox, so desktop-control behaviour (tiling, hiding, focus, recovery) can be tested without risking the desktop you work on. This is the automated part of the INC-49 host acceptance.

## Requirements

- Windows 11 24H2 or later with the **Windows Sandbox** feature enabled (`wsb.exe` on `PATH`)
- Release binaries: `cargo build --release --workspace`

## Run

```powershell
cargo build --release --workspace
powershell -ExecutionPolicy Bypass -File tools/sandbox/run-sandbox-e2e.ps1
```

Exit code `0` means every check passed, `1` means at least one failed and `2` means a harness error. Evidence goes to `target/sandbox-e2e/<timestamp>/out/`:

- `results.json`
- `screen-*.png` and `windows-*.json` for every step
- daemon stdout/stderr per phase
- `cli.log`
- `activations.log`

Use `-KeepSandbox` to leave the sandbox open for inspection.

## What it checks

The sandbox runs with networking, clipboard, audio, video and printer redirection disabled. Binaries are mapped read-only. Infrastructure steps (daemon start/stop, preconditions) only show up when they fail.

| Phase | What is checked |
|---|---|
| A (defaults) | layout at startup: all windows managed and tiled, 10 px visible gaps, windows outside the viewport cloaked in place; IPC pipe ACL limited to the user and SYSTEM; the Alt+H/Alt+L hotkey walk focuses every window and each is visible; a click activates every visible window and focus stays (lockout check); CLI scenario; after `stop` every window is visible; daemon log has no uncloak errors, failed passes or panics |
| B (mouse hooks on) | cursor latency p95 < 50 ms; hovering focuses windows and focus stays with a stationary cursor; leaving for an unmanaged area before the debounce delay preserves focus |
| C (crash) | hard kill while windows are hidden, then `emergency-uncloak`: every window is visible |
| D (restart) | hard kill while windows are cloaked, then a plain daemon restart recovers and manages every window |
| F (startup flags) | `--safe-mode`, `--no-hotkeys`, and `--no-cloak` survive repeated reloads; real OS hotkey registration and hiding strategy match the flags; stop restores every window |
| G (reload shortcuts) | a reload hotkey replaces its own registration; the new binding executes its command; invalid TOML preserves the previous binding and command |
| E (Explorer restart, run last) | after Explorer restarts, shell cloaking still works, and every window is visible after stop |

Run it for changes to `platform_win32` or the daemon's window handling, and before tagging a release.

`guest-e2e.ps1` runs inside the sandbox. Never run it on a real desktop.
