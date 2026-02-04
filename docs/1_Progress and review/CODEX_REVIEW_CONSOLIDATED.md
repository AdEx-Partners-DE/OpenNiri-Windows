# Codex Consolidated Review + Iteration Log Audit (Review 4)
Date: 2026-02-04
Reviewer: Codex
Scope: Repo state verification, `ITERATION_LOG.md` cross-check, and doc drift review
Status: Review complete. No code changes made.

## Verification Summary (Commands Run)
- `cargo test --all` -> Result: 108 passed, 2 ignored. Warnings: `AppState::monitors_list` unused, `Config::default_config_path` unused.

## Confirmed Capabilities (Repo State)
- IPC protocol crate `crates/ipc` includes monitor focus/move and config reload commands.
- CLI sends JSON over a named pipe, supports `reload`, and provides `init` for config generation.
- Daemon hosts the named pipe server, handles IPC commands, routes WinEvent lifecycle events, and processes animation ticks.
- Config system exists at `crates/daemon/src/config.rs` with hotkey bindings and default config generation.
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
- Iterations 8.1–14 in `docs/1_Progress and review/ITERATION_LOG.md` align with current code and tests.
- Test totals in the log match `cargo test --all` output (108 passed, 2 ignored).
- The log’s claims about hotkeys and animation features are confirmed by code and tests.

## Gaps and Follow-ups (Based on Current Code)
- `Config.layout.min_column_width` and `Config.layout.max_column_width` are defined but not applied in daemon.
- `Config.appearance.use_cloaking` is defined but not used to select a hide strategy (currently always Cloak).
- `Config.behavior.track_focus_changes` is defined but not used to enable or disable WinEvent hooks.
- `Config.behavior.log_level` is defined but not used to configure tracing.
- `Reload` updates layout config but does **not** re-register hotkeys; config changes to hotkeys won’t take effect without restart.
- IPC tests cover serialization and protocol framing, but there is still no end-to-end daemon/CLI integration test.
- Toolchain policy is split: `AGENTS.md` says MSVC, but `.cargo/config.toml` forces GNU. This should be reconciled or documented.
- Warnings from `cargo test --all` indicate unused helpers and should be cleaned up or explicitly justified.
