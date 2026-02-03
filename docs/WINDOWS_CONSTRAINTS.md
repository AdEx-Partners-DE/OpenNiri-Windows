# Windows Platform Constraints

This document details the technical constraints and workarounds needed for implementing Niri-style scrolling on Windows.

## The Fundamental Difference

### Niri on Linux (Wayland)

Niri is a **compositor** - it owns the rendering pipeline. It:
- Controls what gets drawn and where
- Can translate window textures freely
- Implements scrolling as texture coordinate changes
- Has perfect frame synchronization

### OpenNiri on Windows

Windows DWM is the **exclusive compositor**. OpenNiri-Windows:
- Cannot control rendering
- Can only manipulate window positions (HWNDs)
- Must use heavyweight Win32 APIs for movement
- Fights DWM for control

## DWM (Desktop Window Manager) Constraints

### DWM Cannot Be Replaced

Since Windows 8, DWM is always on. There's no official API to:
- Disable DWM
- Inject custom shaders
- Control composition order
- Implement custom animations in the render pass

### DWM Occlusion Culling

DWM aggressively optimizes rendering:
- Off-screen windows may stop rendering
- Occluded windows may show stale content
- Priority degradation for background processes

**Workaround**: Use DWM cloaking instead of moving windows far off-screen.

## Win32 API Constraints

### SetWindowPos Overhead

`SetWindowPos` is the primary tool for window management, but:
- It's synchronous and heavyweight
- Each call triggers WM_WINDOWPOSCHANGING messages
- Applications can intercept and delay the operation
- Sequential calls cause visual "cascade" effect

**Workaround**: Use `BeginDeferWindowPos` / `EndDeferWindowPos` to batch moves.

```c
HDWP hdwp = BeginDeferWindowPos(count);
for (each window) {
    hdwp = DeferWindowPos(hdwp, hwnd, ...);
}
EndDeferWindowPos(hdwp);  // Single atomic update
```

### Coordinate System Limits

Win32 uses 32-bit signed integers for coordinates:
- Theoretical range: -2,147,483,648 to +2,147,483,647
- Practical limit: ~±32,767 (GDI legacy)
- Very far off-screen positions may cause rendering bugs

**Workaround**: Keep windows within reasonable coordinate bounds. Consider coordinate renormalization for extreme cases.

### Window Min/Max Constraints

Applications can define minimum and maximum sizes via `WM_GETMINMAXINFO`:
- The WM cannot reliably query these limits beforehand
- Resize attempts that violate constraints are clamped

**Workaround**: Detect clamping and potentially float non-conforming windows.

## Off-Screen Window Handling

### The Problem

Moving a window to `x = 50000` (far off-screen) causes:
- DWM may stop compositing it
- Content becomes stale (black rectangle when scrolled back)
- Process priority may be reduced

### The Cloaking Solution

DWM provides a "cloaking" mechanism via `DwmSetWindowAttribute`:

```c
BOOL cloak = TRUE;
DwmSetWindowAttribute(hwnd, DWMWA_CLOAK, &cloak, sizeof(cloak));
```

Cloaked windows:
- Are invisible but remain "active"
- Stay in Alt-Tab and taskbar
- Maintain their render surfaces
- Can be uncloaked instantly

### Buffer Zone Strategy

To ensure smooth scrolling:

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Cloaked │  Buffer Zone │     Viewport     │  Buffer Zone │   Cloaked   │
│ Windows │  (Uncloaked) │   (On Screen)    │  (Uncloaked) │   Windows   │
│         │  Off-screen  │                  │  Off-screen  │             │
└─────────────────────────────────────────────────────────────────────────┘
     ▲           ▲                                  ▲            ▲
     │           │                                  │            │
   Hidden    Positioned                        Positioned     Hidden
   (cloak)   off-screen                       off-screen     (cloak)
             ~1000px                          ~1000px
```

1. **Viewport**: Windows within monitor bounds - fully managed
2. **Buffer Zone**: Windows within ~1000px of viewport edge - positioned off-screen but uncloaked
3. **Beyond Buffer**: Cloaked to save resources

## Animation Constraints

### No GPU-Accelerated Custom Animations

On Niri, scrolling animations are:
- Computed on GPU
- Perfectly synchronized with refresh rate
- Zero CPU overhead during animation

On Windows, scrolling requires:
- Repeated `SetWindowPos` calls
- CPU-bound animation loop
- Potential visual artifacts at high speeds

### Animation Loop Design

```rust
// Run at monitor refresh rate (e.g., 144Hz)
fn animation_tick() {
    let now = Instant::now();
    let dt = now - last_tick;

    // Interpolate scroll offset
    current_offset = lerp(current_offset, target_offset, ease(dt));

    // Batch all window moves
    let hdwp = BeginDeferWindowPos(count);
    for window in visible_windows {
        let rect = calculate_position(window, current_offset);
        DeferWindowPos(hdwp, window.hwnd, rect);
    }
    EndDeferWindowPos(hdwp);

    last_tick = now;
}
```

### CPU Cost Warning

Komorebi documentation explicitly warns:
- Animation FPS setting increases CPU usage
- Higher durations increase CPU usage
- Artifacts may appear at high settings

## Input Handling

### No Native Gesture Support

Niri uses `libinput` for:
- 1:1 touchpad tracking
- Multi-finger gestures
- Precise scroll deltas

Windows provides:
- Raw Input API
- WM_GESTURE messages (limited)
- No direct touchpad scrolling surface access

**Workaround**: Use external tools (AutoHotKey, AutoHotInterception) to translate gestures to IPC commands.

## Multi-Monitor Challenges

### Monitor Adjacency Problem

On multi-monitor setups, moving a window horizontally can accidentally place it on an adjacent monitor:

```
┌────────────────┐┌────────────────┐
│   Monitor 1    ││   Monitor 2    │
│                ││                │
│    Window ─────┼┼───► Oops!     │
│                ││                │
└────────────────┘└────────────────┘
```

**Workaround**: Cloak windows instead of positioning them where they'd land on another monitor.

### Per-Monitor DPI

Windows supports different DPI scaling per monitor:
- 100% on one monitor, 150% on another
- Window sizes must be scaled appropriately
- Coordinate transforms become complex

## Known Limitations

### Cannot Achieve

1. **True 1:1 Gesture Tracking**: Always some latency due to Win32 overhead
2. **Pixel-Perfect Smooth Scrolling**: DWM composition timing is opaque
3. **Custom Window Decorations**: DWM controls title bars (can hide, but not custom render)
4. **GPU-Accelerated Effects**: No shader injection into DWM composition

### Can Achieve (With Effort)

1. **Functional Infinite Scrolling**: Via cloaking + batched positioning
2. **Decent Animation Smoothness**: ~60fps with optimization
3. **Keyboard Navigation**: Fully controllable
4. **Basic Touchpad Scrolling**: Via external gesture tools
