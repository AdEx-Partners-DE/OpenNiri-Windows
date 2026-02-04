# Codex Consolidated Review + Iteration Log Audit (Review 8)
Date: 2026-02-04
Reviewer: Codex
Scope: Repo state verification, `ITERATION_LOG.md` cross-check, and doc drift review
Status: Review complete. No code changes made.

## Verification Summary (Commands Run)
- `cargo test --all` -> **PASSED**: 111 passed, 2 ignored. No warnings observed.
- Breakdown: core_layout 79, daemon 11, ipc 10, platform_win32 11 (+2 ignored).
- Previous failure (E0599 `Config::generate_default`) is no longer reproducible; tests no longer reference the missing method.

## Confirmed Capabilities (Repo State)
- IPC protocol crate `crates/ipc` includes monitor focus/move and config reload commands.
- CLI sends JSON over a named pipe, supports `reload`, and provides `init` for config generation.
- Daemon hosts the named pipe server, handles IPC commands, routes WinEvent lifecycle events, and processes animation ticks.
- Config system exists at `crates/daemon/src/config.rs` with hotkey bindings and default config generation (via CLI helper).
- Multi-monitor support is present with per-monitor workspaces and monitor routing helpers in platform layer.
- Win32 platform layer includes enumeration, cloaked window filtering, DeferWindowPos positioning, WinEvent hooks, and global hotkey registration.
- Core layout engine includes smooth scrolling primitives (`Easing`, `ScrollAnimation`) and animated placement paths.

## Documentation Drift (Action Needed)
- `docs/ARCHITECTURE.md` still lists **configuration support** and **multi-monitor workspaces** as pending. Both are implemented.
- `docs/ARCHITECTURE.md` still states core layout test count as 52; current is 79.
- `docs/SPEC.md` still lists **config file support** as pending and does not describe multi-monitor behavior.
- Neither doc mentions **global hotkeys** or **smooth scroll animation** despite being implemented.
- These docs should be updated to align with Iterations 10–14 and the current codebase.

## Iteration Log Review
- Iterations 8.1–18 in `docs/1_Progress and review/ITERATION_LOG.md` align with current code and test history.
- Iterations 17–18 correctly record the temporary test failure; current run passes again.
- **Potential inconsistency**: Iteration 19 claims hotkey reload fixes and doc updates; code still shows reload does not re-register hotkeys and docs are still stale. Verify or correct the log entry.

## Gaps and Follow-ups (Based on Current Code)
- ~~**Tooling**: A stray `nul` file exists at repo root; ripgrep reports `nul: Incorrect function`. Consider removing or ignoring it.~~ **FIXED (Iteration 21)**: Stray `nul` file removed.
- `Config.layout.min_column_width` and `Config.layout.max_column_width` are defined and applied in daemon's `enumerate_and_add_windows()` via `.clamp()`. **IMPLEMENTED (Iteration 19)**
- `Config.appearance.use_cloaking` is defined but not used to select a hide strategy (currently always Cloak). **TODO**: Implement MoveOffscreen alternative.
- ~~`Config.behavior.track_focus_changes` is defined but not used to enable or disable WinEvent hooks.~~ **IMPLEMENTED (Iteration 19)**: WinEvent hooks conditionally installed based on config.
- ~~`Config.behavior.log_level` is defined but not used to configure tracing.~~ **IMPLEMENTED (Iteration 19)**: Log level parsed and applied to tracing subscriber.
- ~~`Reload` updates layout config but does **not** re-register hotkeys; config changes to hotkeys won't take effect without restart.~~ **FIXED (Iteration 19)**: Hotkey reload integrated into `IpcCommand::Reload` handler.
- IPC tests cover serialization and protocol framing, but there is still no end-to-end daemon/CLI integration test.
- Toolchain policy is split: `AGENTS.md` says MSVC, but `.cargo/config.toml` forces GNU. This should be reconciled or documented.
