# Codex Consolidated Review + Iteration Log Audit (Review 15)
Date: 2026-02-04
Reviewer: Codex
Scope: Repo state verification, Iteration 23 completion validation, test coverage review
Status: Review complete. No code changes made.

## Verification Summary (Commands Run)
- `cargo test --workspace` -> **PASSED**: 202 passed, 2 ignored (plus 3 ignored doc-tests)
- `cargo clippy --workspace` -> **PASSED**: No warnings
- Breakdown: core_layout 87, daemon 44, cli 28, integration 17, ipc 13, platform_win32 13 (+2 ignored)

## Iteration 23 Completion Verification

### Features Implemented (Verified in Code)
| Feature | Status | Evidence |
|---------|--------|----------|
| DisplayChange event wiring | DONE | `set_display_change_sender()` in platform_win32, handled in daemon event loop |
| focus_follows_mouse | DONE | `install_mouse_hook()` in platform_win32, debounced handling in daemon |
| use_cloaking config wiring | DONE | `HideStrategy::MoveOffScreen` restored, config controls strategy |
| QueryAllWindows CLI | DONE | `QueryType::All` variant in CLI |
| CLI unit tests | DONE | 28 tests added to `crates/cli/src/main.rs` |
| Integration tests | DONE | 17 tests in `crates/daemon/tests/integration.rs` |
| Window rule edge case tests | DONE | 10 tests in `crates/daemon/src/config.rs` |

### Test Growth
- **Before Iteration 23**: 147 tests
- **After Iteration 23**: 202 tests (+55 tests)
- **Target was**: 160+ tests - **EXCEEDED**

## Test Quality & Coverage Assessment

**Strengths**
- `core_layout` has broad unit coverage across layout invariants, focus behavior, scrolling/animation, and floating windows (87 tests).
- `cli` now has comprehensive unit tests covering all command conversions, config generation, and response formatting (28 tests).
- `ipc` tests cover JSON roundtrips for all commands/responses plus protocol framing and invalid JSON handling (13 tests).
- `daemon` config tests exercise defaults, partial TOML parsing, bounds clamping, window rules, and edge cases (44 tests).
- `integration` tests verify IPC protocol correctness without requiring Win32 runtime (17 tests).

**Gaps / Coverage Risks (Remaining)**
- **OS-bound behavior untested**: WinEvent hooks, hotkeys, named pipe server handling, overlay rendering, tray callbacks, and gesture detection rely on runtime/manual validation.
- **Error-path coverage is thin**: no tests for failed hook install, failed hotkey registration, bad pipe reads, or reload failures.
- **Multi-monitor behavior** only validated in pure helper functions; no end-to-end monitor switching tests.
- No coverage instrumentation (line/branch); only test counts are available.

## Recommended Missing Tests (Priority Order)

- **P1**: Daemon reload test: modify config, trigger reload, verify hotkeys + layout config updated.
- **P1**: Multi-monitor focus/move commands using synthetic monitor sets.
- **P2**: Win32 hook registration failure path (simulate/force error; ensure graceful fallback).
- **P2**: Gesture mapping sanity (config -> command mapping) without needing actual touchpad input.
- **P2**: CLI error handling when daemon is absent (connect failure, timeout).

## Project Progress Update (Current Snapshot)

**Phases Completed (Based on Repo Evidence)**
- **Foundation & Layout**: Core layout engine, invariants, and large unit test suite are in place (87 tests).
- **Platform + IPC Integration**: Win32 enumeration/placement/cloaking, IPC protocol crate, named pipe server/client, async daemon loop are implemented.
- **Config + UX Features**: Config load/reload, global hotkeys, multi-monitor logic, smooth scrolling, overlay snap hints, tray wiring, window rules, gesture scaffolding, focus_follows_mouse, and display change handling exist.
- **Testing Infrastructure**: CLI tests, integration tests, and comprehensive edge case coverage added.

**Requirements & Specs Status (Reality vs Docs)**
- Core requirements in `docs/SPEC.md` appear **mostly implemented**, but the spec is **stale** and missing several now-implemented features (overlay hints, tray, QueryAllWindows, gesture plumbing, window rules, focus_follows_mouse).
- `docs/ARCHITECTURE.md` reflects an older snapshot and outdated test counts; it does not describe the current IPC and feature surface accurately.
- Net: code is **ahead of the docs**, so the written requirements/specs need a refresh to match actual behavior.

**Overall Accomplishment / Achievement**
- The project has moved beyond "layout-only" into a functioning vertical slice: **CLI -> IPC -> daemon -> layout -> Win32 placement** exists in code.
- Test coverage has grown significantly (202 tests) with CLI and integration tests filling previous gaps.
- Infrastructure wiring is now complete (DisplayChange, focus_follows_mouse, use_cloaking all functional).

## Previously Reported Gaps - Status Update

| Gap | Previous Status | Current Status |
|-----|-----------------|----------------|
| CLI has 0 tests | GAP | **FIXED** - 28 tests added |
| No integration tests | GAP | **FIXED** - 17 tests added |
| use_cloaking not wired | GAP | **FIXED** - Controls HideStrategy |
| DisplayChange not wired | GAP | **FIXED** - Calls reconcile_monitors() |
| focus_follows_mouse not implemented | GAP | **FIXED** - Mouse hook + debouncing |
| QueryAllWindows missing from CLI | GAP | **FIXED** - `query all` subcommand |
| Window rule edge cases untested | GAP | **FIXED** - 10 tests added |
| Stray `nul` file at repo root | GAP | **FIXED** - Deleted in Iteration 21 |

## Documentation Drift (Action Needed)

- `docs/ARCHITECTURE.md` and `docs/SPEC.md` lag current implementation.
- Test counts in both docs are stale (79/10/11 vs actual 87/44/28/17/13/13).
- `SPEC.md` still lists gestures and per-window floating/rules as pending.
- Neither doc mentions overlay snap hints, tray integration, QueryAllWindows, gesture plumbing, window rules, or focus_follows_mouse.
- Per instruction, no doc edits were made in this review.

## Files Modified in Iteration 23

| File | Changes |
|------|---------|
| `crates/daemon/src/main.rs` | Wire DisplayChange, focus_follows_mouse, use_cloaking |
| `crates/platform_win32/src/lib.rs` | Mouse hook, set_display_change_sender, HideStrategy::MoveOffScreen |
| `crates/cli/src/main.rs` | QueryType::All, 28 unit tests |
| `crates/daemon/src/config.rs` | 10 window rule edge case tests |
| `crates/daemon/tests/integration.rs` | NEW: 17 integration tests |
| `crates/daemon/src/tray.rs` | Fix clippy warning (TrayError naming) |

## Next Iteration (24) Recommendations

1. **Workspace persistence** - Save/restore window positions across daemon restarts
2. **Multi-workspace support** - Named workspaces per monitor
3. **Enhanced window rules** - Assign to specific workspace, monitor targeting
4. **Performance profiling** - Identify hotspots in layout computation and Win32 calls
5. **Documentation refresh** - Update ARCHITECTURE.md and SPEC.md to match implementation
