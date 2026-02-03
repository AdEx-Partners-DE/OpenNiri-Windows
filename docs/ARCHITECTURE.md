# OpenNiri-Windows Architecture

## Overview

OpenNiri-Windows is structured as a Rust workspace with four crates, each with distinct responsibilities.

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
- `insert_window()`: Add a window to the workspace
- `remove_window()`: Remove a window
- `focus_left/right/up/down()`: Navigation
- `compute_placements()`: Calculate window positions given a viewport
- `ensure_focused_visible()`: Adjust scroll offset for focus

**Dependencies**: None (pure Rust + serde)

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

**Dependencies**: `tokio`, `openniri-core-layout`, `openniri-platform-win32`

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

**Dependencies**: `clap`, minimal

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
    columns: Vec<Column>,        // Ordered left-to-right
    focused_column: usize,       // Index of focused column
    focused_window_in_column: usize,
    scroll_offset: f64,          // Viewport position
    gap: i32,
    outer_gap: i32,
    centering_mode: CenteringMode,
}
```

### Daemon State

```rust
AppState {
    workspace: Workspace,
    platform_config: PlatformConfig,
    viewport: Rect,              // Monitor dimensions
    // Future: multi-monitor support
    // monitors: HashMap<MonitorId, Workspace>,
}
```

## Threading Model

- **Main Thread**: Event loop, IPC server
- **WinEvent Callback**: Runs on Windows thread pool, posts to main thread
- **Animation Loop**: Timer-based, runs on main thread

## Error Handling

- Platform errors (Win32) are logged and may be recoverable
- Layout errors should not occur with valid state
- IPC errors result in disconnection, client can retry
