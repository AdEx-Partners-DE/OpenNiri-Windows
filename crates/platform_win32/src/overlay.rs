//! Overlay window for visual snapping hints.
//!
//! This module provides a transparent overlay window that can display
//! visual hints during resize operations, showing snap targets and
//! column boundaries.
//!
//! # Architecture
//!
//! The overlay window runs on a dedicated message-loop thread (see
//! [`spawn_message_loop_thread`]). This ensures that drawing operations don't
//! block the main daemon event loop, and dropping the overlay waits at most a
//! bounded time for that thread.
//!
//! # Thread Safety
//!
//! The [`OverlayWindow`] struct can be safely shared across threads.
//! State is managed through a global mutex-protected [`OverlayState`]
//! which is accessed both from the main thread (for showing/hiding)
//! and the overlay thread (for painting).

use crate::message_loop::{spawn_message_loop_thread, MessageLoopThread, ThreadWindow};
use crate::Win32Error;
use openniri_core_layout::Rect;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, InvalidateRect, PAINTSTRUCT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, SetLayeredWindowAttributes, SetWindowPos, ShowWindow, HWND_TOPMOST, LWA_ALPHA,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNA, WM_PAINT, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

/// RGBA color for overlay (semi-transparent blue).
const OVERLAY_COLOR: u32 = 0x00FF8040; // RGB: 0x4080FF (reversed for Windows)

/// Global state for the overlay window.
static OVERLAY_STATE: std::sync::Mutex<OverlayState> = std::sync::Mutex::new(OverlayState {
    rect: None,
    color: OVERLAY_COLOR,
});
static OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Current overlay display state.
struct OverlayState {
    /// Rectangle to display (None = hidden).
    rect: Option<Rect>,
    /// Color for the overlay.
    color: u32,
}

/// A transparent overlay window for displaying visual snap hints.
///
/// The overlay window is rendered as a semi-transparent rectangle on top of
/// all other windows. It's used to provide visual feedback during resize
/// operations, showing column boundaries and snap targets.
///
/// # Features
///
/// - **Click-through**: The overlay doesn't capture any mouse events
/// - **Always on top**: Rendered above all other windows
/// - **No taskbar presence**: Hidden from taskbar and Alt-Tab
/// - **Configurable opacity and color**: Customize the visual appearance
///
/// # Example
///
/// ```no_run
/// use openniri_platform_win32::overlay::OverlayWindow;
/// use openniri_core_layout::Rect;
/// let overlay = OverlayWindow::new().unwrap();
/// overlay.show_snap_target(Rect::new(100, 100, 800, 600));
/// // ... later
/// overlay.hide();
/// ```
///
/// # Lifecycle
///
/// The overlay is created hidden and must be explicitly shown using
/// [`show_snap_target`](Self::show_snap_target) or
/// [`show_column_boundary`](Self::show_column_boundary). When the
/// `OverlayWindow` is dropped, the overlay thread is signaled to exit
/// and the window is destroyed.
pub struct OverlayWindow {
    hwnd: HWND,
    thread: MessageLoopThread,
}

impl OverlayWindow {
    /// Create a new overlay window.
    ///
    /// Creates a hidden, transparent overlay window on a background thread.
    /// The window is initialized with:
    /// - 50% opacity (alpha = 128)
    /// - Click-through behavior (WS_EX_TRANSPARENT)
    /// - Always-on-top positioning (WS_EX_TOPMOST)
    /// - No taskbar or Alt-Tab presence (WS_EX_TOOLWINDOW)
    ///
    /// The overlay is initially hidden. Use [`show_snap_target`](Self::show_snap_target)
    /// to display it.
    ///
    /// # Errors
    ///
    /// Returns [`Win32Error::WindowCreationFailed`](crate::Win32Error::WindowCreationFailed)
    /// if an overlay already exists or the window cannot be created, and
    /// [`Win32Error::ThreadFailed`](crate::Win32Error::ThreadFailed) if its
    /// thread cannot be started.
    pub fn new() -> Result<Self, Win32Error> {
        if OVERLAY_ACTIVE.swap(true, Ordering::SeqCst) {
            return Err(Win32Error::WindowCreationFailed(
                "Overlay window already active".to_string(),
            ));
        }

        let started = spawn_message_loop_thread(
            "openniri-overlay",
            || {
                // WS_EX_LAYERED: Allows transparency
                // WS_EX_TRANSPARENT: Click-through
                // WS_EX_TOPMOST: Always on top
                // WS_EX_TOOLWINDOW: Not in taskbar
                // WS_EX_NOACTIVATE: Don't steal focus
                let window = ThreadWindow::create(
                    "OpenNiriOverlayClass",
                    Some(overlay_window_proc),
                    WS_EX_LAYERED
                        | WS_EX_TRANSPARENT
                        | WS_EX_TOPMOST
                        | WS_EX_TOOLWINDOW
                        | WS_EX_NOACTIVATE,
                    WS_POPUP,
                    1,
                    1,
                )?;
                // Set transparency (alpha = 128, about 50% opacity)
                let _ = unsafe {
                    SetLayeredWindowAttributes(window.hwnd(), Default::default(), 128, LWA_ALPHA)
                };
                // HWND is not Send; hand back its raw value.
                let hwnd_raw = window.hwnd().0 as isize;
                Ok((window, hwnd_raw))
            },
            |_| {},
        );
        let (thread, hwnd_raw) = match started {
            Ok(started) => started,
            Err(e) => {
                OVERLAY_ACTIVE.store(false, Ordering::SeqCst);
                return Err(e);
            }
        };

        tracing::debug!("Overlay window created");

        Ok(Self {
            hwnd: HWND(hwnd_raw as *mut c_void),
            thread,
        })
    }

    /// Show snap target highlight at the given rectangle.
    ///
    /// Displays a semi-transparent overlay at the specified screen coordinates.
    /// The overlay is immediately visible and will remain shown until
    /// [`hide`](Self::hide) is called.
    ///
    /// # Parameters
    ///
    /// * `rect` - The screen coordinates and dimensions for the overlay.
    ///   Uses absolute screen coordinates, not client coordinates.
    ///
    /// # Thread Safety
    ///
    /// This method is safe to call from any thread. It updates the global
    /// overlay state and sends a message to the overlay thread to repaint.
    pub fn show_snap_target(&self, rect: Rect) {
        // Update global state
        if let Ok(mut state) = OVERLAY_STATE.lock() {
            state.rect = Some(rect);
        }

        unsafe {
            // Reposition and resize the window
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );

            // Show the window without activating it
            let _ = ShowWindow(self.hwnd, SW_SHOWNA);

            // Trigger a repaint
            let _ = InvalidateRect(Some(self.hwnd), None, true);
        }
    }

    /// Show a column boundary hint (vertical line at x position).
    ///
    /// Displays a vertical line centered at the given x coordinate.
    /// Useful for showing column dividers during resize operations.
    ///
    /// # Parameters
    ///
    /// * `x` - The x coordinate for the center of the line (screen coordinates)
    /// * `y` - The top y coordinate for the line (screen coordinates)
    /// * `height` - The height of the line in pixels
    /// * `width` - The width/thickness of the line in pixels
    pub fn show_column_boundary(&self, x: i32, y: i32, height: i32, width: i32) {
        let rect = Rect::new(x - width / 2, y, width, height);
        self.show_snap_target(rect);
    }

    /// Hide the overlay immediately.
    ///
    /// The overlay becomes invisible and stops capturing any visual space.
    /// The window is not destroyed, so it can be shown again efficiently.
    ///
    /// # Thread Safety
    ///
    /// This method is safe to call from any thread.
    pub fn hide(&self) {
        // Clear global state
        if let Ok(mut state) = OVERLAY_STATE.lock() {
            state.rect = None;
        }

        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// Check if the overlay is currently visible.
    ///
    /// Returns `true` if the overlay is showing a rectangle, `false` if hidden.
    ///
    /// # Thread Safety
    ///
    /// This method is safe to call from any thread. It reads from the
    /// mutex-protected global state.
    pub fn is_visible(&self) -> bool {
        if let Ok(state) = OVERLAY_STATE.lock() {
            state.rect.is_some()
        } else {
            false
        }
    }

    /// Update the overlay color and trigger a repaint.
    ///
    /// Changes the fill color of the overlay rectangle. The color is
    /// specified in Windows BGR format (0x00BBGGRR).
    ///
    /// # Parameters
    ///
    /// * `color` - The new color in BGR format (e.g., 0x00FF8040 for blue-ish)
    ///
    /// # Note
    ///
    /// The color change takes effect immediately if the overlay is visible.
    /// A repaint is triggered to apply the new color.
    pub fn set_color(&self, color: u32) {
        if let Ok(mut state) = OVERLAY_STATE.lock() {
            state.color = color;
        }

        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, true);
        }
    }
}

impl Drop for OverlayWindow {
    fn drop(&mut self) {
        // The window is destroyed on its thread; a stuck thread is detached.
        self.thread.shutdown(std::time::Duration::from_secs(2));
        OVERLAY_ACTIVE.store(false, Ordering::SeqCst);
        tracing::debug!("Overlay window destroyed");
    }
}

/// Window procedure for the overlay window.
///
/// Wrapped with catch_unwind to prevent panics from crashing the application.
unsafe extern "system" fn overlay_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Wrap in catch_unwind to prevent panics from crashing
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        overlay_window_proc_inner(hwnd, msg, wparam, lparam)
    }));

    match result {
        Ok(lresult) => lresult,
        Err(e) => {
            tracing::error!("Panic in overlay_window_proc: {:?}", e);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

/// Inner implementation of overlay window procedure.
fn overlay_window_proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let _ = wparam; // Unused in current implementation
    let _ = lparam; // Unused in current implementation
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };

            // Get current color from state
            let color = if let Ok(state) = OVERLAY_STATE.lock() {
                state.color
            } else {
                OVERLAY_COLOR
            };

            // Fill with the overlay color
            let brush = unsafe { CreateSolidBrush(windows::Win32::Foundation::COLORREF(color)) };
            let _ = unsafe { FillRect(hdc, &ps.rcPaint, brush) };
            let _ = unsafe { DeleteObject(brush.into()) };

            let _ = unsafe { EndPaint(hwnd, &ps) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Snap hint types for different operations.
///
/// Different hint types can be styled differently (colors, opacity)
/// to provide clear visual feedback about the current operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapHintType {
    /// Highlight showing the new size during column resize.
    ///
    /// Displayed when the user is actively resizing a column,
    /// showing the boundaries of the new column width.
    ColumnResize,
    /// Highlight showing where a window will be placed during move.
    ///
    /// Displayed when dragging a window to show the target
    /// position where it will snap to.
    MoveTarget,
    /// Highlight showing which column will receive focus.
    ///
    /// Displayed briefly when focus changes to help the user
    /// track which window/column has keyboard focus.
    FocusTarget,
}

/// Configuration for snap hint visual feedback.
///
/// Controls the appearance and behavior of the overlay hints shown
/// during resize, move, and focus operations.
///
/// # Color Format
///
/// Colors are specified in Windows BGR format: `0x00BBGGRR`.
/// For example:
/// - `0x00FF0000` = Blue
/// - `0x0000FF00` = Green
/// - `0x000000FF` = Red
/// - `0x00FF8040` = Blue-ish (default)
#[derive(Debug, Clone)]
pub struct SnapHintConfig {
    /// Whether visual hints are enabled.
    ///
    /// When `false`, no overlay will be shown regardless of other settings.
    pub enabled: bool,
    /// Color for column resize hints (BGR format, e.g., 0x00FF8040).
    pub resize_color: u32,
    /// Color for move target hints (BGR format).
    pub move_color: u32,
    /// Color for focus target hints (BGR format).
    pub focus_color: u32,
    /// How long hints are displayed in milliseconds.
    ///
    /// After this duration, the hint automatically hides.
    /// Typical values are 150-300ms for subtle feedback.
    pub duration_ms: u32,
}

impl Default for SnapHintConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            resize_color: 0x00FF8040, // Semi-transparent blue
            move_color: 0x0040FF40,   // Semi-transparent green
            focus_color: 0x004080FF,  // Semi-transparent orange
            duration_ms: 200,
        }
    }
}
