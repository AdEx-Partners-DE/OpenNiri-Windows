# Configuration Reference

OpenNiri reads its configuration from a TOML file. The daemon searches the following
locations in order, using the first file it finds:

1. `%APPDATA%\openniri\config\config.toml` (Windows standard, via `directories::ProjectDirs`)
2. `~\.config\openniri\config.toml` (Unix-style, for WSL compatibility)
3. `.\config.toml` (current working directory, for development)

If no configuration file is found, all settings use their defaults. Every field is
optional -- you only need to specify what you want to change.

To reload configuration without restarting the daemon, use the CLI:

```
openniri-cli reload
```

---

## Table of Contents

- [layout](#layout)
- [appearance](#appearance)
- [behavior](#behavior)
- [hotkeys](#hotkeys)
- [gestures](#gestures)
- [snap_hints](#snap_hints)
- [window_rules](#window_rules)
- [Available Commands](#available-commands)
- [Example Configurations](#example-configurations)
- [Validation Rules](#validation-rules)

---

## [layout]

Controls column sizing, gaps, and viewport scrolling behavior.

| Field | Type | Default | Valid Range | Description |
|-------|------|---------|-------------|-------------|
| `gap` | integer | `10` | >= 0 | Gap in pixels between adjacent columns. Negative values are clamped to 0. |
| `outer_gap` | integer | `10` | >= 0 | Gap in pixels between the outermost columns and the edge of the monitor. Negative values are clamped to 0. |
| `default_column_width` | integer | `800` | Must be within [`min_column_width`, `max_column_width`] | The width in pixels assigned to newly created columns. If outside the min/max range, it is clamped. |
| `min_column_width` | integer | `400` | Must be <= `max_column_width` | Minimum allowed column width in pixels. Columns cannot be resized below this. If set larger than `max_column_width`, the two values are swapped. |
| `max_column_width` | integer | `1600` | Must be >= `min_column_width` | Maximum allowed column width in pixels. Columns cannot be resized above this. If set smaller than `min_column_width`, the two values are swapped. |
| `centering_mode` | string | `"center"` | `"center"`, `"just_in_view"` | Controls how the viewport scrolls when focus changes. `"center"` keeps the focused column centered in the viewport. `"just_in_view"` only scrolls if the focused column would otherwise be off-screen. |

```toml
[layout]
gap = 10
outer_gap = 10
default_column_width = 800
min_column_width = 400
max_column_width = 1600
centering_mode = "center"
```

---

## [appearance]

Controls visual features like window cloaking, batched positioning, and active border highlighting.

| Field | Type | Default | Valid Values | Description |
|-------|------|---------|--------------|-------------|
| `use_cloaking` | boolean | `true` | `true`, `false` | When enabled, windows scrolled off-screen are hidden using the DWM cloaking API instead of being moved to extreme coordinates. This prevents taskbar thumbnails from showing off-screen content. |
| `use_deferred_positioning` | boolean | `true` | `true`, `false` | When enabled, multiple window move/resize operations are batched into a single `DeferWindowPos` call. This reduces flicker during layout updates. |
| `active_border` | boolean | `true` | `true`, `false` | When enabled, the focused window's border color is set via the DWM border color API (Windows 11 and later). |
| `active_border_color` | string | `"4285F4"` | 6-character hex RGB string | The color applied to the focused window's border when `active_border` is enabled. Format is hex RGB without a leading `#` (e.g., `"4285F4"` for Google Blue, `"FF0000"` for red). |

```toml
[appearance]
use_cloaking = true
use_deferred_positioning = true
active_border = true
active_border_color = "4285F4"
```

---

## [behavior]

Controls runtime behavior such as focus tracking, logging, and focus-follows-mouse.

| Field | Type | Default | Valid Range | Description |
|-------|------|---------|-------------|-------------|
| `focus_new_windows` | boolean | `true` | `true`, `false` | When enabled, newly opened windows are automatically focused and their column is scrolled into view. When disabled, new windows are added to the layout but focus stays on the current window. |
| `track_focus_changes` | boolean | `true` | `true`, `false` | When enabled, the daemon monitors Windows focus events (via `EVENT_SYSTEM_FOREGROUND`) and updates its internal focus state to match. When disabled, focus is only changed by explicit hotkey/IPC commands. |
| `log_level` | string | `"info"` | `"trace"`, `"debug"`, `"info"`, `"warn"`, `"error"` | Controls the daemon's logging verbosity. Uses standard Rust/tracing log levels. |
| `focus_follows_mouse` | boolean | `false` | `true`, `false` | When enabled, moving the mouse cursor into a window automatically focuses it. |
| `focus_follows_mouse_delay_ms` | integer (u32) | `100` | >= 50 (when `focus_follows_mouse` is true) | Delay in milliseconds before a focus change is triggered when the cursor enters a new window. Prevents rapid focus switching when moving the mouse across several windows. Values below 50 are clamped to 50 when `focus_follows_mouse` is enabled. |

```toml
[behavior]
focus_new_windows = true
track_focus_changes = true
log_level = "info"
focus_follows_mouse = false
focus_follows_mouse_delay_ms = 100
```

---

## [hotkeys]

Maps keyboard shortcuts to commands. Each entry is a key-value pair where the key is
a hotkey string and the value is a command name.

Hotkey strings are formatted as modifier keys joined with `+`, followed by the key name.
Supported modifiers: `Win`, `Ctrl`, `Alt`, `Shift`. These can be combined in any order.

### Default Hotkeys

| Hotkey | Command | Description |
|--------|---------|-------------|
| `Win+H` | `focus_left` | Focus the column to the left |
| `Win+L` | `focus_right` | Focus the column to the right |
| `Win+J` | `focus_down` | Focus the window below (in stacked columns) |
| `Win+K` | `focus_up` | Focus the window above (in stacked columns) |
| `Win+Shift+H` | `move_column_left` | Move the focused column one position left |
| `Win+Shift+L` | `move_column_right` | Move the focused column one position right |
| `Win+Ctrl+H` | `resize_shrink` | Shrink the focused column by 50px |
| `Win+Ctrl+L` | `resize_grow` | Grow the focused column by 50px |
| `Win+Ctrl+Escape` | `panic_revert` | Emergency visibility restore and daemon shutdown |
| `Win+Alt+H` | `focus_monitor_left` | Focus the monitor to the left |
| `Win+Alt+L` | `focus_monitor_right` | Focus the monitor to the right |
| `Win+Alt+Shift+H` | `move_to_monitor_left` | Move the focused window to the left monitor |
| `Win+Alt+Shift+L` | `move_to_monitor_right` | Move the focused window to the right monitor |
| `Win+R` | `refresh` | Re-enumerate windows and add new ones |
| `Win+Shift+Q` | `close_window` | Close the focused window |
| `Win+F` | `toggle_floating` | Toggle the focused window between tiled and floating |
| `Win+Shift+F` | `toggle_fullscreen` | Toggle fullscreen for the focused window |
| `Win+1` | `width_third` | Set focused column width to 1/3 of viewport |
| `Win+2` | `width_half` | Set focused column width to 1/2 of viewport |
| `Win+3` | `width_two_thirds` | Set focused column width to 2/3 of viewport |
| `Win+0` | `equalize_widths` | Make all columns equal width |

When you define a `[hotkeys]` section, it completely replaces the defaults. You must
include every binding you want to keep.

```toml
[hotkeys]
"Win+H" = "focus_left"
"Win+L" = "focus_right"
"Win+J" = "focus_down"
"Win+K" = "focus_up"
"Win+Shift+H" = "move_column_left"
"Win+Shift+L" = "move_column_right"
"Win+Ctrl+H" = "resize_shrink"
"Win+Ctrl+L" = "resize_grow"
"Win+Ctrl+Escape" = "panic_revert"
"Win+Alt+H" = "focus_monitor_left"
"Win+Alt+L" = "focus_monitor_right"
"Win+Alt+Shift+H" = "move_to_monitor_left"
"Win+Alt+Shift+L" = "move_to_monitor_right"
"Win+R" = "refresh"
"Win+Shift+Q" = "close_window"
"Win+F" = "toggle_floating"
"Win+Shift+F" = "toggle_fullscreen"
"Win+1" = "width_third"
"Win+2" = "width_half"
"Win+3" = "width_two_thirds"
"Win+0" = "equalize_widths"
```

---

## [gestures]

Maps three-finger touchpad swipe gestures to commands.

| Field | Type | Default | Valid Values | Description |
|-------|------|---------|--------------|-------------|
| `enabled` | boolean | `true` | `true`, `false` | Master toggle for gesture support. When disabled, all touchpad gestures are ignored. |
| `swipe_left` | string | `"focus_left"` | Any valid command name | Command executed on three-finger swipe left. |
| `swipe_right` | string | `"focus_right"` | Any valid command name | Command executed on three-finger swipe right. |
| `swipe_up` | string | `"focus_up"` | Any valid command name | Command executed on three-finger swipe up. |
| `swipe_down` | string | `"focus_down"` | Any valid command name | Command executed on three-finger swipe down. |

```toml
[gestures]
enabled = true
swipe_left = "focus_left"
swipe_right = "focus_right"
swipe_up = "focus_up"
swipe_down = "focus_down"
```

---

## [snap_hints]

Controls the visual snap-hint overlays that appear during resize operations, showing
column boundaries and snap targets.

| Field | Type | Default | Valid Range | Description |
|-------|------|---------|-------------|-------------|
| `enabled` | boolean | `true` | `true`, `false` | Whether snap hints are shown during resize operations. |
| `duration_ms` | integer (u32) | `200` | >= 50 (when enabled) | How long the snap hint overlay is displayed, in milliseconds. Values below 50 are clamped to 50 when snap hints are enabled. |
| `opacity` | integer (u8) | `128` | 0 -- 255 | Opacity of the snap hint overlay. 0 is fully transparent, 255 is fully opaque. |

```toml
[snap_hints]
enabled = true
duration_ms = 200
opacity = 128
```

---

## [[window_rules]]

Window rules let you control how specific windows are handled. Each rule is defined as
a `[[window_rules]]` entry (TOML array-of-tables syntax). Rules are evaluated in order;
the first matching rule wins.

### Match Fields

All match fields are optional. If multiple match fields are specified on the same rule,
ALL of them must match for the rule to apply (AND logic). A rule with no match fields
matches nothing.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `match_class` | string (regex) | none | Regular expression matched against the window's class name. Uses Rust regex syntax. Case-sensitive by default; use `(?i)` prefix for case-insensitive matching. |
| `match_title` | string (regex) | none | Regular expression matched against the window's title text. Same regex rules as `match_class`. |
| `match_executable` | string | none | Executable filename to match (e.g., `"spotify.exe"`). This is a plain string comparison, **case-insensitive**. |

### Action Fields

| Field | Type | Default | Valid Values | Description |
|-------|------|---------|--------------|-------------|
| `action` | string | `"tile"` | `"tile"`, `"float"`, `"ignore"` | What to do with the matched window. `"tile"` adds it to the tiling layout (default behavior). `"float"` leaves the window at its original position and size, outside the tiling layout. `"ignore"` causes the daemon to skip the window entirely (it will not be managed at all). |
| `width` | integer | none | Any positive integer | Fixed width in pixels for floating windows. Only meaningful when `action = "float"`. Optional. |
| `height` | integer | none | Any positive integer | Fixed height in pixels for floating windows. Only meaningful when `action = "float"`. Optional. |

```toml
# Float DevTools popups with a fixed size
[[window_rules]]
match_class = "Chrome_WidgetWin_1"
match_title = ".*DevTools.*"
action = "float"
width = 800
height = 600

# Float Spotify
[[window_rules]]
match_executable = "spotify.exe"
action = "float"

# Ignore Windows common dialogs
[[window_rules]]
match_class = "#32770"
action = "ignore"
```

---

## Available Commands

These command names can be used in `[hotkeys]` bindings and `[gestures]` mappings.

| Command | Description |
|---------|-------------|
| `focus_left` | Focus the column to the left of the current one. |
| `focus_right` | Focus the column to the right of the current one. |
| `focus_up` | Focus the window above in a stacked column. |
| `focus_down` | Focus the window below in a stacked column. |
| `move_column_left` | Swap the focused column with the one to its left. |
| `move_column_right` | Swap the focused column with the one to its right. |
| `focus_monitor_left` | Move focus to the monitor to the left. No-op with a single monitor. |
| `focus_monitor_right` | Move focus to the monitor to the right. No-op with a single monitor. |
| `move_to_monitor_left` | Move the focused window to the monitor on the left. No-op with a single monitor. |
| `move_to_monitor_right` | Move the focused window to the monitor on the right. No-op with a single monitor. |
| `resize_grow` | Increase the focused column's width by 50 pixels. |
| `resize_shrink` | Decrease the focused column's width by 50 pixels. |
| `scroll_left` | Scroll the viewport left by 100 pixels. |
| `scroll_right` | Scroll the viewport right by 100 pixels. |
| `refresh` | Re-enumerate all windows and add any new ones to the layout. |
| `reload` | Reload the configuration file from disk without restarting the daemon. |
| `panic_revert` | Emergency recovery: restore visibility state and exit daemon via panic-revert cleanup path. |
| `toggle_pause` | Toggle tiling paused/resumed without stopping the daemon. |
| `close_window` | Close the focused window (sends WM_CLOSE). |
| `toggle_floating` | Toggle the focused window between tiled and floating. If the OS-foreground window is floating, it is returned to tiling; otherwise the tiled focused window is moved to floating. |
| `toggle_fullscreen` | Toggle fullscreen for the focused window. |
| `width_third` | Set the focused column's width to 1/3 of the viewport. |
| `width_half` | Set the focused column's width to 1/2 of the viewport. |
| `width_two_thirds` | Set the focused column's width to 2/3 of the viewport. |
| `equalize_widths` | Set all columns to equal width. |

Command names are case-insensitive (e.g., `"Focus_Left"` and `"focus_left"` are equivalent).

---

## Example Configurations

### 1. Basic setup -- just gaps and column width

A minimal config that adjusts spacing and default column size:

```toml
[layout]
gap = 16
outer_gap = 8
default_column_width = 900
```

Everything else uses defaults: vim-style hotkeys, cloaking enabled, center scroll mode.

### 2. Custom hotkeys

Replace the default vim-style bindings with arrow-key navigation:

```toml
[hotkeys]
"Win+Left" = "focus_left"
"Win+Right" = "focus_right"
"Win+Up" = "focus_up"
"Win+Down" = "focus_down"
"Win+Shift+Left" = "move_column_left"
"Win+Shift+Right" = "move_column_right"
"Win+Ctrl+Left" = "resize_shrink"
"Win+Ctrl+Right" = "resize_grow"
"Win+Ctrl+Escape" = "panic_revert"
# Optional: temporary kill switch without daemon exit
#"Win+Ctrl+P" = "toggle_pause"
"Win+Alt+Left" = "focus_monitor_left"
"Win+Alt+Right" = "focus_monitor_right"
"Win+Shift+Q" = "close_window"
"Win+F" = "toggle_floating"
"Win+Shift+F" = "toggle_fullscreen"
"Win+R" = "refresh"
"Win+1" = "width_third"
"Win+2" = "width_half"
"Win+3" = "width_two_thirds"
"Win+0" = "equalize_widths"
```

### 3. Window rules for floating specific apps

Float certain applications that do not work well in a tiling layout, and ignore
system dialogs:

```toml
# Spotify has a non-standard window that does not resize well
[[window_rules]]
match_executable = "spotify.exe"
action = "float"

# Float all "Settings" windows
[[window_rules]]
match_title = "(?i)settings"
action = "float"

# Float calculator with a fixed size
[[window_rules]]
match_executable = "CalculatorApp.exe"
action = "float"
width = 400
height = 600

# Ignore standard Windows dialogs (Open, Save, Print, etc.)
[[window_rules]]
match_class = "#32770"
action = "ignore"

# Float DevTools at a fixed size
[[window_rules]]
match_title = ".*DevTools.*"
action = "float"
width = 1200
height = 800
```

### 4. Focus-follows-mouse setup

Enable sloppy focus with a 150ms delay, and use the just-in-view scrolling mode
so the viewport does not jump around as focus changes:

```toml
[behavior]
focus_follows_mouse = true
focus_follows_mouse_delay_ms = 150

[layout]
centering_mode = "just_in_view"
```

### 5. Full annotated config

A complete config file showing every available option with its default value:

```toml
[layout]
gap = 10
outer_gap = 10
default_column_width = 800
min_column_width = 400
max_column_width = 1600
centering_mode = "center"

[appearance]
use_cloaking = true
use_deferred_positioning = true
active_border = true
active_border_color = "4285F4"

[behavior]
focus_new_windows = true
track_focus_changes = true
log_level = "info"
focus_follows_mouse = false
focus_follows_mouse_delay_ms = 100

[hotkeys]
"Win+H" = "focus_left"
"Win+L" = "focus_right"
"Win+J" = "focus_down"
"Win+K" = "focus_up"
"Win+Shift+H" = "move_column_left"
"Win+Shift+L" = "move_column_right"
"Win+Ctrl+H" = "resize_shrink"
"Win+Ctrl+L" = "resize_grow"
"Win+Ctrl+Escape" = "panic_revert"
"Win+Alt+H" = "focus_monitor_left"
"Win+Alt+L" = "focus_monitor_right"
"Win+Alt+Shift+H" = "move_to_monitor_left"
"Win+Alt+Shift+L" = "move_to_monitor_right"
"Win+R" = "refresh"
"Win+Shift+Q" = "close_window"
"Win+F" = "toggle_floating"
"Win+Shift+F" = "toggle_fullscreen"
"Win+1" = "width_third"
"Win+2" = "width_half"
"Win+3" = "width_two_thirds"
"Win+0" = "equalize_widths"

[gestures]
enabled = true
swipe_left = "focus_left"
swipe_right = "focus_right"
swipe_up = "focus_up"
swipe_down = "focus_down"

[snap_hints]
enabled = true
duration_ms = 200
opacity = 128

# Example window rules (uncomment to use):
# [[window_rules]]
# match_executable = "spotify.exe"
# action = "float"
#
# [[window_rules]]
# match_class = "#32770"
# action = "ignore"
```

---

## Validation Rules

The daemon validates the configuration after loading and automatically corrects
invalid values. When a value is corrected, a warning is logged.

| Condition | Correction |
|-----------|------------|
| `gap` < 0 | Clamped to 0 |
| `outer_gap` < 0 | Clamped to 0 |
| `min_column_width` > `max_column_width` | The two values are swapped |
| `default_column_width` outside [`min_column_width`, `max_column_width`] | Clamped to the valid range |
| `focus_follows_mouse_delay_ms` < 50 (when `focus_follows_mouse` is true) | Clamped to 50 |
| `snap_hints.duration_ms` < 50 (when `snap_hints.enabled` is true) | Clamped to 50 |
| Invalid regex in a window rule's `match_class` or `match_title` | The entire rule is skipped and a warning is logged |
