# Codex Consolidated Review + Iteration Log Audit (Review 15)
Date: 2026-02-04
Reviewer: Codex
Scope: Repo state verification, Claude 8.x output validation, doc drift review, test quality/coverage scan, project progress update
Status: Review complete. No code changes made.

## Verification Summary (Commands Run)
- `cargo test --all` -> **PASSED**: 202 passed, 2 ignored (plus 1 ignored doc-test).
- Breakdown: cli 28, core_layout 87, daemon 44, daemon integration 17, ipc 13, platform_win32 13 (+2 ignored). Doc-tests: platform_win32 1 ignored.

## Corrections vs Prior Review Notes
- Prior review claimed CLI had 0 tests; current CLI has **28** unit tests.
- Integration tests now exist under `crates/daemon/tests/integration.rs`, but they validate IPC serialization/protocol only (not end-to-end daemon/CLI behavior).

## Test Quality & Coverage Assessment
**Strengths**
- `core_layout` has broad unit coverage across layout invariants, focus behavior, scrolling/animation, and floating windows.
- `ipc` tests cover JSON roundtrips for all commands/responses plus protocol framing and invalid JSON handling.
- `daemon` config tests exercise defaults, partial TOML parsing, bounds clamping, command parsing, and window rule matching.
- `cli` tests cover config generation, IPC command mapping, and timeout defaults.
- `daemon` integration tests provide extra IPC protocol validation (line-delimited framing and edge values).

**Gaps / Coverage Risks**
- **No end-to-end tests** across CLI <-> IPC <-> daemon <-> Win32 placement. Current “integration” tests do not run the daemon or named pipes.
- **OS-bound behavior untested**: WinEvent hooks, hotkeys, named pipe server handling, overlay rendering, tray callbacks, and gesture detection rely on runtime/manual validation.
- **Error-path coverage is thin**: no tests for failed hook install, failed hotkey registration, pipe disconnects/partial reads, or reload failures.
- **Multi-monitor behavior** only validated in pure helper functions; no end-to-end monitor switching tests.
- No coverage instrumentation (line/branch); only test counts are available.

## Recommended Missing Tests (Priority Order)
- **P0**: End-to-end IPC test (spawn daemon, send CLI command via named pipe, verify response and workspace mutation).
- **P0**: CLI behavior when daemon is absent or pipe is unavailable (connect failure, timeout).
- **P1**: Daemon reload test: modify config, trigger reload, verify hotkeys + layout config updated.
- **P1**: Multi-monitor focus/move commands using synthetic monitor sets.
- **P2**: Win32 hook registration failure path (simulate/force error; ensure graceful fallback).
- **P2**: Gesture mapping sanity (config -> command mapping) without needing actual touchpad input.

## Project Progress Update (Current Snapshot)
**Phases Completed (Based on Repo Evidence)**
- **Foundation & Layout**: Core layout engine, invariants, and large unit test suite are in place (87 tests).
- **Platform + IPC Integration**: Win32 enumeration/placement/cloaking, IPC protocol crate, named pipe server/client, async daemon loop are implemented.
- **Config + UX Features (Partial)**: Config load/reload, global hotkeys, multi-monitor logic, smooth scrolling, overlay snap hints, tray wiring, window rules, and gesture scaffolding exist.
- **Test Expansion (Partial)**: CLI unit tests and daemon IPC integration tests have been added, but end-to-end coverage is still missing.

**Requirements & Specs Status (Reality vs Docs)**
- Core requirements in `docs/SPEC.md` appear **mostly implemented**, but the spec is **stale** and missing several now‑implemented features (overlay hints, tray, QueryAllWindows, gesture plumbing, window rules).
- `docs/ARCHITECTURE.md` reflects an older snapshot and outdated test counts; it does not describe the current IPC and feature surface accurately.
- Net: code appears **ahead of the docs**, so the written requirements/specs need a refresh to match actual behavior.

**Overall Accomplishment / Achievement**
- The project has moved beyond “layout-only” into a functioning vertical slice: **CLI → IPC → daemon → layout → Win32 placement** exists in code.
- The main remaining risk is **system-level validation** (runtime behavior on real Windows desktops) and lack of **end-to-end tests**.

## Claude Output Verification (8.1, 8.2, 8.3)
**Confirmed Accurate (Still True in Repo)**
- Win32 enumeration, DeferWindowPos batching, and DWM cloaking are implemented in `platform_win32`.
- IPC crate (`openniri-ipc`) exists with named pipe `PIPE_NAME`, command/response enums, and serialization tests.
- Daemon uses tokio named pipes and async event loop; CLI connects via named pipes.
- Monitor detection functions `enumerate_monitors()` and `get_primary_monitor()` exist (hardware-dependent tests ignored).

**Stale or Superseded Statements**
- Test counts cited in 8.1-8.3 (52/60) are outdated; current suite is 202.
- 8.1/8.2 status statements about "no IPC" or "CLI prints intended commands" are now false; IPC is implemented.
- 8.3 claims release build success and environment fixes; not verified in this review run.

**Doc Update Claims vs Current Docs**
- 8.1 claims ARCHITECTURE/SPEC updates; those sections exist, but they are now stale.
- ARCHITECTURE/SPEC still show old test counts (79/10/11) and list gestures or floating/rules as pending.
- Specs do not mention overlay snap hints, tray integration, QueryAllWindows, gesture plumbing, or window rules.

## Documentation Drift (Action Needed)
- `docs/ARCHITECTURE.md` and `docs/SPEC.md` lag current implementation.
- Test counts in both docs are stale (79/10/11 vs actual 87/13/44/13 plus CLI 28 and daemon integration 17).
- `SPEC.md` still lists gestures and per-window floating/rules as pending.
- Neither doc mentions overlay snap hints, tray integration, QueryAllWindows, gesture plumbing, or window rules.
- Per instruction, no doc edits were made.

## Iteration Log Review (Read-only)
- `docs/1_Progress and review/ITERATION_LOG.md` still claims Iterations 21-22 tests at 131/147; current suite is 202. Needs correction or justification.

## Gaps and Follow-ups (Based on Current Code)
- `Config.appearance.use_cloaking` is defined but not used; TODO remains for alternate hide strategy.
- No end-to-end integration test for CLI <-> IPC <-> daemon <-> Win32 layout application.
- Toolchain guidance conflict: `AGENTS.md` recommends MSVC but `.cargo/config.toml` forces GNU target.
- Stray `nul` file at repo root breaks `rg` in some shells.
