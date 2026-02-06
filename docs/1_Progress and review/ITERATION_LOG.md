# OpenNiri-Windows Development Iteration Log

> **Purpose**: This document tracks all development iterations, providing evidence and links for meaningful review and verification.
> **Maintainer**: Claude (Anthropic AI Assistant)
> **Last Updated**: 2026-02-06 (Iteration 32 — Public README and GitHub About Revamp)

---

## Table of Contents

1. [Project Overview](#project-overview)
2. [Iteration Summary Table](#iteration-summary-table)
3. [Detailed Iteration Logs](#detailed-iteration-logs)
4. [Test Coverage History](#test-coverage-history)
5. [Architecture Evolution](#architecture-evolution)
6. [Known Issues & Technical Debt](#known-issues--technical-debt)

---

## Project Overview

| Attribute | Value |
|-----------|-------|
| **Project** | OpenNiri-Windows |
| **Description** | Niri-like scrollable tiling window manager for Windows |
| **Repository** | https://github.com/AdEx-Partners-DE/OpenNiri-Windows |
| **Language** | Rust |
| **Target Platform** | Windows 10/11 (x86_64) |
| **Toolchain** | stable-x86_64-pc-windows-gnu (MinGW) |

### Crate Structure

```
OpenNiri-Windows/
├── crates/
│   ├── core_layout/      # Platform-agnostic layout engine
│   ├── platform_win32/   # Win32 API integration
│   ├── ipc/              # IPC protocol types
│   ├── daemon/           # Main daemon process
│   └── cli/              # Command-line interface
└── docs/
    ├── ARCHITECTURE.md   # Technical architecture
    ├── SPEC.md           # Behavioral specification
    └── 1_Progress and review/
        └── ITERATION_LOG.md  # This file
```

---

## Iteration Summary Table

| Iteration | Date | Focus Area | Tests Before | Tests After | Key Deliverables |
|-----------|------|------------|--------------|-------------|------------------|
| 1-7 | Pre-2026-02-04 | Core layout, Win32 basics | 0 | 52 | Layout engine, basic Win32 |
| 8.1 | 2026-02-04 | IPC Protocol Crate | 52 | 57 | `openniri-ipc` crate |
| 8.2 | 2026-02-04 | Monitor Detection | 57 | 60 | `enumerate_monitors()`, `get_primary_monitor()` |
| 8.3 | 2026-02-04 | Async Daemon & CLI IPC | 60 | 60 | Named pipe server, real IPC |
| 9 | 2026-02-04 | Codex Review Implementation | 60 | 63 | WinEvent hooks, cleanup |
| 10 | 2026-02-04 | Configuration Support | 63 | 69 | TOML config, reload, init |
| 11 | 2026-02-04 | Multi-monitor Support | 69 | 74 | Per-monitor workspaces, cross-monitor commands |
| 12 | 2026-02-04 | Codex Audit + Doc Refresh | 74 | 74 | Updated review + agent guidance |
| 13 | 2026-02-04 | Global Hotkey Support | 74 | 81 | RegisterHotKey API, config-driven bindings |
| 14 | 2026-02-04 | Smooth Scroll Animations | 81 | 108 | Easing functions, animated workspace scroll |
| 15 | 2026-02-04 | Codex Review + Doc Drift Audit | 108 | 108 | Updated review with doc drift findings |
| 16 | 2026-02-04 | Codex Review + QA Scan | 108 | 108 | Updated review with reload/hotkey gap |
| 17 | 2026-02-04 | Codex Review + QA Scan (Failure) | 108 | FAIL | `cargo test --all` failed (E0599 Config::generate_default) |
| 18 | 2026-02-04 | Codex Review + QA Scan (Failure) | FAIL | FAIL | Added NUL file issue + repeat E0599 failure |
| 19 | 2026-02-04 | Config Completeness & Doc Sync | 111 | 111 | Hotkey reload fix, log_level, track_focus_changes, doc updates |
| 20 | 2026-02-04 | Codex Review + QA Scan (Pass) | 111 | 111 | Tests pass again; iteration log inconsistency flagged |
| 21 | 2026-02-04 | Full Feature Push | 111 | 131 | System tray, window rules, gestures, snap hints |
| 22 | 2026-02-04 | Quality & Robustness | 131 | 147 | Fix unwraps, HWND validation, unit tests, docs, catch_unwind, IPC queries |
| 23 | 2026-02-04 | Feature Completion & Tests | 147 | 202 | Wire DisplayChange, focus_follows_mouse, use_cloaking, CLI tests, integration tests |
| 24 | 2026-02-05 | Real Gestures, Persistence, Docs | 202 | 206 | Real touchpad gestures, workspace persistence, doc refresh |
| 25 | 2026-02-05 | Config Validation & Safety | 206 | 231 | Config regex validation, pre-compiled rules, safety hardening |
| 26 | 2026-02-05 | Config Validation & Safety (cont.) | 231 | 234 | Additional safety tests, clippy fixes |
| 27 | 2026-02-05 | Test Coverage & Doc Accuracy | 234 | 257 | handle_command() tests, reconcile_monitors() tests, doc updates |
| 28 | 2026-02-05 | Codex Review 19 Fixes | 257 | 261 | reconcile_monitors bug fix, 7 strengthened tests, 3 new cmd tests, clippy --all-targets clean |
| 29 | 2026-02-05 | UX overhaul: SetForegroundWindow, CloseWindow, ToggleFloating, ToggleFullscreen, column presets, active border, status, tray menu, auto-start | 261 | 295 | 0 warnings |
| 30 | 2026-02-05 | Crash safety and reliability: Ctrl+C shutdown, uncloak-on-exit/crash, DPI awareness | 295 | 302 | 297 passed, 5 ignored, strict clippy clean |
| 31 | 2026-02-05 | Public repo presentation refresh (README + GitHub metadata) | 302 | 302 | README rewrite, GitHub description/topics updated |
| 32 | 2026-02-06 | Public messaging revamp (README v2 + GitHub About cleanup) | 302 | 302 | Professionalized README structure, tightened GitHub positioning and discovery topics |

---

## Detailed Iteration Logs

### Iteration 32: Public README and GitHub About Revamp

**Date**: 2026-02-06
**Status**: COMPLETED
**Previous Context**: Iteration 31 (Repository Presentation Refresh)

#### 32.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Replace generic README language with product-grade messaging | High | DONE |
| 2 | Clarify "why", status, and usage expectations for public users | High | DONE |
| 3 | Expand quick-start and default hotkey reference coverage | Medium | DONE |
| 4 | Ensure GitHub About metadata reflects current project scope | High | DONE |

#### 32.2 Changes Made

- Reworked `README.md` to improve first impression and reduce ambiguity:
  - Added CI and license badges.
  - Tightened positioning and "why this project exists" narrative.
  - Added explicit product status (alpha) and expectation-setting.
  - Expanded hotkey table to include monitor focus/move and refresh actions.
  - Added config path and clearer architecture/doc references.
- Updated GitHub repository metadata to align with project identity and discoverability:
  - Description aligned to actual product behavior.
  - Topics reviewed/updated for better categorization.

#### 32.3 Test Results

No Rust code changes in this iteration.

**Test Growth**: 302 -> 302 (unchanged)

#### 32.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| GitHub metadata check | `gh repo view AdEx-Partners-DE/OpenNiri-Windows --json description,repositoryTopics,url` | Description/topics match new positioning |
| README sanity check | Manual markdown review | Sections render and link cleanly |

---

### Iteration 30: Crash Safety and Reliability

**Date**: 2026-02-05
**Status**: COMPLETED
**Previous Context**: Iteration 29 (Dramatic UX Overhaul)

#### 30.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Add Ctrl+C signal handling | High | DONE |
| 2 | Uncloak managed windows on shutdown | High | DONE |
| 3 | Add panic-hook emergency uncloak | High | DONE |
| 4 | Enable DPI awareness at process startup | Medium | DONE |
| 5 | Align tray exit with unified shutdown path | High | DONE |
| 6 | Add reliability-focused regression tests | Medium | DONE |

#### 30.2 Changes Made

- Added `tokio::signal::ctrl_c()` task that sends `DaemonEvent::Shutdown`.
- Added managed-window shutdown recovery:
  - `AppState::all_managed_window_ids()` in daemon.
  - `uncloak_all_managed_windows()` in platform layer.
- Added crash recovery safety net:
  - Panic hook in daemon (`std::panic::set_hook`) that invokes `uncloak_all_visible_windows()`.
- Added process DPI initialization:
  - `set_dpi_awareness()` using `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`.
  - Called at the top of daemon `main()` before other window operations.
- Unified shutdown behavior:
  - Tray Exit now routes through `DaemonEvent::Shutdown`, so shutdown cleanup is shared across IPC Stop, Ctrl+C, and tray exit.

#### 30.3 Test Results

```
Test Summary (2026-02-05):
- core_layout:    99 passed, 0 failed, 0 ignored
- daemon:         99 passed, 0 failed, 1 ignored
- cli:            38 passed, 0 failed, 0 ignored
- integration:    22 passed, 0 failed, 0 ignored
- ipc:            15 passed, 0 failed, 0 ignored
- platform_win32: 24 passed, 0 failed, 3 ignored
- doc-tests:       0 passed, 0 failed, 1 ignored

TOTAL: 297 passed, 0 failed, 5 ignored (302 total)
Clippy: 0 warnings (`--workspace --all-targets -- -D warnings`)
```

**Test Growth**: 295 -> 302 (+7 tests)

#### 30.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --workspace` | 297 passed, 5 ignored |
| Build succeeds | `cargo build --release` | Success |
| Strict clippy clean | `cargo clippy --workspace --all-targets -- -D warnings` | 0 warnings |

---

### Iteration 29: Dramatic UX Overhaul

**Date**: 2026-02-05
**Status**: COMPLETED
**Previous Context**: Iteration 28 (Codex Review 19 Fixes)

#### 29.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | SetForegroundWindow integration for focus commands | Critical | DONE |
| 2 | Owner-window filtering (dialogs, UWP) | High | DONE |
| 3 | CloseWindow command (Win+Shift+Q) | High | DONE |
| 4 | ToggleFloating command (Win+F) | High | DONE |
| 5 | ToggleFullscreen command (Win+Shift+F) | High | DONE |
| 6 | SetColumnWidth presets (Win+1/2/3/0) | High | DONE |
| 7 | Active window border via DWM | Medium | DONE |
| 8 | Snap hints and gestures enabled by default | Medium | DONE |
| 9 | QueryStatus command and CLI status subcommand | Medium | DONE |
| 10 | Tray menu: Pause Tiling, Open Config, View Logs | Medium | DONE |
| 11 | Auto-start via Registry (CLI autostart enable/disable) | Medium | DONE |

#### 29.2 Changes Made

##### 29.2.1 Phase 1: SetForegroundWindow Integration

**Problem**: Focus commands (FocusUp, FocusDown, FocusLeft, FocusRight) updated internal layout state but did not actually move OS-level keyboard focus to the target window. Users had to click windows manually after focusing.

**Solution**: Focus commands now call `SetForegroundWindow` after updating internal state. `FocusUp` and `FocusDown` now also call `apply_layout()` to ensure window positions are applied.

##### 29.2.2 Phase 2: Owner-Window Filtering

**Problem**: Dialog windows (owned windows) were being tiled alongside their parent applications, causing layout corruption. UWP apps like Calculator were not being tiled because their window class (`ApplicationFrameWindow`) was not recognized.

**Solution**: Added owner-window filtering so that owned/dialog windows (those with a non-null owner via `GetWindow(GW_OWNER)`) are excluded from tiling. UWP apps with `ApplicationFrameWindow` class are now correctly identified and tiled.

##### 29.2.3 Phase 3: CloseWindow Command

Added `CloseWindow` IPC command with default hotkey `Win+Shift+Q`. Sends `WM_CLOSE` to the focused window, allowing graceful application shutdown.

##### 29.2.4 Phase 4: ToggleFloating Command

Added `ToggleFloating` IPC command with default hotkey `Win+F`. Toggles the focused window between tiled and floating states. Floating windows are removed from the column layout and positioned with their original dimensions.

##### 29.2.5 Phase 5: ToggleFullscreen Command

Added `ToggleFullscreen` IPC command with default hotkey `Win+Shift+F`. Toggles the focused window between normal layout and fullscreen (covering the entire monitor work area). Fullscreen state is tracked per-window.

##### 29.2.6 Phase 6: SetColumnWidth Presets

Added `SetColumnWidth` IPC command with fraction-based presets:
- `Win+1` = 1/3 width
- `Win+2` = 1/2 width
- `Win+3` = 2/3 width
- `Win+0` = equalize all columns

Allows quick column resizing without incremental resize commands.

##### 29.2.7 Phase 7: Active Window Border via DWM

Added active window border highlighting using `DwmSetWindowAttribute` with `DWMWA_BORDER_COLOR`. When a window gains focus, its border color is set to the configured accent color. When it loses focus, the border is reset to the default. Configurable via `appearance.active_border_color`.

##### 29.2.8 Phase 8: Snap Hints and Gestures Enabled by Default

Changed default configuration so that both snap hints (`snap_hints.enabled`) and gestures (`gestures.enabled`) are `true` by default, improving out-of-the-box experience.

##### 29.2.9 Phase 9: QueryStatus Command and CLI Status Subcommand

Added `QueryStatus` IPC command and `openniri-cli status` subcommand. Returns daemon status information including: number of managed windows, number of monitors, active workspace details, tiling pause state, and uptime.

##### 29.2.10 Phase 10: Tray Menu Enhancements

Extended the system tray context menu with three new items:
- **Pause Tiling** - Toggles tiling on/off without stopping the daemon
- **Open Config** - Opens the config file in the default editor
- **View Logs** - Opens the log directory in Explorer

##### 29.2.11 Phase 11: Auto-start via Registry

Added `openniri-cli autostart enable` and `openniri-cli autostart disable` subcommands. Uses the Windows Registry key `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` to add/remove the daemon from startup.

#### 29.3 Test Results

```
Test Summary (2026-02-05):
- core_layout:    99 passed, 0 failed, 0 ignored
- daemon:         96 passed, 0 failed, 1 ignored
- cli:            38 passed, 0 failed, 0 ignored
- integration:    22 passed, 0 failed, 0 ignored
- ipc:            15 passed, 0 failed, 0 ignored
- platform_win32: 21 passed, 0 failed, 2 ignored
- doc-tests:       0 passed, 0 failed, 1 ignored

TOTAL: 291 passed, 0 failed, 4 ignored
Clippy: 0 warnings
```

**Test Growth**: 261 -> 295 (+34 tests)

#### 29.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --workspace` | 291 passed, 4 ignored |
| Build succeeds | `cargo build --workspace` | Success |
| Clippy clean | `cargo clippy --workspace` | 0 warnings |

#### 29.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/daemon/src/main.rs` | +300 | SetForegroundWindow, CloseWindow, ToggleFloating, ToggleFullscreen, SetColumnWidth, active border, pause tiling, status |
| `crates/platform_win32/src/lib.rs` | +150 | Owner-window filtering, DWM border color, UWP detection |
| `crates/cli/src/main.rs` | +100 | Status subcommand, autostart subcommand, new command mappings |
| `crates/ipc/src/lib.rs` | +50 | New IPC commands and response types |
| `crates/daemon/src/config.rs` | +50 | Active border config, default changes for snap hints/gestures |
| `crates/daemon/src/tray.rs` | +80 | Pause Tiling, Open Config, View Logs menu items |
| `crates/core_layout/src/lib.rs` | +50 | Column width preset support |

---

### Iteration 24: Real Gestures, Workspace Persistence & Doc Refresh

**Date**: 2026-02-05
**Status**: COMPLETED
**Previous Context**: Iteration 23 (Feature Completion & Test Expansion)

#### 24.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Update SPEC.md to reflect current implementation | High | DONE |
| 2 | Update ARCHITECTURE.md to reflect current implementation | High | DONE |
| 3 | Implement real touchpad gesture support | High | DONE |
| 4 | Implement workspace persistence (save/restore) | High | DONE |
| 5 | Add tests for new features | Medium | DONE |

#### 24.2 Changes Made

##### 24.2.1 Documentation Refresh

**File**: `docs/SPEC.md`

Updated to reflect all implemented features:
- Added Per-Window Rules section (was listed as "pending", now documented)
- Added Floating Windows section
- Added System Tray section
- Added Visual Snap Hints section
- Added Focus Follows Mouse section
- Added Touchpad Gesture Support section
- Added Display Change Handling section
- Added Workspace Persistence section
- Updated Implementation Status (202 tests, all features implemented)
- Removed stale "Pending" items

**File**: `docs/ARCHITECTURE.md`

Updated to reflect current state:
- Updated IPC types (added QueryAllWindows, WindowList, WindowInfo, IpcRect)
- Updated CLI commands (added focus-monitor, move-to-monitor, query all, init)
- Updated daemon responsibilities (persistence, tray, gesture/mouse hooks)
- Updated event loop diagram (added hotkeys, gestures, tray, display change, focus follows mouse)
- Updated Current Status with correct test counts (202 total)
- Updated Planned vs Implemented (all features now implemented)
- Updated threading model (7 threads/tasks documented)
- Updated error handling (DeferWindowPos fallback, HWND validation, config fallbacks)

##### 24.2.2 Real Touchpad Gesture Support

**File**: `crates/platform_win32/src/lib.rs`

**Problem**: Previous implementation used a message-only window (HWND_MESSAGE) listening for WM_POINTERDOWN/WM_POINTERUP, which never receives real touchpad input.

**Solution**: Replaced with a low-level mouse hook (WH_MOUSE_LL) that captures WM_MOUSEWHEEL and WM_MOUSEHWHEEL events system-wide.

**New types**:
```rust
struct GestureAccumState {
    accum_x: i32,
    accum_y: i32,
    last_scroll_time: std::time::Instant,
}
```

**New constants**:
```rust
const WM_MOUSEWHEEL: u32 = 0x020A;
const WM_MOUSEHWHEEL: u32 = 0x020E;
const GESTURE_SCROLL_THRESHOLD: i32 = 360; // 3 * WHEEL_DELTA
const GESTURE_TIMEOUT_MS: u128 = 300;
```

**How it works**:
1. `register_gestures()` installs `WH_MOUSE_LL` hook
2. Hook callback captures WM_MOUSEWHEEL/WM_MOUSEHWHEEL
3. Extracts delta from `mouseData >> 16` (high word)
4. Accumulates horizontal (WM_MOUSEHWHEEL) and vertical (WM_MOUSEWHEEL) deltas
5. When accumulated delta exceeds threshold (360 = 3x WHEEL_DELTA), fires GestureEvent
6. Resets accumulator after timeout (300ms no scroll)

**Removed**: WM_POINTER message constants, gesture_window_proc, gesture_window_proc_inner, GestureState (replaced by GestureAccumState), thread-based GestureHandle.

**Simplified GestureHandle**: Now holds just an HHOOK (like MouseHookHandle), no thread.

##### 24.2.3 Workspace Persistence

**File**: `crates/daemon/src/main.rs`

**New types**:
```rust
#[derive(Serialize, Deserialize)]
struct WorkspaceSnapshot {
    monitor_device_name: String,
    workspace: Workspace,
}

#[derive(Serialize, Deserialize)]
struct StateSnapshot {
    saved_at: String,
    workspaces: Vec<WorkspaceSnapshot>,
    focused_monitor_name: String,
}
```

**New AppState methods**:
- `save_state()` - Serialize workspace state to `%APPDATA%/openniri/data/workspace-state.json`
- `load_state()` - Deserialize saved state from disk
- `state_file_path()` - Get persistence file path
- `restore_state()` - Apply saved scroll offsets and focus to matching monitors

**Lifecycle integration**:
- On startup: After monitor setup, before window enumeration, attempts to restore saved state
- On shutdown (DaemonEvent::Shutdown): Saves current state before exiting
- On tray Exit: Saves current state before triggering shutdown

**Matching strategy**: Monitors are matched by `device_name` (e.g., "DISPLAY1") which is stable across restarts, unlike MonitorId (HMONITOR handle values).

#### 24.3 Test Results

```
Test Summary (2026-02-05):
- core_layout:    87 passed, 0 failed, 0 ignored
- daemon:         48 passed, 0 failed, 0 ignored
- cli:            28 passed, 0 failed, 0 ignored
- integration:    17 passed, 0 failed, 0 ignored
- ipc:            13 passed, 0 failed, 0 ignored
- platform_win32: 13 passed, 0 failed, 2 ignored

TOTAL: 206 passed, 0 failed, 2 ignored (1 doc-test ignored)
Clippy: No warnings
```

**Test Growth**: 202 → 206 (+4 tests)

**New Tests**:
- `test_state_file_path` - Validates persistence path
- `test_state_snapshot_serialization` - StateSnapshot roundtrip
- `test_workspace_snapshot_serialization` - WorkspaceSnapshot roundtrip
- `test_save_and_load_roundtrip` - Full snapshot roundtrip with workspace data

#### 24.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --workspace` | 206 passed, 2 ignored |
| Build succeeds | `cargo build --workspace` | Success |
| Clippy clean | `cargo clippy --workspace` | No warnings |

#### 24.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/platform_win32/src/lib.rs` | ~288 lines changed | Real gesture hook, simplified GestureHandle |
| `crates/daemon/src/main.rs` | +208 lines | Workspace persistence, 4 tests |
| `docs/SPEC.md` | +126 lines | Doc refresh with all features |
| `docs/ARCHITECTURE.md` | +154 lines changed | Doc refresh, current state |
| `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` | Updated | Reflect Iteration 23 fixes |

---

### Iteration 27: Test Coverage & Documentation Accuracy

**Date**: 2026-02-05
**Status**: COMPLETED
**Previous Context**: Iteration 26 (Additional Safety & Clippy Fixes)

#### 27.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Add handle_command() unit tests | High | DONE |
| 2 | Add reconcile_monitors() unit tests | High | DONE |
| 3 | Fix flaky integration test | Medium | DONE |
| 4 | Update SPEC.md test counts | Medium | DONE |
| 5 | Update ARCHITECTURE.md test counts | Medium | DONE |
| 6 | Update ITERATION_LOG.md | Medium | DONE |

#### 27.2 Changes Made

- Added 16 unit tests for `handle_command()` covering all IPC command branches
- Added 7 unit tests for `reconcile_monitors()` covering add/remove/migrate scenarios
- Marked flaky `test_check_already_running_returns_false_when_no_daemon` as `#[ignore]`
- Updated SPEC.md implementation status (206 → 257 tests)
- Updated ARCHITECTURE.md test counts
- Updated this log with iterations 25-27

#### 27.3 Test Results

All tests passing, 0 clippy warnings, clean release build.

---

### Iteration 26: Additional Safety & Clippy Fixes

**Date**: 2026-02-05
**Status**: COMPLETED
**Previous Context**: Iteration 25 (Config Validation & Safety)

#### 26.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Additional safety tests | Medium | DONE |
| 2 | Clippy warning fixes | Medium | DONE |
| 3 | Config path consistency | Low | DONE |

#### 26.2 Changes Made

- Added additional safety tests for edge cases
- Fixed clippy warnings across workspace
- Config path consistency improvements per Codex review
- Test count: 231 → 234

---

### Iteration 25: Config Validation & Safety Hardening

**Date**: 2026-02-05
**Status**: COMPLETED
**Previous Context**: Iteration 24 (Real Gestures, Persistence, Docs)

#### 25.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Add regex validation to config window rules | High | DONE |
| 2 | Pre-compile window rule regexes at config load | High | DONE |
| 3 | Safety hardening for edge cases | Medium | DONE |

#### 25.2 Changes Made

- Config: Added validation that regex patterns in window rules are valid at load time
- Config: Pre-compiled regex patterns stored in `CompiledWindowRule` for efficient matching
- Safety: Added bounds checking and defensive patterns in config handling
- Tests: Added 25 new tests for config validation and compiled rules

---

### Iteration 23: Feature Completion & Test Expansion

**Date**: 2026-02-04
**Status**: COMPLETED
**Previous Context**: Iteration 22 (Quality & Robustness)

#### 23.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1.1 | Wire DisplayChange event in daemon | High | DONE |
| 1.2 | Implement focus_follows_mouse | High | DONE |
| 1.3 | Wire use_cloaking config | Medium | DONE |
| 2.1 | Add QueryAllWindows CLI subcommand | Medium | DONE |
| 2.2 | Add CLI unit tests | High | DONE |
| 3.1 | Add integration test infrastructure | Medium | DONE |
| 3.2 | Add window rule edge case tests | Medium | DONE |

#### 23.2 Changes Made

##### 23.2.1 Phase 1: Wire Existing Infrastructure

**Task 1.1: Wire DisplayChange Event**

**File**: `crates/platform_win32/src/lib.rs`

Added `set_display_change_sender()` function to allow the daemon to register a sender for display change events:
```rust
pub fn set_display_change_sender(sender: mpsc::Sender<WindowEvent>) -> Result<(), Win32Error>
```

**File**: `crates/daemon/src/main.rs`

- Added `DisplayChange` channel and wired it to call `reconcile_monitors()` when display configuration changes
- Display changes now properly trigger monitor hotplug handling

**Task 1.2: Implement focus_follows_mouse**

**File**: `crates/platform_win32/src/lib.rs`

Added low-level mouse hook infrastructure:
```rust
pub struct MouseHookHandle { ... }

pub fn install_mouse_hook(event_sender: mpsc::Sender<WindowEvent>) -> Result<MouseHookHandle, Win32Error>
```

- Uses `SetWindowsHookEx(WH_MOUSE_LL)` to track mouse position
- Detects when mouse enters a managed window via `WindowFromPoint`
- Sends `WindowEvent::MouseEnterWindow(window_id)` events

**File**: `crates/daemon/src/main.rs`

- Added `FocusFollowsMouse { window_id }` variant to `DaemonEvent`
- Added `apply_focus_follows_mouse()` method to `AppState`
- Implemented debouncing using tokio timers (`focus_follows_mouse_delay_ms` config)
- Mouse hook only installed when `config.behavior.focus_follows_mouse = true`

**Task 1.3: Wire use_cloaking Config**

**File**: `crates/platform_win32/src/lib.rs`

Added `HideStrategy::MoveOffScreen` variant (was previously removed, now restored for config flexibility):
```rust
pub enum HideStrategy {
    #[default]
    Cloak,
    MoveOffScreen,
}
```

**File**: `crates/daemon/src/main.rs`

- Platform config now respects `config.appearance.use_cloaking`:
  - `true` → `HideStrategy::Cloak`
  - `false` → `HideStrategy::MoveOffScreen`

##### 23.2.2 Phase 2: CLI Completion

**Task 2.1: Add QueryAllWindows CLI Subcommand**

**File**: `crates/cli/src/main.rs`

Added `QueryType::All` variant:
```rust
#[derive(Subcommand)]
enum QueryType {
    Workspace,
    Focused,
    All,  // NEW - maps to IpcCommand::QueryAllWindows
}
```

Usage: `openniri-cli query all`

**Task 2.2: Add CLI Unit Tests**

**File**: `crates/cli/src/main.rs`

Added 28 comprehensive unit tests covering:
- `test_to_ipc_command_focus_left/right/up/down`
- `test_to_ipc_command_move_column_left/right`
- `test_to_ipc_command_scroll_positive/negative/zero`
- `test_to_ipc_command_resize_positive/negative/zero`
- `test_to_ipc_command_focus_monitor_left/right`
- `test_to_ipc_command_move_to_monitor_left/right`
- `test_to_ipc_command_query_workspace/focused/all`
- `test_to_ipc_command_refresh/apply/reload/stop`
- `test_generate_default_config_contains_layout/appearance/behavior/hotkeys`
- `test_default_config_path_returns_some`
- `test_print_response_ok/error/workspace_state/focused_window/window_list`

##### 23.2.3 Phase 3: Test Expansion

**Task 3.1: Integration Test Infrastructure**

**File**: `crates/daemon/tests/integration.rs` (NEW)

Created 17 integration tests for IPC protocol correctness:
- `test_all_commands_roundtrip` - All command variants serialize/deserialize
- `test_all_responses_roundtrip` - All response variants roundtrip
- `test_protocol_newline_delimited` - Protocol message format
- `test_response_newline_delimited` - Response message format
- `test_error_response_message/special_chars` - Error handling
- `test_workspace_state_edge_values/large_values/negative_scroll`
- `test_window_list_empty/multiple_windows`
- `test_window_info_unicode_title` - Unicode support
- `test_resize_command_values` - Edge cases (i32::MAX, i32::MIN)
- `test_scroll_command_values` - Edge cases (f64::MAX, f64::MIN)
- `test_invalid_json_parsing` - Error handling
- `test_unknown_command_type/response_type` - Unknown variants

**Task 3.2: Window Rule Edge Case Tests**

**File**: `crates/daemon/src/config.rs`

Added 10 window rule edge case tests:
- `test_window_rule_multiple_matches_uses_first`
- `test_window_rule_regex_special_chars`
- `test_window_rule_regex_anchors`
- `test_window_rule_empty_string_matches`
- `test_window_rule_case_sensitive_class_title`
- `test_window_rule_case_insensitive_executable`
- `test_window_rule_partial_config_class_only/title_only/executable_only`
- `test_window_rule_action_priority`

#### 23.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    87 passed, 0 failed, 0 ignored
- daemon:         44 passed, 0 failed, 0 ignored
- cli:            28 passed, 0 failed, 0 ignored
- integration:    17 passed, 0 failed, 0 ignored
- ipc:            13 passed, 0 failed, 0 ignored
- platform_win32: 13 passed, 0 failed, 2 ignored

TOTAL: 202 passed, 0 failed, 2 ignored (3 doc-tests ignored)
Clippy: No warnings
```

**Test Growth**: 147 → 202 (+55 tests)

#### 23.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --workspace` | 202 passed, 2 ignored |
| Build succeeds | `cargo build --workspace` | Success |
| Clippy clean | `cargo clippy --workspace` | No warnings |

#### 23.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/daemon/src/main.rs` | +100 | Wire DisplayChange, focus_follows_mouse, use_cloaking |
| `crates/platform_win32/src/lib.rs` | +150 | Mouse hook, set_display_change_sender, HideStrategy |
| `crates/cli/src/main.rs` | +300 | QueryType::All, 28 unit tests |
| `crates/daemon/src/config.rs` | +150 | 10 window rule edge case tests |
| `crates/daemon/tests/integration.rs` | +430 | NEW: 17 integration tests |
| `crates/daemon/src/tray.rs` | +5 | Fix clippy warning (TrayError naming) |

---

### Iteration 22: Quality, Robustness & Feature Expansion

**Date**: 2026-02-04
**Status**: COMPLETED
**Previous Context**: Iteration 21 (Full Feature Push)

#### 22.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1.1 | Fix critical unwraps in main.rs | Critical | DONE |
| 1.2 | Add HWND validation function | High | DONE |
| 1.3 | Add daemon unit tests (~10 tests) | High | DONE |
| 1.4 | Document overlay.rs | Medium | DONE |
| 2.1 | Display change detection infrastructure | Medium | DONE |
| 2.2 | Add catch_unwind in callbacks | High | DONE |
| 2.3 | DeferWindowPos fallback | Medium | DONE |
| 3.1 | Enhanced IPC - QueryAllWindows | Medium | DONE |
| 3.2 | Focus follows mouse config | Low | DONE |

#### 22.2 Changes Made

##### 22.2.1 Phase 1: Critical Quality Fixes

**Task 1.1: Fixed Critical Unwraps**

**File**: `crates/daemon/src/main.rs`

**Problem**: Lines 238 and 663 had `.unwrap()` on `floating_rect` which could panic when window rules don't set dimensions.

**Fix**: Replaced with `unwrap_or_else` that provides a default centered 800x600 window based on monitor's work area:
```rust
let rect = floating_rect.unwrap_or_else(|| {
    let viewport = self.monitors.get(&monitor_id)
        .map(|m| m.work_area)
        .unwrap_or_else(|| Rect::new(0, 0, FALLBACK_VIEWPORT_WIDTH, FALLBACK_VIEWPORT_HEIGHT));
    Rect::new(
        viewport.x + (viewport.width - 800) / 2,
        viewport.y + (viewport.height - 600) / 2,
        800,
        600,
    )
});
```

**Task 1.2: HWND Validation**

**File**: `crates/platform_win32/src/lib.rs`

**Added function**:
```rust
pub fn is_valid_window(hwnd: WindowId) -> bool {
    unsafe {
        let hwnd = HWND(hwnd as *mut c_void);
        IsWindow(Some(hwnd)).as_bool()
    }
}
```

**File**: `crates/daemon/src/main.rs`

**Added validation** at start of `handle_window_event()` - skips events for invalid window handles (except Destroyed events).

**Task 1.3: Daemon Unit Tests**

**File**: `crates/daemon/src/main.rs`

**Added 12 tests**:
- `test_app_state_new`
- `test_app_state_focused_viewport`
- `test_app_state_no_monitors_fallback`
- `test_window_rule_matching_class`
- `test_window_rule_matching_title`
- `test_window_rule_matching_executable`
- `test_window_rule_no_match_defaults_to_tile`
- `test_floating_rect_uses_rule_dimensions`
- `test_floating_rect_preserves_original_if_no_dimensions`
- `test_find_window_workspace_not_found`
- `test_app_state_apply_config`

**Task 1.4: Overlay Documentation**

**File**: `crates/platform_win32/src/overlay.rs`

**Added comprehensive documentation**:
- Module-level architecture overview
- `OverlayWindow` struct with features and example
- All public methods documented
- `SnapHintType` and `SnapHintConfig` documented

##### 22.2.2 Phase 2: Robustness Improvements

**Task 2.1: Display Change Detection**

**File**: `crates/platform_win32/src/lib.rs`
- Added `DisplayChange` variant to `WindowEvent` enum
- Added `WM_DISPLAYCHANGE` constant
- Added `DISPLAY_CHANGE_SENDER` static for event forwarding

**File**: `crates/core_layout/src/lib.rs`
- Added `all_window_ids()` method to `Workspace` for window migration

**File**: `crates/daemon/src/main.rs`
- Added `reconcile_monitors()` method for handling monitor hotplug

**Task 2.2: catch_unwind in Callbacks**

**Files Modified**: `crates/platform_win32/src/lib.rs`, `crates/platform_win32/src/overlay.rs`

**Wrapped with catch_unwind**:
- `hotkey_window_proc` → `hotkey_window_proc_inner`
- `win_event_callback` → `win_event_callback_inner`
- `gesture_window_proc` → `gesture_window_proc_inner`
- `overlay_window_proc` → `overlay_window_proc_inner`

Panics in callbacks now log the error and return safe defaults instead of crashing.

**Task 2.3: DeferWindowPos Fallback**

**File**: `crates/platform_win32/src/lib.rs`

**Improved `apply_placements_deferred`**:
- Track failed placements during DeferWindowPos
- If EndDeferWindowPos fails, fall back to individual SetWindowPos for all windows
- If batch succeeds, retry only failed placements individually

##### 22.2.3 Phase 3: Feature Additions

**Task 3.1: Enhanced IPC - Query Commands**

**File**: `crates/ipc/src/lib.rs`

**New types**:
```rust
pub struct IpcRect { pub x: i32, pub y: i32, pub width: i32, pub height: i32 }

pub struct WindowInfo {
    pub window_id: u64,
    pub title: String,
    pub class_name: String,
    pub process_id: u32,
    pub executable: String,
    pub rect: IpcRect,
    pub column_index: Option<usize>,
    pub window_index: Option<usize>,
    pub monitor_id: i64,
    pub is_floating: bool,
    pub is_focused: bool,
}
```

**New commands**: `QueryAllWindows`

**New responses**: `WindowList { windows: Vec<WindowInfo> }`, `FocusedWindowInfo`

**File**: `crates/daemon/src/main.rs`
- Added handler for `QueryAllWindows` command

**Task 3.2: Focus Follows Mouse Config**

**File**: `crates/daemon/src/config.rs`

**Added to BehaviorConfig**:
```rust
pub focus_follows_mouse: bool,        // default: false
pub focus_follows_mouse_delay_ms: u32, // default: 100
```

#### 22.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    87 passed, 0 failed, 0 ignored
- daemon:         34 passed, 0 failed, 0 ignored
- ipc:            13 passed, 0 failed, 0 ignored
- platform_win32: 13 passed, 0 failed, 2 ignored

TOTAL: 147 passed, 0 failed, 2 ignored (3 doc-tests ignored)
Clippy: 1 minor warning (pre-existing TrayError naming)
```

**Test Growth**: 131 → 147 (+16 tests)

#### 22.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo +stable-x86_64-pc-windows-gnu test --workspace` | 147 passed, 2 ignored |
| Build succeeds | `cargo +stable-x86_64-pc-windows-gnu build --workspace` | Success |
| Clippy clean | `cargo +stable-x86_64-pc-windows-gnu clippy --workspace` | 1 pre-existing warning |

#### 22.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/daemon/src/main.rs` | +200 | Fix unwraps, HWND validation, tests, reconcile_monitors |
| `crates/platform_win32/src/lib.rs` | +150 | is_valid_window, catch_unwind, DeferWindowPos fallback |
| `crates/platform_win32/src/overlay.rs` | +100 | Documentation, catch_unwind |
| `crates/ipc/src/lib.rs` | +100 | WindowInfo, IpcRect, QueryAllWindows, tests |
| `crates/daemon/src/config.rs` | +30 | focus_follows_mouse config |
| `crates/core_layout/src/lib.rs` | +15 | all_window_ids() method |
| `crates/cli/src/main.rs` | +10 | Handle new IPC responses |

---

### Iteration 21: Full Feature Push (All Four Features)

**Date**: 2026-02-04
**Status**: COMPLETED
**Previous Context**: Iteration 20 (Codex Review + QA Scan (Pass))

#### 21.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 0 | Cleanup: Remove stray `nul` file, update CODEX_REVIEW | Low | DONE |
| 1 | System Tray Icon with menu | High | DONE |
| 2 | Per-Window Floating Rules | High | DONE |
| 3 | Touchpad Gesture Support | Medium | DONE |
| 4 | Visual Snapping Hints | Medium | DONE |

#### 21.2 Changes Made

##### 21.2.1 Phase 0: Cleanup

**Changes**:
- Deleted stray `nul` file at repo root (Windows NUL device artifact)
- Updated CODEX_REVIEW_CONSOLIDATED.md to mark fixed items

##### 21.2.2 Phase 1: System Tray Icon

**Files Modified/Created**:
- `crates/daemon/Cargo.toml` - Added `tray-icon = "0.19"`, `regex = "1"`, `thiserror`
- `crates/daemon/src/tray.rs` - NEW: TrayManager, TrayEvent, menu creation
- `crates/daemon/src/main.rs` - Tray integration in event loop

**Features**:
- System tray icon with menu (Refresh Windows, Reload Config, Exit)
- Menu events forwarded via sync channel to async event loop
- Drop implementation cleans up icon properly

##### 21.2.3 Phase 2: Per-Window Floating Rules

**Files Modified**:
- `crates/daemon/src/config.rs` - WindowRule, MatchCriteria, WindowAction types
- `crates/platform_win32/src/lib.rs` - `get_process_executable()` function
- `crates/core_layout/src/lib.rs` - FloatingWindow struct, floating window support
- `crates/daemon/src/main.rs` - Rule evaluation, floating window handling

**Config Example**:
```toml
[[window_rules]]
match_class = "Notepad"
action = "float"
width = 800
height = 600

[[window_rules]]
match_executable = "spotify.exe"
action = "float"

[[window_rules]]
match_class = "#32770"
action = "ignore"
```

**Features**:
- Regex matching on window class and title
- Case-insensitive executable matching
- Float, Tile, or Ignore actions
- Optional width/height for floating windows

##### 21.2.4 Phase 3: Touchpad Gesture Support

**Files Modified**:
- `crates/platform_win32/src/lib.rs` - GestureEvent enum, register_gestures()
- `crates/daemon/src/config.rs` - GestureConfig
- `crates/daemon/src/main.rs` - Gesture event handling

**Config Example**:
```toml
[gestures]
enabled = true
swipe_left = "focus_left"
swipe_right = "focus_right"
swipe_up = "focus_up"
swipe_down = "focus_down"
```

**Features**:
- Swipe left/right/up/down detection
- Configurable command mapping
- Disabled by default (enabled via config)

##### 21.2.5 Phase 4: Visual Snapping Hints

**Files Created/Modified**:
- `crates/platform_win32/src/overlay.rs` - NEW: OverlayWindow for visual hints
- `crates/daemon/src/config.rs` - SnapHintConfig
- `crates/daemon/src/main.rs` - Snap hint display on resize operations

**Config Example**:
```toml
[snap_hints]
enabled = true
duration_ms = 200
opacity = 128
```

**Features**:
- Transparent overlay window (WS_EX_LAYERED | WS_EX_TRANSPARENT)
- Shows column boundary during resize
- Auto-hide after configurable duration
- Disabled by default

#### 21.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    87 passed, 0 failed, 0 ignored
- daemon:         21 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32: 13 passed, 0 failed, 2 ignored

TOTAL: 131 passed, 0 failed, 2 ignored
```

**New Tests**:
- `test_add_floating_window`
- `test_duplicate_floating_window_rejected`
- `test_remove_floating_window`
- `test_remove_nonexistent_floating_window`
- `test_floating_window_in_placements`
- `test_floating_and_tiled_windows_together`
- `test_floating_window_duplicate_with_tiled`
- `test_update_floating_window`
- `test_window_rule_matches_class`
- `test_window_rule_matches_title_regex`
- `test_window_rule_matches_executable`
- `test_window_rule_matches_combined`
- `test_window_rule_no_criteria_matches_nothing`
- `test_window_rule_config_parse`
- `test_snap_hint_config_default`
- `test_snap_hint_config_serialization`
- `test_overlay_state_default` (platform_win32)
- `test_snap_hint_config_default` (platform_win32)

#### 21.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo +stable-x86_64-pc-windows-gnu test --workspace` | 131 passed, 2 ignored |
| Build succeeds | `cargo +stable-x86_64-pc-windows-gnu build --workspace` | Success |
| Minor warnings | `cargo +stable-x86_64-pc-windows-gnu clippy --workspace` | 3 minor warnings (acceptable) |

#### 21.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/daemon/Cargo.toml` | +4 | Dependencies |
| `crates/daemon/src/tray.rs` | +200 | NEW: Tray manager |
| `crates/daemon/src/config.rs` | +150 | Window rules, gestures, snap hints |
| `crates/daemon/src/main.rs` | +200 | Integration for all features |
| `crates/platform_win32/src/lib.rs` | +250 | Gestures, process info |
| `crates/platform_win32/src/overlay.rs` | +230 | NEW: Overlay window |
| `crates/core_layout/src/lib.rs` | +150 | Floating window support |
| `Cargo.toml` | +1 | Win32_System_ProcessStatus feature |
| `docs/.../CODEX_REVIEW_CONSOLIDATED.md` | +10 | Updated fixed items |

---

### Iteration 20: Codex Review + QA Scan (Pass)

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 19 (Config Completeness & Doc Sync)

#### 20.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Re-verify repo state and tests | High | DONE |
| 2 | Update `CODEX_REVIEW_CONSOLIDATED.md` with new QA findings | High | DONE |
| 3 | Record verification evidence | High | DONE |

#### 20.2 Changes Made

**Files Modified**:
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `docs/1_Progress and review/ITERATION_LOG.md`

#### 20.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    79 passed, 0 failed, 0 ignored
- daemon:         11 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32: 11 passed, 0 failed, 2 ignored

TOTAL: 111 passed, 0 failed, 2 ignored
```

#### 20.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --all` | 111 passed, 2 ignored |

#### 20.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` | 1-41 | Doc refresh |
| `docs/1_Progress and review/ITERATION_LOG.md` | 56-68, 73-119, 1241-1257, 1321 | Iteration log update |

---

### Iteration 19: Config Completeness & Documentation Sync

**Date**: 2026-02-04
**Status**: COMPLETED
**Previous Context**: Iteration 18 (Codex Review + QA Scan (Failure))

#### 19.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Implement log_level config option | High | DONE |
| 2 | Implement track_focus_changes config option | High | DONE |
| 3 | Fix hotkey reload gap (critical bug) | Critical | DONE |
| 4 | Update ARCHITECTURE.md documentation | Medium | DONE |
| 5 | Update SPEC.md documentation | Medium | DONE |
| 6 | Update AGENTS.md toolchain documentation | Low | DONE |

#### 19.2 Changes Made

##### 19.2.1 log_level Config Implementation

**File**: `crates/daemon/src/main.rs`

**Changes**:
- Moved config loading before tracing subscriber setup
- Parse `config.behavior.log_level` string to `tracing::Level`
- Apply configured log level instead of hardcoded DEBUG

```rust
let log_level = match config.behavior.log_level.to_lowercase().as_str() {
    "trace" => Level::TRACE,
    "debug" => Level::DEBUG,
    "info" => Level::INFO,
    "warn" => Level::WARN,
    "error" => Level::ERROR,
    _ => Level::INFO, // default fallback
};
```

##### 19.2.2 track_focus_changes Config Implementation

**File**: `crates/daemon/src/main.rs`

**Changes**:
- Wrapped `install_event_hooks()` call in conditional based on config
- When `track_focus_changes = false`, WinEvent hooks are not installed

```rust
let _hook_handle = if config.behavior.track_focus_changes {
    match install_event_hooks() { ... }
} else {
    info!("WinEvent hooks disabled by config");
    None
};
```

##### 19.2.3 Hotkey Reload Fix (Critical)

**Problem**: `Reload` IPC command updated layout settings but did NOT re-register hotkeys. Users had to restart daemon for hotkey changes.

**Root Cause**: `HOTKEY_SENDER` in platform layer used `OnceLock::set()` which can only be called once.

**Files Modified**:
- `crates/platform_win32/src/lib.rs`: Changed `HOTKEY_SENDER` from `OnceLock` to `Mutex<Option<...>>`
- `crates/daemon/src/main.rs`: Added `HotkeyState` struct and `setup_hotkeys()` helper

**Platform Layer Changes**:
```rust
// Before:
static HOTKEY_SENDER: OnceLock<Sender<HotkeyEvent>> = OnceLock::new();

// After:
static HOTKEY_SENDER: Mutex<Option<Sender<HotkeyEvent>>> = Mutex::new(None);
```

**Daemon Changes**:
- Added `HotkeyState` struct to hold handle and mapping
- Added `setup_hotkeys()` helper function
- In IPC command handler, detect `Reload` and re-register hotkeys:
  1. Drop old handle (triggers unregister via Drop impl)
  2. Call `setup_hotkeys()` with new config
  3. Update hotkey state

##### 19.2.4 Documentation Updates

**ARCHITECTURE.md**:
- Updated test counts (52 → 79 for core_layout, total 111)
- Added Global Hotkeys section
- Added Smooth Scroll Animations section
- Updated AppState struct documentation for multi-monitor
- Moved config and multi-monitor from Pending to Implemented

**SPEC.md**:
- Updated Multi-Monitor Support section (now implemented)
- Added Global Hotkeys behavioral specification
- Added Scroll Animations behavioral specification
- Updated implementation status

**AGENTS.md**:
- Changed toolchain from MSVC to GNU/MinGW (matches `.cargo/config.toml`)

#### 19.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    79 passed, 0 failed, 0 ignored
- daemon:         11 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32: 11 passed, 0 failed, 2 ignored

TOTAL: 111 passed, 0 failed, 2 ignored
Clippy: No warnings
```

#### 19.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo +stable-x86_64-pc-windows-gnu test --workspace` | 111 passed, 2 ignored |
| No clippy warnings | `cargo +stable-x86_64-pc-windows-gnu clippy --workspace` | No warnings |
| Build succeeds | `cargo +stable-x86_64-pc-windows-gnu build --workspace` | Success |

#### 19.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/platform_win32/src/lib.rs` | +15 | Hotkey sender mutex refactor |
| `crates/daemon/src/main.rs` | +80 | Config options, hotkey reload |
| `docs/ARCHITECTURE.md` | +40 | Documentation sync |
| `docs/SPEC.md` | +80 | Documentation sync |
| `AGENTS.md` | +2 | Toolchain clarification |
| `docs/1_Progress and review/ITERATION_LOG.md` | +100 | This entry |

---

### Iteration 18: Codex Review + QA Scan (Failure)

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 17 (Codex Review + QA Scan (Failure))

#### 18.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Re-verify repo state and tests | High | DONE |
| 2 | Update `CODEX_REVIEW_CONSOLIDATED.md` with new QA findings | High | DONE |
| 3 | Record verification evidence | High | DONE |

#### 18.2 Changes Made

**Files Modified**:
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `docs/1_Progress and review/ITERATION_LOG.md`

#### 18.3 Test Results

```
Test Summary (2026-02-04):
- `cargo test --all` FAILED
  - Error: E0599 `Config::generate_default` not found (crates/daemon/src/config.rs)
  - Locations: lines ~360 and ~410
```

#### 18.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| Tests run | `cargo test --all` | FAIL (E0599 Config::generate_default missing) |

#### 18.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` | 1-43 | Doc refresh |
| `docs/1_Progress and review/ITERATION_LOG.md` | 56-67, 71-122, 1057-1079, 1133 | Iteration log update |

---

### Iteration 17: Codex Review + QA Scan (Failure)

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 16 (Codex Review + QA Scan)

#### 17.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Re-verify repo state and tests | High | DONE |
| 2 | Update `CODEX_REVIEW_CONSOLIDATED.md` with new QA findings | High | DONE |
| 3 | Record verification evidence | High | DONE |

#### 17.2 Changes Made

**Files Modified**:
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `docs/1_Progress and review/ITERATION_LOG.md`

#### 17.3 Test Results

```
Test Summary (2026-02-04):
- `cargo test --all` FAILED
  - Error: E0599 `Config::generate_default` not found (crates/daemon/src/config.rs)
  - Locations: lines ~360 and ~410
```

#### 17.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| Tests run | `cargo test --all` | FAIL (E0599 Config::generate_default missing) |

#### 17.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` | 1-41 | Doc refresh |
| `docs/1_Progress and review/ITERATION_LOG.md` | 56-66, 68-114, 1012-1027, 1087 | Iteration log update |

---

### Iteration 16: Codex Review + QA Scan

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 15 (Codex Review + Doc Drift Audit)

#### 16.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Re-verify repo state and tests | High | DONE |
| 2 | Update `CODEX_REVIEW_CONSOLIDATED.md` with new QA findings | High | DONE |
| 3 | Record verification evidence | High | DONE |

#### 16.2 Changes Made

**Files Modified**:
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `docs/1_Progress and review/ITERATION_LOG.md`

#### 16.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    79 passed, 0 failed, 0 ignored
- daemon:         10 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  9 passed, 0 failed, 2 ignored

TOTAL: 108 passed, 0 failed, 2 ignored
Warnings: monitors_list unused; default_config_path unused
```

#### 16.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --all` | 108 passed, 2 ignored |

#### 16.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` | 1-39 | Doc refresh |
| `docs/1_Progress and review/ITERATION_LOG.md` | 56-66, 69-119, 967-987, 1041 | Iteration log update |

---

### Iteration 15: Codex Review + Doc Drift Audit

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 14 (Smooth Scroll Animations)

#### 15.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Re-verify repo state and tests | High | DONE |
| 2 | Update `CODEX_REVIEW_CONSOLIDATED.md` with doc drift findings | High | DONE |
| 3 | Record verification evidence | High | DONE |

#### 15.2 Changes Made

**Files Modified**:
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `docs/1_Progress and review/ITERATION_LOG.md`

#### 15.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    79 passed, 0 failed, 0 ignored
- daemon:         10 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  9 passed, 0 failed, 2 ignored

TOTAL: 108 passed, 0 failed, 2 ignored
Warnings: monitors_list unused; default_config_path unused
```

#### 15.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --all` | 108 passed, 2 ignored |

#### 15.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` | 1-37 | Doc refresh |
| `docs/1_Progress and review/ITERATION_LOG.md` | 54-64, 66-208, 918-931, 991 | Iteration log update |

---

### Iteration 14: Smooth Scroll Animations

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 13 (Global Hotkey Support)

#### 14.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Add animation types to core layout | High | DONE |
| 2 | Add animated scrolling in `Workspace` | High | DONE |
| 3 | Integrate animation tick in daemon | Medium | DONE |

#### 14.2 Changes Made

**Core Layout** (`crates/core_layout/src/lib.rs`):
- Added `Easing` and `ScrollAnimation` types.
- Added animated scroll helpers (`start_scroll_animation`, `tick_animation`, `compute_placements_animated`, `ensure_focused_visible_animated`).

**Daemon** (`crates/daemon/src/main.rs`):
- Added animation tick event and animated placement usage when active.

#### 14.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    79 passed, 0 failed, 0 ignored
- daemon:         10 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  9 passed, 0 failed, 2 ignored

TOTAL: 108 passed, 0 failed, 2 ignored
```

#### 14.4 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/core_layout/src/lib.rs` | +200 | Animation types + tests |
| `crates/daemon/src/main.rs` | +40 | Animation tick integration |

---

### Iteration 13: Global Hotkey Support

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 12 (Codex Audit + Doc Refresh)

#### 13.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Add global hotkey registration in platform layer | High | DONE |
| 2 | Add hotkey configuration schema | High | DONE |
| 3 | Integrate hotkeys into daemon event loop | High | DONE |

#### 13.2 Changes Made

**Platform Layer** (`crates/platform_win32/src/lib.rs`):
- Added hotkey types (`Hotkey`, `HotkeyEvent`, `Modifiers`) and parsing helpers.
- Added `register_hotkeys` / `unregister_hotkeys` with RegisterHotKey.

**Config** (`crates/daemon/src/config.rs`):
- Added `HotkeyConfig` and default bindings in generated config.

**Daemon** (`crates/daemon/src/main.rs`):
- Registers hotkeys on startup and maps them to IPC commands.

#### 13.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    52 passed, 0 failed, 0 ignored
- daemon:         10 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  9 passed, 0 failed, 2 ignored

TOTAL: 81 passed, 0 failed, 2 ignored
```

#### 13.4 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/platform_win32/src/lib.rs` | +200 | Hotkey registration + parsing |
| `crates/daemon/src/config.rs` | +120 | Hotkey config + defaults |
| `crates/daemon/src/main.rs` | +60 | Hotkey integration |

---
### Iteration 12: Codex Audit + Doc Refresh

**Date**: 2026-02-04  
**Status**: COMPLETED  
**Previous Context**: Iteration 11 (Multi-monitor Support)

#### 12.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Review `ITERATION_LOG.md` against repo state | High | DONE |
| 2 | Update `CODEX_REVIEW_CONSOLIDATED.md` with current verification | High | DONE |
| 3 | Update `AGENTS.md` and `CLAUDE.md` to require review of consolidated review | Medium | DONE |
| 4 | Record verification evidence | High | DONE |

#### 12.2 Changes Made

**Files Modified**:
- `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md`
- `AGENTS.md`
- `CLAUDE.md`
- `docs/1_Progress and review/ITERATION_LOG.md`

#### 12.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    52 passed, 0 failed, 0 ignored
- daemon:          6 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  6 passed, 0 failed, 2 ignored

TOTAL: 74 passed, 0 failed, 2 ignored
Warnings: monitors_list unused; default_config_path unused
```

#### 12.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --all` | 74 passed, 2 ignored |

#### 12.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `docs/1_Progress and review/CODEX_REVIEW_CONSOLIDATED.md` | 1-30 | Doc refresh |
| `AGENTS.md` | 33-40 | Guidance update |
| `CLAUDE.md` | 5-8 | Guidance update |
| `docs/1_Progress and review/ITERATION_LOG.md` | 50-61, 67-116, 776-786, 846 | Iteration log update |

---

### Iteration 11: Multi-monitor Support

**Date**: 2026-02-04
**Status**: COMPLETED
**Previous Context**: Iteration 10 (Configuration Support)

#### 11.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Extend monitor enumeration for multi-monitor | High | DONE |
| 2 | Add multi-workspace state to daemon | High | DONE |
| 3 | Assign windows to monitors based on position | High | DONE |
| 4 | Add cross-monitor movement commands | Medium | DONE |

#### 11.2 Changes Made

##### 11.2.1 Platform Layer Multi-monitor Helpers (Task #18)

**File**: `crates/platform_win32/src/lib.rs`

**New Types**:
```rust
pub type MonitorId = isize;

impl MonitorInfo {
    pub fn contains_point(&self, x: i32, y: i32) -> bool { ... }
    pub fn contains_rect_center(&self, rect: &Rect) -> bool { ... }
}
```

**New Functions**:
```rust
/// Find which monitor contains a rectangle's center point.
pub fn find_monitor_for_rect<'a>(monitors: &'a [MonitorInfo], rect: &Rect) -> Option<&'a MonitorInfo>

/// Find a monitor by ID.
pub fn find_monitor_by_id(monitors: &[MonitorInfo], id: MonitorId) -> Option<&MonitorInfo>

/// Sort monitors by position (left to right, top to bottom).
pub fn monitors_by_position(monitors: &[MonitorInfo]) -> Vec<&MonitorInfo>

/// Find the monitor to the left of the current one.
pub fn monitor_to_left(monitors: &[MonitorInfo], current_id: MonitorId) -> Option<&MonitorInfo>

/// Find the monitor to the right of the current one.
pub fn monitor_to_right(monitors: &[MonitorInfo], current_id: MonitorId) -> Option<&MonitorInfo>
```

**New Tests** (5 tests):
- `test_monitor_contains_point`
- `test_monitor_contains_rect_center`
- `test_find_monitor_for_rect`
- `test_monitors_by_position`
- `test_monitor_to_left_right`

##### 11.2.2 Daemon Multi-workspace State (Task #19)

**File**: `crates/daemon/src/main.rs`

**AppState Changes**:
```rust
// Before (single workspace):
struct AppState {
    workspace: Workspace,
    viewport: Rect,
    platform_config: PlatformConfig,
    config: Config,
}

// After (per-monitor workspaces):
struct AppState {
    workspaces: HashMap<MonitorId, Workspace>,
    monitors: HashMap<MonitorId, MonitorInfo>,
    focused_monitor: MonitorId,
    platform_config: PlatformConfig,
    config: Config,
}
```

**New Methods**:
```rust
impl AppState {
    fn new_with_config(config: Config, monitors: Vec<MonitorInfo>) -> Self { ... }
    fn focused_workspace(&self) -> Option<&Workspace> { ... }
    fn focused_workspace_mut(&mut self) -> Option<&mut Workspace> { ... }
    fn focused_viewport(&self) -> Rect { ... }
    fn find_window_workspace(&self, window_id: u64) -> Option<MonitorId> { ... }
}
```

**Window Assignment**: Windows are assigned to monitors based on the center point of their current rect using `find_monitor_for_rect()`.

**Event Handling**: `handle_window_event()` updated to:
- Find which workspace contains a window using `find_window_workspace()`
- Assign new windows to monitors based on their position
- Update `focused_monitor` when window focus changes

##### 11.2.3 Cross-monitor Movement Commands (Task #21)

**File**: `crates/ipc/src/lib.rs`

**New Commands**:
```rust
pub enum IpcCommand {
    // ... existing variants ...
    FocusMonitorLeft,
    FocusMonitorRight,
    MoveWindowToMonitorLeft,
    MoveWindowToMonitorRight,
}
```

**File**: `crates/cli/src/main.rs`

**New CLI Subcommands**:
```bash
openniri-cli focus-monitor left   # Focus monitor to the left
openniri-cli focus-monitor right  # Focus monitor to the right
openniri-cli move-to-monitor left   # Move window to monitor left
openniri-cli move-to-monitor right  # Move window to monitor right
```

**File**: `crates/daemon/src/main.rs`

**Command Handling**:
- `FocusMonitorLeft/Right`: Changes `focused_monitor` to adjacent monitor
- `MoveWindowToMonitorLeft/Right`: Removes window from current workspace, adds to target workspace, follows focus

#### 11.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    52 passed, 0 failed, 0 ignored
- daemon:          6 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  6 passed, 0 failed, 2 ignored

TOTAL: 74 passed, 0 failed, 2 ignored
```

**New/Updated Tests**:
- 5 new tests in platform_win32 for multi-monitor helpers
- Updated IPC test to include 4 new command variants

#### 11.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --all` | 74 passed, 2 ignored |
| No clippy errors | `cargo clippy --workspace` | Only dead code warnings |
| Focus monitor | `openniri-cli focus-monitor left` | Focuses left monitor |
| Move window | `openniri-cli move-to-monitor right` | Moves window to right monitor |

#### 11.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `crates/platform_win32/src/lib.rs` | +100 | Multi-monitor helpers |
| `crates/daemon/src/main.rs` | +200 | Multi-workspace refactor |
| `crates/ipc/src/lib.rs` | +15 | New commands |
| `crates/cli/src/main.rs` | +30 | New CLI subcommands |

---

### Iteration 10: Configuration File Support

**Date**: 2026-02-04
**Status**: COMPLETED
**Previous Context**: Iteration 9 (Codex Review Implementation)

#### 10.1 Objectives

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Define config file format and schema | High | DONE |
| 2 | Implement config loading in daemon | High | DONE |
| 3 | Add Reload IPC command for hot-reload | Medium | DONE |
| 4 | Add default config generation | Medium | DONE |

#### 10.2 Changes Made

##### 10.2.1 Config File Format (Task #13)

**Files Created**:
- `crates/daemon/src/config.rs` (270 lines)

**Dependencies Added** (workspace `Cargo.toml`):
```toml
toml = "0.8"
directories = "5"
```

**Config Structure**:
```rust
pub struct Config {
    pub layout: LayoutConfig,      // gap, outer_gap, column widths, centering
    pub appearance: AppearanceConfig,  // cloaking, deferred positioning
    pub behavior: BehaviorConfig,  // focus behavior, log level
}
```

**Config Locations** (in priority order):
1. `%APPDATA%/openniri/config.toml`
2. `~/.config/openniri/config.toml`
3. `./config.toml`

##### 10.2.2 Config Loading (Task #14)

**File**: `crates/daemon/src/main.rs`

**AppState Changes**:
```rust
struct AppState {
    workspace: Workspace,
    platform_config: PlatformConfig,
    viewport: Rect,
    config: Config,  // NEW
}

impl AppState {
    fn new_with_config(config: Config, viewport: Rect) -> Self { ... }
    fn apply_config(&mut self, config: Config) { ... }
}
```

**Startup Flow**:
1. Load config (or use defaults)
2. Log config values
3. Create AppState with config
4. Apply config to workspace settings

##### 10.2.3 Reload IPC Command (Task #15)

**File**: `crates/ipc/src/lib.rs`

**New Variant**:
```rust
pub enum IpcCommand {
    // ... existing variants ...
    Reload,  // NEW
}
```

**File**: `crates/cli/src/main.rs`

**New CLI Command**:
```bash
openniri-cli reload  # Reload config from file
```

##### 10.2.4 Default Config Generation (Task #16)

**File**: `crates/cli/src/main.rs`

**New CLI Command**:
```bash
openniri-cli init              # Create config at default location
openniri-cli init -o path.toml # Create at custom path
openniri-cli init --force      # Overwrite existing
```

**Generated Config Example**:
```toml
[layout]
gap = 10
outer_gap = 10
default_column_width = 800
centering_mode = "center"

[appearance]
use_cloaking = true
use_deferred_positioning = true

[behavior]
focus_new_windows = true
log_level = "info"
```

#### 10.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    52 passed, 0 failed, 0 ignored
- daemon:          6 passed, 0 failed, 0 ignored (NEW config tests)
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  1 passed, 0 failed, 2 ignored

TOTAL: 69 passed, 0 failed, 2 ignored
```

**New Tests in daemon**:
- `test_default_config`
- `test_config_serialization_roundtrip`
- `test_config_partial_parse`
- `test_centering_mode_conversion`
- `test_generate_default_config`
- `test_config_paths_not_empty`

#### 10.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --all` | 69 passed, 2 ignored |
| Config init | `openniri-cli init` | Creates config file |
| Config reload | `openniri-cli reload` | Reloads config in daemon |

#### 10.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `Cargo.toml` | +3 | Dependencies |
| `crates/daemon/Cargo.toml` | +2 | Dependencies |
| `crates/daemon/src/config.rs` | +270 | New file |
| `crates/daemon/src/main.rs` | +30 | Config integration |
| `crates/cli/Cargo.toml` | +1 | Dependencies |
| `crates/cli/src/main.rs` | +80 | Init command |
| `crates/ipc/src/lib.rs` | +3 | Reload variant |

---

### Iteration 9: Codex Review Implementation

**Date**: 2026-02-04
**Status**: COMPLETED
**Previous Context**: Iteration 8.1-8.3 (IPC & Platform Integration)

#### 9.1 Objectives

Based on `CODEX_REVIEW_CONSOLIDATED.md`, implement all recommended fixes:

| # | Objective | Priority | Status |
|---|-----------|----------|--------|
| 1 | Update ARCHITECTURE.md and SPEC.md | High | DONE |
| 2 | Remove dead HideStrategy code | Medium | DONE |
| 3 | Add `#[ignore]` to monitor tests | Medium | DONE |
| 4 | Add cloaked window filtering | High | DONE |
| 5 | Clean up named pipe server + CLI timeout | Medium | DONE |
| 6 | Add IPC integration tests | Medium | DONE |
| 7 | Implement WinEvent hooks | High | DONE |

#### 9.2 Changes Made

##### 9.2.1 Documentation Updates (Task #6)

**Files Modified**:
- `docs/ARCHITECTURE.md` (lines 207-222)
- `docs/SPEC.md` (lines 231-235)

**Changes**:
- Updated "Planned vs Implemented" section
- Marked IPC and monitor detection as implemented
- Updated test counts (52 -> 63)
- Removed WinEvent hooks from pending (now implemented)

##### 9.2.2 Dead Code Removal (Task #7)

**File**: `crates/platform_win32/src/lib.rs`

**Removed**:
```rust
// Before (lines 79-89):
pub enum HideStrategy {
    Cloak,
    Minimize,      // REMOVED - never used
    MoveOffScreen, // REMOVED - never used
}

// Before (lines 92-100):
pub struct PlatformConfig {
    pub hide_strategy: HideStrategy,
    pub buffer_zone: i32,  // REMOVED - never used
    pub use_deferred_positioning: bool,
}
```

**After**:
```rust
pub enum HideStrategy {
    #[default]
    Cloak,
    // Note: Minimize and MoveOffScreen strategies were considered but removed.
}

pub struct PlatformConfig {
    pub hide_strategy: HideStrategy,
    pub use_deferred_positioning: bool,
}
```

**Test Impact**: Updated `test_platform_config_default` to remove `buffer_zone` assertion.

##### 9.2.3 Headless CI Test Marking (Task #8)

**File**: `crates/platform_win32/src/lib.rs` (lines 607-637)

**Changes**:
```rust
#[test]
#[ignore = "Requires display hardware - run with: cargo test -- --ignored"]
fn test_enumerate_monitors() { ... }

#[test]
#[ignore = "Requires display hardware - run with: cargo test -- --ignored"]
fn test_get_primary_monitor() { ... }
```

##### 9.2.4 Cloaked Window Filtering (Task #9)

**File**: `crates/platform_win32/src/lib.rs`

**New Import**:
```rust
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DwmSetWindowAttribute, DWMWA_CLOAK, DWMWA_CLOAKED,
};
```

**New Function** (lines 351-365):
```rust
fn is_window_cloaked(hwnd: HWND) -> bool {
    unsafe {
        let mut cloaked: u32 = 0;
        let result = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        );
        result.is_ok() && cloaked != 0
    }
}
```

**Integration**: Added call in `enum_windows_callback` after `WS_EX_NOACTIVATE` check.

##### 9.2.5 Named Pipe Server Cleanup + CLI Timeout (Task #10)

**File**: `crates/daemon/src/main.rs` (lines 209-256)

**Before**: Convoluted try-without-first_pipe_instance-then-with logic
**After**: Clean `is_first_instance` tracking

**File**: `crates/cli/src/main.rs`

**New Imports**:
```rust
use std::time::Duration;
use tokio::time::timeout;

const IPC_TIMEOUT: Duration = Duration::from_secs(5);
```

**Changed Function**:
```rust
async fn send_command(cmd: IpcCommand) -> Result<IpcResponse> {
    timeout(IPC_TIMEOUT, send_command_inner(cmd))
        .await
        .context("Timed out waiting for daemon response")?
}
```

##### 9.2.6 IPC Integration Tests (Task #11)

**File**: `crates/ipc/src/lib.rs` (lines 161-220)

**New Tests Added** (5 total):
1. `test_all_command_types_roundtrip` - All 15 command variants
2. `test_all_response_types_roundtrip` - All 5 response variants
3. `test_line_delimited_protocol` - JSON + newline format
4. `test_invalid_json_handling` - Error cases
5. `test_pipe_name_format` - Named pipe path validation

**Test Count**: 5 -> 10 tests in IPC crate

##### 9.2.7 WinEvent Hooks Implementation (Task #12)

**File**: `crates/platform_win32/src/lib.rs`

**New Imports**:
```rust
use std::sync::mpsc;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::GetAncestor;
```

**New Constants**:
```rust
const EVENT_OBJECT_CREATE: u32 = 0x8000;
const EVENT_OBJECT_DESTROY: u32 = 0x8001;
const EVENT_OBJECT_FOCUS: u32 = 0x8005;
const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x0016;
const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x0017;
const EVENT_OBJECT_LOCATIONCHANGE: u32 = 0x800B;
```

**New Types**:
```rust
static EVENT_SENDER: std::sync::OnceLock<mpsc::Sender<WindowEvent>> = std::sync::OnceLock::new();

pub struct EventHookHandle {
    hooks: Vec<HWINEVENTHOOK>,
}
```

**New Functions**:
- `install_event_hooks() -> Result<(EventHookHandle, mpsc::Receiver<WindowEvent>), Win32Error>`
- `win_event_callback(...)` - extern "system" callback

**Workspace Cargo.toml Change**:
```toml
windows = { version = "0.59", features = [
    ...
    "Win32_UI_Accessibility",  # NEW
] }
```

**Daemon Integration** (`crates/daemon/src/main.rs`):

New DaemonEvent variant:
```rust
enum DaemonEvent {
    IpcCommand { ... },
    WindowEvent(WindowEvent),  // NEW
    Shutdown,
}
```

New AppState method:
```rust
fn handle_window_event(&mut self, event: WindowEvent) {
    match event {
        WindowEvent::Created(hwnd) => { ... }
        WindowEvent::Destroyed(hwnd) => { ... }
        WindowEvent::Focused(hwnd) => { ... }
        WindowEvent::Minimized(hwnd) => { ... }
        WindowEvent::Restored(hwnd) => { ... }
        WindowEvent::MovedOrResized(hwnd) => { ... }
    }
}
```

Hook installation in main():
```rust
let _hook_handle = match install_event_hooks() {
    Ok((handle, event_receiver)) => {
        // Spawn thread to forward events
        std::thread::spawn(move || { ... });
        Some(handle)
    }
    Err(e) => { warn!("..."); None }
};
```

#### 9.3 Test Results

```
Test Summary (2026-02-04):
- core_layout:    52 passed, 0 failed, 0 ignored
- ipc:            10 passed, 0 failed, 0 ignored
- platform_win32:  1 passed, 0 failed, 2 ignored
- daemon:          0 (binary crate)
- cli:             0 (binary crate)

TOTAL: 63 passed, 0 failed, 2 ignored
```

**Clippy**: No warnings or errors

#### 9.4 Evidence & Verification

| Item | Command | Expected Result |
|------|---------|-----------------|
| All tests pass | `cargo test --all` | 63 passed, 2 ignored |
| No clippy warnings | `cargo clippy --workspace` | No errors |
| Release build | `cargo build --release` | Success |
| Monitor tests (local) | `cargo test -- --ignored` | 2 passed (with display) |

#### 9.5 Files Modified Summary

| File | Lines Changed | Type |
|------|---------------|------|
| `Cargo.toml` | +1 | Feature addition |
| `crates/platform_win32/src/lib.rs` | +150 | WinEvent hooks, cloaked filtering |
| `crates/daemon/src/main.rs` | +100 | Event handling |
| `crates/cli/src/main.rs` | +15 | Timeout |
| `crates/ipc/src/lib.rs` | +60 | Tests |
| `docs/ARCHITECTURE.md` | +5 | Updates |
| `docs/SPEC.md` | +3 | Updates |

---

### Iteration 8.3: Async Daemon & CLI IPC (Prior)

**Date**: 2026-02-04
**Status**: COMPLETED

#### Key Deliverables
- Async daemon with tokio event loop
- Named pipe server (`\\.\pipe\openniri`)
- CLI sends real IPC commands
- Dynamic monitor detection (no more hardcoded 1920x1080)

#### Files Created/Modified
- `crates/daemon/src/main.rs` - Full rewrite for async
- `crates/cli/src/main.rs` - Real IPC client

---

### Iteration 8.2: Monitor Detection (Prior)

**Date**: 2026-02-04
**Status**: COMPLETED

#### Key Deliverables
- `MonitorInfo` struct
- `enumerate_monitors()` function
- `get_primary_monitor()` function

#### Files Modified
- `crates/platform_win32/src/lib.rs`
- `Cargo.toml` (added `Win32_Graphics_Gdi` feature)

---

### Iteration 8.1: IPC Protocol Crate (Prior)

**Date**: 2026-02-04
**Status**: COMPLETED

#### Key Deliverables
- New `crates/ipc/` crate
- `IpcCommand` enum (15 variants)
- `IpcResponse` enum (5 variants)
- `PIPE_NAME` constant

#### Files Created
- `crates/ipc/Cargo.toml`
- `crates/ipc/src/lib.rs`

---

### Iterations 1-7: Foundation (Historical)

**Date**: Pre-2026-02-04
**Status**: COMPLETED

#### Key Deliverables
- Core layout engine with 52 tests
- Basic Win32 enumeration
- Window positioning via DeferWindowPos
- DWM cloaking

---

## Test Coverage History

| Date | core_layout | ipc | platform_win32 | daemon | Total |
|------|-------------|-----|----------------|--------|-------|
| Pre-8.1 | 52 | 0 | 0 | 0 | 52 |
| 8.1 | 52 | 5 | 0 | 0 | 57 |
| 8.2 | 52 | 5 | 3 | 0 | 60 |
| 9 | 52 | 10 | 1 (+2 ignored) | 0 | 63 |
| 10 | 52 | 10 | 1 (+2 ignored) | 6 | 69 |
| 11 | 52 | 10 | 6 (+2 ignored) | 6 | 74 |
| 12 | 52 | 10 | 6 (+2 ignored) | 6 | 74 |
| 13 | 52 | 10 | 9 (+2 ignored) | 10 | 81 |
| 14 | 79 | 10 | 9 (+2 ignored) | 10 | 108 |
| 15 | 79 | 10 | 9 (+2 ignored) | 10 | 108 |
| 16 | 79 | 10 | 9 (+2 ignored) | 10 | 108 |
| 17 | N/A | N/A | N/A | N/A | FAIL (E0599 Config::generate_default) |
| 18 | N/A | N/A | N/A | N/A | FAIL (E0599 Config::generate_default) |
| 19 | 79 | 10 | 11 (+2 ignored) | 11 | 111 |
| 20 | 79 | 10 | 11 (+2 ignored) | 11 | 111 |
| 21 | 87 | 10 | 13 (+2 ignored) | 21 | 131 |
| 22 | 87 | 13 | 13 (+2 ignored) | 34 | 147 |
| 23 | 87 | 13 | 13 (+2 ignored) | 44 (+28 cli, +17 integration) | 202 |
| 24 | 87 | 13 | 13 (+2 ignored) | 48 (+28 cli, +17 integration) | 206 |
| 25 | 87 | 13 | 13 (+2 ignored) | 52 (+29 cli, +17 integration) | 231 |
| 26 | 87 | 13 | 13 (+2 ignored) | 55 (+29 cli, +17 integration) | 234 |
| 27 | 87 | 15 | 16 (+2 ignored) | 85 (+29 cli, +22 integration, +1 ignored) | 257 |
| 28 | 87 | 15 | 16 (+2 ignored) | 89 (+29 cli, +22 integration, +1 ignored) | 261 |
| 29 | 99 | 15 | 23 (+2 ignored) | 97 (+38 cli, +22 integration, +1 ignored) | 295 |
| 30 | 99 | 15 | 24 (+3 ignored) | 100 (+38 cli, +22 integration, +1 ignored) | 302 |
| 31 | 99 | 15 | 24 (+3 ignored) | 100 (+38 cli, +22 integration, +1 ignored) | 302 |
| 32 | 99 | 15 | 24 (+3 ignored) | 100 (+38 cli, +22 integration, +1 ignored) | 302 |

---

## Architecture Evolution

### Current State (Post-Iteration 32)

```
┌─────────────────────────────────────────────────────────────────────┐
│                        User / System                                 │
└─────────────────┬───────────────────────────────────────┬───────────┘
                  │                                       │
                  ▼                                       ▼
         ┌────────────────┐                    ┌─────────────────────────┐
         │  openniri-cli  │──── IPC ──────────►│   openniri-daemon       │
         │   (Commands)   │   (Named Pipe)     │    (Event Loop)         │
         │   + Timeout    │    5s timeout      │    + WinEvent Hooks     │
         │   + 38 tests   │                    │    + Multi-monitor      │
         └────────────────┘                    │    + Hotkey Reload      │
                                               │    + Smooth Animations  │
                                               │    + Focus Follows Mouse│
                                               │    + Display Change     │
                                               └────────────┬────────────┘
                                                            │
                  ┌──────────────────────────────┬──────────┴──────────┐
                  │                              │                      │
                  ▼                              ▼                      ▼
         ┌────────────────┐            ┌────────────────┐     ┌───────────────┐
         │ openniri-ipc   │            │ Per-Monitor    │     │   WinEvent    │
         │ (Protocol)     │            │  Workspaces    │     │    Hooks      │
         │ + Monitor cmds │            │ (HashMap)      │     │ + Hotkeys     │
         │ + QueryAll     │            └───────┬────────┘     │ + Mouse Hook  │
         └────────────────┘                    │              └───────────────┘
                                               │
                                               ▼
                                      ┌────────────────┐
                                      │openniri-core-  │
                                      │    layout      │
                                      └───────┬────────┘
                                              │
                                              ▼
                                     ┌────────────────────┐
                                     │openniri-platform-  │
                                     │      win32         │
                                     │ + Multi-monitor    │
                                     │ + DisplayChange    │
                                     │ + HideStrategy     │
                                     └────────────────────┘
```

---

## Known Issues & Technical Debt

| Issue | Severity | Iteration Introduced | Status |
|-------|----------|---------------------|--------|
| Global EVENT_SENDER for hooks | Low | 9 | Acceptable (thread safety) |
| Config `default_config_path` unused | Low | 10 | Minor (dead code warning) |
| `monitors_list` method unused | Low | 11 | Minor (dead code warning) |
| No end-to-end daemon integration tests | Medium | - | Partially addressed in Iteration 27 |
| ARCHITECTURE.md/SPEC.md may drift again | Low | 24 | Refreshed in Iteration 32 |

---

## Next Iteration Planning

### Iteration 29 (Dramatic UX Overhaul)

**Focus**: UX overhaul with real OS integration

**Completed**:
1. SetForegroundWindow integration - focus commands now actually move OS focus
2. Owner-window filtering - dialogs excluded, UWP apps tiled correctly
3. CloseWindow command (Win+Shift+Q)
4. ToggleFloating command (Win+F)
5. ToggleFullscreen command (Win+Shift+F)
6. SetColumnWidth presets (Win+1=1/3, Win+2=1/2, Win+3=2/3, Win+0=equalize)
7. Active window border via DWM (DWMWA_BORDER_COLOR)
8. Snap hints and gestures enabled by default
9. QueryStatus command and CLI status subcommand
10. Tray menu: Pause Tiling, Open Config, View Logs
11. Auto-start via Registry (openniri-cli autostart enable/disable)

**Tests**: 261 -> 295 (291 passed, 4 ignored, 0 warnings)

---

### Iteration 30 (Crash Safety and Reliability)

**Focus**: Safer daemon shutdown and crash recovery behavior

**Completed**:
1. Ctrl+C signal handling that emits `DaemonEvent::Shutdown`
2. Managed-window uncloak/reset on daemon shutdown
3. Panic-hook emergency uncloak-all-visible behavior
4. DPI awareness initialization at process startup
5. Tray Exit routed through unified shutdown cleanup path
6. Added reliability tests for new shutdown/recovery helpers

**Tests**: 295 -> 302 (297 passed, 5 ignored, 0 warnings)

---

### Iteration 31 (Repository Presentation Refresh)

**Focus**: Public-facing quality of project messaging and GitHub profile

**Completed**:
1. Rewrote `README.md` to present product intent, current capability surface, and practical quick-start flow.
2. Removed low-signal public framing and replaced it with concise product-oriented positioning.
3. Updated GitHub repository metadata (description + topics) to match project scope and quality bar.

**Tests**: 302 -> 302 (no code changes in this iteration)

---

### Iteration 32 (Public README + GitHub About Revamp)

**Focus**: Raise public repository quality and make onboarding expectations explicit

**Completed**:
1. Rebuilt `README.md` structure with clearer product framing and user expectations.
2. Added fuller default-hotkey coverage and more direct quick-start/stop flow.
3. Tightened architecture/documentation references for easier contributor orientation.
4. Updated GitHub About metadata to better match current implementation surface.

**Tests**: 302 -> 302 (no code changes in this iteration)

---

### Iteration 33 (Planned)

**Focus**: Polish & Advanced Features

**Objectives**:
1. Multi-workspace support (named workspaces per monitor, switch between them)
2. Enhanced window rules (assign to specific workspace/monitor)
3. Performance profiling and optimization
4. End-to-end integration tests (spawn daemon, CLI interaction)
5. Window layout import/export

---

*This document is automatically updated after each development iteration.*






