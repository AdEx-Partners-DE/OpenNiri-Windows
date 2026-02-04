# Codex Consolidated Review + Iteration Log Audit (Review 13)
Date: 2026-02-04
Reviewer: Claude Opus 4.5
Scope: Post-Iteration 22 verification, quality/robustness improvements, test coverage update
Status: Review complete. Iterations 21-22 committed and pushed to GitHub.

## Verification Summary (Commands Run)
- `cargo test --all` -> **PASSED**: 147 passed, 2 ignored (plus 1 ignored doc-test). CLI has 0 tests.
- Breakdown: core_layout 87, daemon 34, ipc 13, platform_win32 13 (+2 ignored). Doc-tests: platform_win32 1 ignored.

## Test Quality & Coverage Assessment
**Strengths**
- `core_layout` has broad unit coverage across layout invariants, focus behavior, scrolling/animation, and floating windows.
- `ipc` tests cover JSON roundtrips for all commands/responses plus protocol framing and invalid JSON handling.
- `daemon` config tests exercise defaults, partial TOML parsing, bounds clamping, and command parsing.

**Gaps / Coverage Risks**
- **No integration tests** across CLI <-> IPC <-> daemon (CLI has 0 tests). This is the largest blind spot.
- **OS-bound behavior untested**: WinEvent hooks, hotkeys, named pipe server handling, overlay rendering, tray callbacks, and gesture detection rely on runtime/manual validation.
- **Error-path coverage is thin**: no tests for failed hook install, failed hotkey registration, bad pipe reads, or reload failures.
- **Multi-monitor behavior** only validated in pure helper functions; no end-to-end monitor switching tests.
- No coverage instrumentation (line/branch); only test counts are available.

## Recommended Missing Tests (Priority Order)
- **P0**: End-to-end IPC test (spawn daemon, send CLI command, verify response and workspace mutation).
- **P0**: CLI error handling when daemon is absent (connect failure, timeout).
- **P1**: Daemon reload test: modify config, trigger reload, verify hotkeys + layout config updated.
- ~~**P1**: Window rule precedence (multiple matching rules) and floating dimension fallback rules.~~ **DONE** (Iter 22)
- **P1**: Multi-monitor focus/move commands using synthetic monitor sets.
- **P2**: Win32 hook registration failure path (simulate/force error; ensure graceful fallback).
- **P2**: Gesture mapping sanity (config -> command mapping) without needing actual touchpad input.

### Tests Added in Iteration 22
- `test_app_state_new`, `test_app_state_focused_viewport`, `test_app_state_no_monitors_fallback`
- `test_window_rule_matching_class`, `test_window_rule_matching_title`, `test_window_rule_matching_executable`
- `test_window_rule_no_match_defaults_to_tile`
- `test_floating_rect_uses_rule_dimensions`, `test_floating_rect_preserves_original_if_no_dimensions`
- `test_find_window_workspace_not_found`, `test_app_state_apply_config`
- IPC tests for `QueryAllWindows`, `WindowInfo`, `WindowList`

## Claude Output Verification (8.1, 8.2, 8.3)
**Confirmed Accurate (Still True in Repo)**
- Win32 enumeration, DeferWindowPos batching, and DWM cloaking are implemented in `platform_win32`.
- IPC crate (`openniri-ipc`) exists with named pipe `PIPE_NAME`, command/response enums, and serialization tests.
- Daemon uses tokio named pipes and async event loop; CLI connects via named pipes.
- Monitor detection functions `enumerate_monitors()` and `get_primary_monitor()` exist (hardware-dependent tests ignored).

**Stale or Superseded Statements**
- Test counts cited in 8.1-8.3 (52/60) are outdated; current suite is 147.
- 8.1/8.2 status statements about "no IPC" or "CLI prints intended commands" are now false; IPC is implemented.
- 8.3 claims release build success and environment fixes; verified working in Iteration 22.

**Doc Update Claims vs Current Docs**
- 8.1 claims ARCHITECTURE/SPEC updates; those sections exist, but they are now stale.
- ARCHITECTURE/SPEC still show old test counts (79/10/11) and list gestures or floating/rules as pending.
- Specs do not mention overlay snap hints, tray integration, QueryAllWindows, gesture plumbing, or window rules.

## Confirmed Capabilities (Repo State)
- Floating windows supported alongside tiled layout (`FloatingWindow`).
- IPC protocol supports `QueryAllWindows`, `WindowInfo`, and `WindowList`.
- Window rules and snap hint config implemented (daemon config and tests).
- Overlay snap hints, global hotkeys, gesture scaffolding, and system tray wiring present.
- Multi-monitor support and animated scrolling appear implemented in daemon and core layout.

## Iteration 22 Improvements (New)
- **Critical unwrap fixes**: Floating window rect defaults to centered 800x600 if not specified.
- **HWND validation**: `is_valid_window()` function prevents race conditions with destroyed windows.
- **Crash protection**: `catch_unwind` wrappers on all Win32 callbacks (hotkey, gesture, overlay, WinEvent).
- **DeferWindowPos fallback**: Falls back to individual SetWindowPos if batch fails.
- **Display change infrastructure**: `reconcile_monitors()` ready for monitor hotplug handling.
- **Comprehensive overlay docs**: Full documentation for overlay.rs module.
- **12 new daemon tests**: Testing config, window rules, and state management.

## Documentation Drift (Action Needed)
- `docs/ARCHITECTURE.md` and `docs/SPEC.md` lag current implementation.
- Test counts in both docs are stale and should be updated to 147.
- SPEC.md mentions gestures and floating rules but needs details on implementation.
- Docs should mention overlay snap hints, tray integration, QueryAllWindows, catch_unwind robustness.

## Iteration Log Review
- `docs/1_Progress and review/ITERATION_LOG.md` is now accurate with 147 tests after Iteration 22.
- Test progression: 111 (Iter 20) → 131 (Iter 21) → 147 (Iter 22).

## Gaps and Follow-ups (Based on Current Code)
- `Config.appearance.use_cloaking` is defined but not used; TODO remains for alternate hide strategy.
- No end-to-end integration test for CLI <-> IPC <-> daemon <-> Win32 layout application.
- Display change events infrastructure added but not wired into daemon event loop yet.
- Focus follows mouse config added but behavior not yet implemented.
