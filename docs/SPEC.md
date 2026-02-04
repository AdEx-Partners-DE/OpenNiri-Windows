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

## Multi-Monitor Support (Future)

Each monitor will have:
- Its own workspace (infinite strip)
- Its own scroll offset
- Independent focus

Windows can be moved between monitors, effectively moving between workspaces.

## Configuration

### Per-Workspace Settings

All configuration fields are accessed via getters and modified via setters that enforce invariants:

- `gap()` / `set_gap(px)`: Inner gap between columns (clamped to >= 0)
- `outer_gap()` / `set_outer_gap(px)`: Gap at viewport edges (clamped to >= 0)
- `default_column_width()` / `set_default_column_width(px)`: Default width for new columns (clamped to >= 100)
- `centering_mode()` / `set_centering_mode(mode)`: `Center` or `JustInView`

### Per-Window Overrides (Future)

- Floating windows
- Forced column width
- Stack position preference

---

## Implementation Gaps vs Intended Behavior
- **Implemented**: Core layout (52 tests), IPC protocol (10 tests), Win32 enumeration with cloaked window filtering, monitor detection, batched positioning (DeferWindowPos), DWM cloaking, async daemon with IPC server and WinEvent hooks, CLI with IPC client and timeout.
- **Pending**: Config file support.
- Multi-monitor support is still planned and not implemented.
- Touchpad gesture input is planned but not implemented; current flow is keyboard/CLI oriented.
