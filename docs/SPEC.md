# OpenNiri-Windows Behavioral Specification

This document describes the intended behavior of OpenNiri-Windows, inspired by the Niri Wayland compositor.

## Core Paradigm: Scrollable Tiling

### The Infinite Strip Model

Unlike traditional tiling window managers that divide the screen into fixed regions, OpenNiri-Windows arranges windows on an **infinite horizontal strip**. The physical monitor acts as a **viewport** (camera) that slides over this strip.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          INFINITE STRIP                                  │
│  ┌────────┐    ┌────────┐    ┌────────┐    ┌────────┐    ┌────────┐    │
│  │Window 1│    │Window 2│    │Window 3│    │Window 4│    │Window 5│    │
│  │        │    │        │    │        │    │        │    │        │    │
│  └────────┘    └────────┘    └────────┘    └────────┘    └────────┘    │
└─────────────────────────────────────────────────────────────────────────┘
                     ╔═══════════════════╗
                     ║     VIEWPORT      ║
                     ║   (Your Monitor)  ║
                     ╚═══════════════════╝
```

### Key Principles

1. **Spatial Consistency**: Windows maintain their relative positions. "Browser is always to the right of terminal."

2. **No Compression**: Opening a new window does not resize existing windows. New windows append to the strip.

3. **Focus-Driven Navigation**: Moving focus between windows scrolls the viewport to keep the focused window visible.

4. **Predictable Sizing**: Window widths are set explicitly and don't change based on neighbor count.

---

## Current Implementation Notes (Reality Check)
- **Window uniqueness enforced**: Duplicate `WindowId` insertions return `LayoutError::DuplicateWindow`.
- **Insertion API is fallible**: `insert_window()` and `insert_window_in_column()` return `Result`.
- **Empty workspace allowed**: Removing the last window removes the last column and resets focus indices.
- **Dimension safety**: Column widths are clamped to a minimum (100px); `Rect::new` clamps negative sizes to zero.
- **Gap safety**: Gap fields are private with setters that clamp to >= 0; defensive clamping also in calculations.
- **Scroll precision**: `scroll_offset` is rounded during placement calculations to avoid sub-pixel jitter.
- **Focus policy defined**: When removing windows from stacked columns, focus follows a clear policy (see Window Removal).
- **Full encapsulation**: All state fields are private; use accessor methods and setters.
- **Saturating arithmetic**: All internal arithmetic uses saturating operations to prevent overflow.
- **Debug assertions**: Internal invariants are checked via debug_assert! (compiled out in release builds).
- **API unstable**: The library is pre-1.0; breaking changes may occur between versions.

---

## Deviations, Lessons Learned, and Why We Pivoted
- **Duplicate windows were possible** early on. This breaks state integrity and removal logic, so we now reject duplicates at insertion time.
- **Insertions cannot be assumed infallible** in a real WM. The API now returns `Result` to surface invalid operations.
- **Negative or invalid geometry caused invalid placements**. We now clamp widths/heights to prevent negative sizes.
- **Rounding scroll offsets is necessary** to keep smooth scrolling visually stable on Windows.
- **Focus rules now explicit**: Removal in stacked columns follows a clear policy (next window, or previous if at end).
- **Full encapsulation enforced**: All fields are now private to prevent invariant violations; use getters/setters.
- **Saturating arithmetic adopted**: All internal arithmetic uses saturating ops to prevent overflow with extreme values.
- **Debug assertions added**: Internal invariants are validated after mutations (debug builds only).

## Window Management

### Columns

Windows are organized into **columns** on the strip. Each column can contain:
- A single window (most common)
- Multiple stacked windows (vertical stack within the column)

```
Single Column          Stacked Column
┌──────────────┐       ┌──────────────┐
│              │       │   Window A   │
│   Window 1   │       ├──────────────┤
│              │       │   Window B   │
│              │       ├──────────────┤
└──────────────┘       │   Window C   │
                       └──────────────┘
```

### Window Insertion

When a new window is created:
1. Create a new column to the **right** of the currently focused column
2. Place the new window in this column
3. Focus the new window
4. Scroll viewport to center the new window (depending on centering mode)

**Current behavior**: insertion can fail if the window ID already exists. Callers must handle `Result`.

### Window Removal

When a window is closed:
1. Remove the window from its column
2. If the column is now empty, remove the column
3. Adjust focus according to the focus policy (see below)
4. Scroll viewport if needed (caller responsibility)

**Current behavior**: if the last window is removed, the workspace becomes empty and focus indices reset to zero.

#### Focus Policy on Removal (Stacked Columns)

When removing a window from a stacked column:
- **Removed before focused**: Focus index decrements to stay on the same window
- **Removed is focused**: Focus moves to the next window (the one that slides into its position), or previous if at end
- **Removed after focused**: Focus index stays the same

This ensures the user's focus remains predictable and intuitive.

## Focus Navigation

### Horizontal Navigation (Between Columns)

- **Focus Left**: Move focus to the column on the left
- **Focus Right**: Move focus to the column on the right

### Vertical Navigation (Within Stacked Columns)

- **Focus Up**: Move focus to the window above in the stack
- **Focus Down**: Move focus to the window below in the stack

### Programmatic Focus

- `focus_window(id)`: Focus a specific window by its ID
- `set_focus(col, win)`: Set focus to specific indices (with validation)

**Note**: Focus methods only update indices; scrolling to keep focus visible is the caller's responsibility (call `ensure_focused_visible()` after focus changes).

### Utility Methods

- `find_window_location(id) -> Option<(col_idx, win_idx)>`: Locate a window by its ID
- `window_count() -> usize`: Total windows across all columns
- `column(idx) -> Option<&Column>`: Safe column access by index
- `contains_window(id) -> bool`: Check if a window exists in the workspace

### Centering Modes

**Center Mode** (default):
- When focus changes, scroll viewport to center the focused column

**Just-In-View Mode**:
- Only scroll if the focused column would be outside the viewport
- Minimizes unnecessary scrolling

## Viewport Scrolling

### Automatic Scrolling

The viewport automatically scrolls when:
- Focus changes (based on centering mode)
- A new window is inserted
- A window is removed and focus shifts

### Manual Scrolling

Users can manually scroll:
- **Scroll Left/Right**: Move viewport by pixel amount
- Via touchpad gestures (when implemented)
- Via keyboard shortcuts

### Scroll Constraints

- Scroll offset cannot go below 0 (left edge of strip)
- Scroll offset cannot exceed `total_strip_width - viewport_width` (right edge)

## Gaps and Spacing

### Inner Gaps

Space between columns:
```
┌────────┐  gap  ┌────────┐  gap  ┌────────┐
│        │ ◄───► │        │ ◄───► │        │
└────────┘       └────────┘       └────────┘
```

### Outer Gaps

Space at the edges of the viewport:
```
outer ┌────────┐       ┌────────┐ outer
 gap  │        │       │        │  gap
◄───► │        │       │        │ ◄───►
      └────────┘       └────────┘
```

### Stack Gaps

When windows are stacked in a column, vertical gaps separate them.

## Column Operations

### Resize Column

Change the width of the focused column by a delta amount.

### Move Column

Swap the focused column with its neighbor:
- **Move Left**: Swap with column to the left
- **Move Right**: Swap with column to the right

## Multi-Monitor Support

Each monitor has:
- Its own workspace (infinite strip)
- Its own scroll offset
- Independent focus tracking

**Monitor Navigation**:
- `FocusMonitorLeft/Right`: Move focus to adjacent monitor
- `MoveWindowToMonitorLeft/Right`: Move focused window to adjacent monitor (focus follows)

Windows can be moved between monitors, effectively moving between workspaces. Monitor adjacency is determined by physical position (x-coordinate comparison).

## Configuration

### Per-Workspace Settings

All configuration fields are accessed via getters and modified via setters that enforce invariants:

- `gap()` / `set_gap(px)`: Inner gap between columns (clamped to >= 0)
- `outer_gap()` / `set_outer_gap(px)`: Gap at viewport edges (clamped to >= 0)
- `default_column_width()` / `set_default_column_width(px)`: Default width for new columns (clamped to >= 100)
- `centering_mode()` / `set_centering_mode(mode)`: `Center` or `JustInView`

### Per-Window Rules

Window rules allow per-window behavior overrides based on class name, title, or executable:

**Actions**:
- `float` — Window floats outside the tiling strip (optional width/height)
- `tile` — Default tiling behavior
- `ignore` — Window is not managed

**Matching**:
- `match_class` — Regex match on window class name (case-sensitive)
- `match_title` — Regex match on window title (case-sensitive)
- `match_executable` — Case-insensitive match on executable name

Multiple rules are evaluated in order; first match wins.

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

### Floating Windows

Floating windows are positioned independently of the tiling strip:
- Maintain their own position and size
- Can overlap tiled windows
- Default to centered 800x600 if no dimensions specified in rule

---

## Global Hotkeys

Global hotkeys allow keyboard-driven navigation without focusing the daemon window.

### Hotkey Syntax

Hotkeys are specified as strings: `Modifier+Modifier+Key`

**Supported Modifiers**: `Win`, `Ctrl`, `Alt`, `Shift`

**Example**: `Win+Shift+H` = Windows key + Shift + H

### Default Bindings

| Hotkey | Command |
|--------|---------|
| Win+H/J/K/L | Focus left/down/up/right |
| Win+Shift+H/L | Move column left/right |
| Win+Ctrl+H/L | Resize shrink/grow |
| Win+Alt+H/L | Focus monitor left/right |
| Win+Alt+Shift+H/L | Move window to monitor left/right |
| Win+R | Refresh (re-enumerate windows) |

### Live Reload

Hotkey bindings are reloaded when the `reload` command is issued. The daemon:
1. Unregisters all existing hotkeys (drops handle)
2. Parses new hotkey config
3. Registers new hotkeys
4. Updates the command mapping

---

## Scroll Animations

Viewport scrolling uses smooth animations for better visual feedback.

### Animation Parameters

- **Duration**: Configurable (default: 200ms)
- **Tick Rate**: ~60 FPS (16ms intervals)
- **Easing**: Configurable (default: ease-out)

### Easing Functions

| Mode | Description |
|------|-------------|
| Linear | Constant speed |
| EaseIn | Slow start, fast end |
| EaseOut | Fast start, slow end (default) |
| EaseInOut | Slow start and end, fast middle |

### Animation Lifecycle

1. Focus change triggers `ensure_focused_visible_animated()`
2. Animation state created with target scroll offset
3. Animation timer starts (if not running)
4. Each tick advances `elapsed_ms` and applies layout
5. Timer stops when all animations complete

---

## System Tray

The daemon displays a system tray icon with a context menu:
- **Refresh Windows**: Re-enumerate and re-tile all windows
- **Reload Config**: Reload configuration from disk
- **Exit**: Gracefully shut down the daemon

The tray icon uses a procedurally generated blue/green checkerboard icon representing tiling.

---

## Visual Snap Hints

When resizing columns, an overlay window can display the column boundary:

```toml
[snap_hints]
enabled = true
duration_ms = 200
opacity = 128
```

The overlay is a transparent, click-through layered window (WS_EX_LAYERED | WS_EX_TRANSPARENT) that auto-hides after the configured duration.

---

## Focus Follows Mouse

When enabled, moving the mouse over a managed window automatically focuses it after a configurable delay:

```toml
[behavior]
focus_follows_mouse = true
focus_follows_mouse_delay_ms = 100
```

Uses a low-level mouse hook (WH_MOUSE_LL) to track mouse position. Rapid movements are debounced using the configured delay.

---

## Touchpad Gesture Support

Touchpad gestures provide a natural way to navigate the scrollable strip:

```toml
[gestures]
enabled = true
swipe_left = "focus_left"
swipe_right = "focus_right"
swipe_up = "focus_up"
swipe_down = "focus_down"
```

- Horizontal touchpad scroll maps to workspace strip scrolling
- Discrete swipe gestures (threshold-based) map to configurable commands
- Disabled by default

---

## Display Change Handling

When monitors are connected/disconnected:
1. The daemon receives `WM_DISPLAYCHANGE` events
2. Monitors are re-enumerated
3. `reconcile_monitors()` migrates windows between workspaces as needed
4. New monitors get empty workspaces; orphaned windows move to the primary monitor

---

## Workspace Persistence

Window layout state can be saved and restored across daemon restarts:
- Layout structure (columns, window order, scroll offset) saved to `%APPDATA%/openniri/workspace-state.json`
- On startup, windows are matched to persisted positions by class name and executable
- Auto-saves on daemon shutdown

---

## Implementation Status
- **Implemented (202 tests)**: Core layout engine (87 tests), IPC protocol (13 tests), CLI (28 tests), daemon (44 tests), integration tests (17 tests), platform layer (13 tests). Win32 enumeration with cloaked window filtering, monitor detection, batched positioning (DeferWindowPos), DWM cloaking, async daemon with IPC server and WinEvent hooks, CLI with IPC client and timeout, configuration file support (TOML), multi-monitor workspaces, global hotkeys with live reload, smooth scroll animations, per-window floating/rules, system tray, visual snap hints, focus follows mouse, display change handling, touchpad gesture support, workspace persistence.
- **All major features implemented.** Remaining work is polish, testing, and documentation.
