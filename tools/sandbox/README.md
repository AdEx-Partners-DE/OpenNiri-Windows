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

The sandbox runs with networking, clipboard, audio, video and printer redirection disabled. Binaries are mapped read-only.

| Phase | Settings | Checks |
|---|---|---|
| A-default | default config | daemon starts and manages six test windows; windows outside the viewport are hidden; an `Alt+H`/`Alt+L` hotkey walk focuses every window, and each is visible when focused; clicking each visible window activates it and focus stays (no focus stealing); CLI scenario (`apply`, `focus`, `scroll`, `toggle-pause`, `equalize-widths`, `refresh`); after `stop`, all windows are on screen, uncloaked and clickable; no uncloak error spam or failed layout passes in the log |
| B-hooks | focus-follows-mouse and gestures on | p95 cursor latency below 50 ms with the low-level mouse hooks installed; focus follows the mouse; clean stop |
| C-crash | default config | daemon is killed with `Stop-Process -Force` while windows are hidden; `openniri-cli emergency-uncloak` brings every window back on screen |

`guest-e2e.ps1` runs inside the sandbox. Never run it on a real desktop.
