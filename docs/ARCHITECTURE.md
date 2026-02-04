# OpenNiri-Windows Architecture

## Overview

OpenNiri-Windows is structured as a Rust workspace with five crates, each with distinct responsibilities.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                            User / System                                 │
└─────────────────┬───────────────────────────────────────┬───────────────┘
                  │                                       │
                  ▼                                       ▼
         ┌────────────────┐                    ┌─────────────────┐
         │  openniri-cli  │──── IPC ──────────►│ openniri-daemon │
         │   (Commands)   │    (Named Pipe)    │  (Event Loop)   │
         └────────────────┘                    └────────┬────────┘
                                                        │
                                                        ▼
                                               ┌────────────────────┐
                                               │ openniri-core-layout│
                                               │  (Layout Engine)    │
                                               └────────────────────┘
                                                        │
                                                        ▼
                                               ┌────────────────────┐
                                               │openniri-platform-  │
                                               │      win32         │
                                               │ (Win32 APIs)       │
                                               └────────┬───────────┘
                                                        │
                                                        ▼
                                               ┌────────────────────┐
                                               │   Windows DWM      │
                                               │   (Compositor)     │
                                               └────────────────────┘
```

## Crate Responsibilities

### openniri-core-layout

**Purpose**: Platform-agnostic scrollable tiling layout engine.

**Key Types**:
- `Workspace`: The infinite horizontal strip
- `Column`: A vertical container for windows
- `WindowPlacement`: Computed position and visibility for a window
- `Rect`: Screen coordinates rectangle

**Key Functions**:
- `insert_window() -> Result<(), LayoutError>`: Add a window to the workspace (rejects duplicates)
- `insert_window_in_column() -> Result<(), LayoutError>`: Stack a window in an existing column (rejects duplicates)
- `remove_window() -> Result<(), LayoutError>`: Remove a window (with focus policy)
- `focus_left/right/up/down()`: Navigation
- `focus_window(id) -> Result`: Focus a specific window by ID
- `set_focus(col, win) -> Result`: Set focus with validation
- `focused_column_index()`, `focused_window_index_in_column()`: Focus getters
- `columns()`, `column(idx)`, `scroll_offset()`: State getters
- `find_window_location(id) -> Option<(col, win)>`: Locate a window
- `window_count() -> usize`: Total windows across all columns
- `contains_window(id) -> bool`: Check if window exists
- `gap()`, `set_gap()`, `outer_gap()`, `set_outer_gap()`: Gap configuration
- `default_column_width()`, `set_default_column_width()`: Column width config
- `centering_mode()`, `set_centering_mode()`: Centering mode config
- `compute_placements()`: Calculate window positions given a viewport
- `ensure_focused_visible()`: Adjust scroll offset for focus

**Error Variants**:
- `LayoutError::DuplicateWindow`: Window ID already exists
- `LayoutError::WindowNotFound`: Window ID not in workspace
- `LayoutError::ColumnOutOfBounds`: Invalid column index
- `LayoutError::WindowIndexOutOfBounds`: Invalid window index in column

**Invariants (Current Implementation)**:
- No duplicate `WindowId` values (insertions return `LayoutError::DuplicateWindow`)
- Column widths are clamped to a minimum width (100px)
- Gap fields are private; setters clamp to >= 0 (defensive clamping also in calculations)
- Scroll offset is clamped when using `scroll_by()` and `ensure_focused_visible()`
- Focus remains valid after window removal (policy: next window, or previous if at end)
- All internal arithmetic uses saturating operations to prevent overflow
- Debug assertions validate invariants after mutations (debug builds only)

**Note**: All state fields are private; use accessor methods and setters. Scrolling after focus changes is caller responsibility.

**Dependencies**: None (pure Rust + serde + thiserror)

**Testing**: Fully unit-testable without Win32 dependencies.

### openniri-platform-win32

**Purpose**: Windows platform integration layer.

**Key Functions**:
- `enumerate_windows()`: Get list of manageable windows
- `apply_placements()`: Move and resize windows
- `cloak_window()` / `uncloak_window()`: DWM cloaking for off-screen windows
- `install_event_hooks()`: WinEvent hooks for window lifecycle

**Win32 APIs Used**:
- `EnumWindows`: Window enumeration
- `SetWindowPos`: Window positioning
- `BeginDeferWindowPos` / `EndDeferWindowPos`: Batched positioning
- `DwmSetWindowAttribute`: Cloaking, rounded corners
- `SetWinEventHook`: Event hooks

**Dependencies**: `windows-rs`, `openniri-core-layout`

### openniri-ipc

**Purpose**: Shared IPC protocol types for daemon-CLI communication.

**Key Types**:
- `IpcCommand`: Commands sent from CLI to daemon (FocusLeft/Right/Up/Down, MoveColumnLeft/Right, Resize, Scroll, QueryWorkspace, QueryFocused, Refresh, Apply, Stop)
- `IpcResponse`: Responses from daemon (Ok, Error, WorkspaceState, FocusedWindow)
- `PIPE_NAME`: Named pipe path (`\\.\pipe\openniri`)

**Dependencies**: `serde`, `serde_json`, `thiserror`

### openniri-daemon

**Purpose**: Main process that orchestrates everything.

**Responsibilities**:
1. Initialize workspace state
2. Enumerate existing windows on startup
3. Install WinEvent hooks
4. Run IPC server for CLI commands
5. Process events and commands
6. Trigger layout recalculation
7. Apply placements via platform layer

**Event Loop**:
```
┌─────────────────────────────────────────────────────────────────┐
│                         Event Loop                               │
│                                                                  │
│   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│   │ Window Events│    │ IPC Commands │    │ Timer Events │     │
│   │ (WinEvent)   │    │ (Named Pipe) │    │ (Animations) │     │
│   └──────┬───────┘    └──────┬───────┘    └──────┬───────┘     │
│          │                   │                   │              │
│          └───────────────────┼───────────────────┘              │
│                              ▼                                   │
│                    ┌─────────────────┐                          │
│                    │  Update State   │                          │
│                    │  (Workspace)    │                          │
│                    └────────┬────────┘                          │
│                             ▼                                    │
│                    ┌─────────────────┐                          │
│                    │ Compute Layout  │                          │
│                    └────────┬────────┘                          │
│                             ▼                                    │
│                    ┌─────────────────┐                          │
│                    │Apply Placements │                          │
│                    └─────────────────┘                          │
└─────────────────────────────────────────────────────────────────┘
```

**Dependencies**: `tokio`, `openniri-core-layout`, `openniri-platform-win32`, `openniri-ipc`

### openniri-cli

**Purpose**: Command-line interface for user interaction.

**Commands**:
- `focus left|right|up|down`: Navigation
- `scroll left|right`: Manual scrolling
- `move left|right`: Move column
- `resize --delta <N>`: Resize column
- `query workspace|focused|placements`: State queries
- `reload`: Reload configuration
- `stop`: Stop daemon

**IPC Protocol**: JSON over named pipe `\\.\pipe\openniri`

**Dependencies**: `clap`, `tokio`, `openniri-ipc`

---

## Current Status (Reality Check)
- `openniri-core-layout` is implemented and unit-tested (79 tests).
- `openniri-platform-win32` has real Win32 implementations (11 tests + 2 hardware-dependent):
  - `enumerate_windows()` - Uses EnumWindows with filtering
  - `enumerate_monitors()` / `get_primary_monitor()` - Uses EnumDisplayMonitors
  - `apply_placements()` - Uses DeferWindowPos for batched moves
  - `cloak_window()` / `uncloak_window()` - Uses DwmSetWindowAttribute
  - `install_event_hooks()` - WinEvent hooks for window lifecycle
  - `register_hotkeys()` - Global hotkey registration with reload support
- `openniri-ipc` provides shared IPC types (10 tests).
- `openniri-daemon` runs an async event loop with named pipe IPC server (11 tests):
  - Configuration loading from TOML files
  - Global hotkey handling with live reload
  - Smooth scroll animations (~60 FPS)
  - Multi-monitor workspace support
- `openniri-cli` sends IPC commands to the daemon and prints responses.

---

## Deviations, Lessons Learned, and Pivots
- **Duplicate window IDs**: Early assumptions allowed duplicates; we now enforce uniqueness and return explicit errors.
- **Insertion API**: `insert_window()` was assumed infallible; it now returns `Result` to surface duplicates and invalid input.
- **Dimension safety**: We clamp column widths and `Rect` sizes to prevent negative or zero dimensions.
- **Gap safety**: Gap fields are now private with clamping setters; defensive clamping also in calculations.
- **Scroll precision**: Rounding is used to avoid sub-pixel jitter in placement computation.
- **Full encapsulation**: All state and configuration fields are now private; use getters/setters to maintain invariants.
- **Saturating arithmetic**: All internal arithmetic uses saturating operations to prevent overflow with extreme values.
- **Debug assertions**: Internal invariants are validated after mutations (compiled out in release builds).
- **Focus policy defined**: Removal in stacked columns follows a clear policy (next window, or previous if at end).
- **API stability**: The library is pre-1.0; breaking changes may occur. `Column::remove_window()` returns `Option<usize>` (the removed index).

---

## Planned vs Implemented (Gap Summary)
- **Implemented**:
  - Core layout engine with 79 unit tests
  - IPC protocol crate with 10 unit tests
  - Win32 enumeration with filtering (visible, non-tool, non-cloaked, non-system windows)
  - Monitor enumeration via EnumDisplayMonitors (dynamic viewport detection)
  - Window positioning via DeferWindowPos batching
  - DWM cloaking for off-screen windows
  - Async daemon with named pipe IPC server
  - CLI sends real IPC commands and receives responses (with timeout)
  - WinEvent hooks for real-time window tracking (create/destroy/focus/minimize/restore)
  - Configuration file support (TOML format) with live reload
  - Multi-monitor workspace support (one workspace per monitor)
  - Global hotkeys with configurable bindings and live reload
  - Smooth scroll animations (~60 FPS) with easing functions
- **Pending**:
  - Touchpad gesture support
  - Per-window floating/rules
- **Next Steps**: Add touchpad gesture support, implement per-window rules.

## Data Flow

### Window Creation Event

```
1. Windows creates new window
      │
2. WinEvent hook fires (EVENT_OBJECT_CREATE)
      │
3. Daemon receives event
      │
4. Platform layer provides WindowInfo
      │
5. Daemon calls workspace.insert_window()
      │
6. Daemon calls workspace.ensure_focused_visible()
      │
7. Daemon calls workspace.compute_placements()
      │
8. Platform layer calls apply_placements()
      │
9. SetWindowPos + Cloak/Uncloak applied
      │
10. DWM renders updated layout
```

### CLI Command

```
1. User runs: openniri-cli focus right
      │
2. CLI connects to named pipe
      │
3. CLI sends JSON: {"command": "focus", "direction": "right"}
      │
4. Daemon receives command
      │
5. Daemon calls workspace.focus_right()
      │
6. Daemon calls workspace.ensure_focused_visible()
      │
7. Daemon calls workspace.compute_placements()
      │
8. Platform layer applies placements
      │
9. CLI receives confirmation
```

## State Management

### Workspace State

```rust
Workspace {
    // All fields are private - use accessor methods and setters

    // Layout state
    columns: Vec<Column>,               // Use columns(), column(idx)
    focused_column: usize,              // Use focused_column_index()
    focused_window_in_column: usize,    // Use focused_window_index_in_column()
    scroll_offset: f64,                 // Use scroll_offset()

    // Configuration (use getters/setters)
    gap: i32,                           // Use gap(), set_gap() - clamped >= 0
    outer_gap: i32,                     // Use outer_gap(), set_outer_gap() - clamped >= 0
    default_column_width: i32,          // Use default_column_width(), set_default_column_width()
    centering_mode: CenteringMode,      // Use centering_mode(), set_centering_mode()
}
```

### Daemon State

```rust
AppState {
    workspaces: HashMap<MonitorId, Workspace>,  // One workspace per monitor
    monitors: HashMap<MonitorId, MonitorInfo>,  // Monitor info by ID
    focused_monitor: MonitorId,                 // Currently focused monitor
    platform_config: PlatformConfig,            // Window positioning config
    config: Config,                             // User configuration
}
```

### Global Hotkeys

Hotkeys are registered via Win32 `RegisterHotKey` API:
- Hotkey presses are received in a dedicated message window thread
- Events are forwarded to the main event loop via channel
- Hotkey bindings are configurable in TOML config
- Live reload: dropping `HotkeyHandle` unregisters all hotkeys, allowing re-registration

### Smooth Scroll Animations

Viewport scrolling uses animated transitions:
- Animation tick at ~60 FPS (16ms intervals)
- Configurable easing functions (linear, ease-in, ease-out, ease-in-out)
- Animation state tracked per-workspace
- Timer spawned on-demand, stopped when animations complete

## Threading Model

- **Main Thread**: Event loop, IPC server
- **WinEvent Callback**: Runs on Windows thread pool, posts to main thread
- **Animation Loop**: Timer-based, runs on main thread

## Error Handling

- Platform errors (Win32) are logged and may be recoverable
- Layout errors should not occur with valid state
- IPC errors result in disconnection, client can retry
