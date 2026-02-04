# OpenNiri-Windows Development Iteration Log

> **Purpose**: This document tracks all development iterations, providing evidence and links for meaningful review and verification.
> **Maintainer**: Claude (Anthropic AI Assistant)
> **Last Updated**: 2026-02-04 (Iteration 16)

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

---

## Detailed Iteration Logs

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

---

## Architecture Evolution

### Current State (Post-Iteration 16)

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
         └────────────────┘                    │    + Multi-monitor      │
                                               └────────────┬────────────┘
                                                            │
                  ┌──────────────────────────────┬──────────┴──────────┐
                  │                              │                      │
                  ▼                              ▼                      ▼
         ┌────────────────┐            ┌────────────────┐     ┌───────────────┐
         │ openniri-ipc   │            │ Per-Monitor    │     │   WinEvent    │
         │ (Protocol)     │            │  Workspaces    │     │    Hooks      │
         │ + Monitor cmds │            │ (HashMap)      │     │ + Hotkeys     │
         └────────────────┘            └───────┬────────┘     └───────────────┘
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
                                     │   helpers          │
                                     └────────────────────┘
```

---

## Known Issues & Technical Debt

| Issue | Severity | Iteration Introduced | Status |
|-------|----------|---------------------|--------|
| Global EVENT_SENDER for hooks | Low | 9 | Acceptable (thread safety) |
| No touchpad gestures | Low | - | Planned |
| Config `default_config_path` unused | Low | 10 | Minor (dead code warning) |
| `monitors_list` method unused | Low | 11 | Minor (dead code warning) |

---

## Next Iteration Planning

### Iteration 17 (Planned)

**Focus**: Window management polish & UX

**Objectives**:
2. Touchpad gesture support (if feasible)
3. Window snapping/docking hints
4. System tray icon & status

---

*This document is automatically updated after each development iteration.*






