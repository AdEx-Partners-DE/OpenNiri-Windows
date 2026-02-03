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

### Window Removal

When a window is closed:
1. Remove the window from its column
2. If the column is now empty, remove the column
3. Adjust focus to the nearest window
4. Scroll viewport if needed

## Focus Navigation

### Horizontal Navigation (Between Columns)

- **Focus Left**: Move focus to the column on the left
- **Focus Right**: Move focus to the column on the right

### Vertical Navigation (Within Stacked Columns)

- **Focus Up**: Move focus to the window above in the stack
- **Focus Down**: Move focus to the window below in the stack

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

- `gap`: Inner gap between columns (pixels)
- `outer_gap`: Gap at viewport edges (pixels)
- `default_column_width`: Default width for new columns
- `centering_mode`: `center` or `just_in_view`

### Per-Window Overrides (Future)

- Floating windows
- Forced column width
- Stack position preference
