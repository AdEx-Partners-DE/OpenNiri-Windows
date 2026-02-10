# Compatibility Matrix

This document lists tested configurations and known behaviors across Windows versions, application frameworks, and monitor setups.

## Windows Versions

| OS Version | Status | Notes |
|-----------|--------|-------|
| Windows 10 21H2+ | Supported | Primary development target |
| Windows 10 1903-21H1 | Expected to work | DWM cloaking available |
| Windows 11 | Supported | Tested regularly |
| Windows Server | Not tested | May work for desktop sessions |

## Application Frameworks

| Framework | Tiling | Floating | Known Issues |
|-----------|--------|----------|-------------|
| Win32 (native) | Yes | Yes | None |
| WPF | Yes | Yes | None |
| WinForms | Yes | Yes | None |
| Electron | Yes | Yes | Some apps use non-standard close behavior |
| UWP/Store | Partial | Partial | Some use non-standard window classes; may need window rules to ignore |
| WinUI 3 | Expected | Expected | Limited testing |
| Qt | Yes | Yes | None |
| GTK (MSYS2) | Yes | Yes | None |

## Elevated (Admin) Windows

Windows running as administrator **cannot be managed** by a non-elevated OpenNiri daemon. This is a Windows security restriction.

- The daemon logs a warning when it encounters an elevated window
- Elevated windows are simply ignored (not tiled)
- To manage elevated windows, run the daemon itself as administrator (not recommended for daily use)

## Shell & System Windows

The following window classes are excluded from management by default:

- `Shell_TrayWnd` — Windows taskbar
- `Progman` — Desktop window
- `WorkerW` — Desktop wallpaper layer
- `Shell_SecondaryTrayWnd` — Secondary taskbar
- `Windows.UI.Core.CoreWindow` — Start menu, Action Center

These are filtered during `enumerate_windows()` in the platform layer.

## Monitor Topologies

| Setup | Status | Notes |
|-------|--------|-------|
| Single monitor | Fully supported | Primary development configuration |
| Dual side-by-side | Supported | `FocusMonitorLeft/Right` and `MoveToMonitor*` work |
| Dual stacked (vertical) | Supported | Uses `monitor_to_left/right` based on X-coordinate proximity |
| Triple+ monitors | Supported | Left/right navigation follows physical X-order |
| Ultrawide (3440x1440) | Supported | Consider wider `default_column_width` in config |
| Mixed DPI | Supported | Per-monitor DPI awareness V2 enabled |
| DisplayLink adapters | Expected to work | May have slight delay in monitor detection |
| Virtual monitors | Not tested | Behavior depends on driver implementation |

### Hot-Plug Behavior

When monitors are connected or disconnected while the daemon is running:

1. The platform layer detects `WM_DISPLAYCHANGE`
2. `reconcile_monitors()` adds/removes workspaces
3. Windows from removed monitors migrate to the primary monitor
4. Layout is re-applied automatically

## Known Edge Cases

### Exclusive-Fullscreen Games

Games running in exclusive fullscreen mode may conflict with the window manager. Use window rules to ignore game windows:

```toml
[[window_rules]]
match_executable = "game.exe"
action = "ignore"
```

### Virtual Desktops

Windows virtual desktops (`Win+Ctrl+D`) are **not integrated**. The daemon manages windows on the current virtual desktop. Switching desktops may cause unexpected behavior with window visibility.

### Screen Readers & Accessibility Tools

OpenNiri repositions windows using `SetWindowPos`. Screen readers that depend on window position may need adjustment. DWM cloaking preserves windows in the accessibility tree.

### High-Refresh-Rate Displays

Scroll animations use a 16ms tick (~60 FPS). On 120Hz+ displays, animations may appear slightly less smooth. The layout application itself is not frame-rate dependent.
