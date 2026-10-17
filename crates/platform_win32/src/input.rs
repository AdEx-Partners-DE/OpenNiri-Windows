//! Low-level mouse hooks: touchpad gesture detection and focus-follows-mouse.

use crate::error::{recover_poisoned_mutex, Win32Error};
use crate::events::{should_emit_window_event, WindowEvent};
use crate::message_loop::{spawn_message_loop_thread, MessageLoopThread};
use crate::window::normalize_to_root_window;
use openniri_core_layout::WindowId;
use std::sync::mpsc;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, WindowFromPoint, HHOOK, MSLLHOOKSTRUCT,
    WH_MOUSE_LL, WM_MOUSEMOVE,
};

/// Install the focus-follows-mouse hook on a dedicated message-loop thread.
pub fn spawn_mouse_hook_thread(
    sender: mpsc::Sender<WindowEvent>,
) -> Result<MessageLoopThread, Win32Error> {
    spawn_message_loop_thread(
        "openniri-mouse-hook",
        move || install_mouse_hook(sender).map(|handle| (handle, ())),
        |_| {},
    )
    .map(|(thread, ())| thread)
}

/// Install the gesture hook on a dedicated message-loop thread.
pub fn spawn_gesture_thread(
) -> Result<(MessageLoopThread, mpsc::Receiver<GestureEvent>), Win32Error> {
    spawn_message_loop_thread("openniri-gesture-hook", register_gestures, |_| {})
}

// ============================================================================
// Touchpad Gesture Support
// ============================================================================

/// Gesture events detected from touchpad/pointer input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureEvent {
    /// Three-finger swipe left
    SwipeLeft,
    /// Three-finger swipe right
    SwipeRight,
    /// Three-finger swipe up
    SwipeUp,
    /// Three-finger swipe down
    SwipeDown,
}

/// Wheel message constants (not all exposed by windows-rs).
const WM_MOUSEWHEEL: u32 = 0x020A;
const WM_MOUSEHWHEEL: u32 = 0x020E;

/// Threshold for accumulated wheel delta before firing a gesture event.
/// 3 * WHEEL_DELTA (120) = 360.
const GESTURE_SCROLL_THRESHOLD: i32 = 360;

/// Timeout in milliseconds: if no scroll event arrives within this window,
/// accumulators are reset.
const GESTURE_TIMEOUT_MS: u128 = 300;

/// Gesture accumulator state for the low-level mouse hook.
struct GestureAccumState {
    /// Accumulated horizontal wheel delta.
    accum_x: i32,
    /// Accumulated vertical wheel delta.
    accum_y: i32,
    /// Timestamp of the last scroll event.
    last_scroll_time: std::time::Instant,
}

/// Global sender for gesture events.
static GESTURE_SENDER: std::sync::Mutex<Option<mpsc::Sender<GestureEvent>>> =
    std::sync::Mutex::new(None);

/// Global gesture accumulator state.
/// Initialized to `None`; `register_gestures()` sets it to `Some(...)`.
static GESTURE_STATE: std::sync::Mutex<Option<GestureAccumState>> = std::sync::Mutex::new(None);

/// Handle for gesture detection.
///
/// Dropping this handle will unhook the low-level mouse hook and stop
/// gesture detection.
pub struct GestureHandle {
    hook: HHOOK,
}

impl Drop for GestureHandle {
    fn drop(&mut self) {
        unsafe {
            if !self.hook.is_invalid() {
                let _ = UnhookWindowsHookEx(self.hook);
            }
        }

        // Clear the global sender and state (recover from mutex poisoning)
        let mut sender = GESTURE_SENDER.lock().unwrap_or_else(recover_poisoned_mutex);
        *sender = None;
        drop(sender);
        let mut state = GESTURE_STATE.lock().unwrap_or_else(recover_poisoned_mutex);
        *state = None;

        tracing::debug!("Gesture detection stopped");
    }
}

/// Register a low-level mouse hook for gesture detection via wheel events.
///
/// Returns a handle that must be kept alive to receive gesture events,
/// and a channel receiver for gesture events.
///
/// Touchpad scroll gestures are delivered as WM_MOUSEWHEEL (vertical) and
/// WM_MOUSEHWHEEL (horizontal) messages. The hook accumulates wheel deltas
/// and fires swipe events when the threshold is exceeded.
pub fn register_gestures() -> Result<(GestureHandle, mpsc::Receiver<GestureEvent>), Win32Error> {
    // Create channel for events
    let (tx, rx) = mpsc::channel();

    // Store sender globally
    {
        let mut sender = GESTURE_SENDER.lock().map_err(|_| {
            Win32Error::HookInstallFailed("Gesture sender mutex poisoned".to_string())
        })?;
        if sender.is_some() {
            return Err(Win32Error::HookInstallFailed(
                "Gesture sender already initialized - drop existing GestureHandle first"
                    .to_string(),
            ));
        }
        *sender = Some(tx);
    }

    // Initialize accumulator state
    {
        let mut state = GESTURE_STATE.lock().map_err(|_| {
            Win32Error::HookInstallFailed("Gesture state mutex poisoned".to_string())
        })?;
        *state = Some(GestureAccumState {
            accum_x: 0,
            accum_y: 0,
            last_scroll_time: std::time::Instant::now(),
        });
    }

    // Install low-level mouse hook
    let hook = unsafe {
        SetWindowsHookExW(WH_MOUSE_LL, Some(gesture_mouse_hook_proc), None, 0).map_err(|e| {
            Win32Error::HookInstallFailed(format!(
                "SetWindowsHookExW for gesture hook failed: {}",
                e
            ))
        })?
    };

    tracing::info!("Gesture detection registered (low-level mouse hook)");

    Ok((GestureHandle { hook }, rx))
}

/// Low-level mouse hook callback for gesture detection.
///
/// Handles WM_MOUSEWHEEL and WM_MOUSEHWHEEL to accumulate scroll deltas
/// and fire swipe gesture events when the threshold is exceeded.
unsafe extern "system" fn gesture_mouse_hook_proc(
    ncode: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    if ncode >= 0 {
        let msg = wparam.0 as u32;
        if msg == WM_MOUSEHWHEEL || msg == WM_MOUSEWHEEL {
            let mouse_struct = &*(lparam.0 as *const MSLLHOOKSTRUCT);
            // The high word of mouseData contains the wheel delta (signed).
            let delta = (mouse_struct.mouseData >> 16) as i16 as i32;

            // Recover from mutex poisoning for both state and sender
            let mut state_guard = GESTURE_STATE.lock().unwrap_or_else(recover_poisoned_mutex);
            if let Some(state) = state_guard.as_mut() {
                let now = std::time::Instant::now();

                // Reset accumulators if timeout exceeded
                if now.duration_since(state.last_scroll_time).as_millis() > GESTURE_TIMEOUT_MS {
                    state.accum_x = 0;
                    state.accum_y = 0;
                }
                state.last_scroll_time = now;

                if msg == WM_MOUSEHWHEEL {
                    state.accum_x += delta;
                } else {
                    state.accum_y += delta;
                }

                // Check thresholds and determine gesture
                let gesture = if state.accum_x.abs() >= GESTURE_SCROLL_THRESHOLD {
                    let g = if state.accum_x > 0 {
                        GestureEvent::SwipeRight
                    } else {
                        GestureEvent::SwipeLeft
                    };
                    state.accum_x = 0;
                    Some(g)
                } else if state.accum_y.abs() >= GESTURE_SCROLL_THRESHOLD {
                    let g = if state.accum_y > 0 {
                        GestureEvent::SwipeDown
                    } else {
                        GestureEvent::SwipeUp
                    };
                    state.accum_y = 0;
                    Some(g)
                } else {
                    None
                };

                if let Some(event) = gesture {
                    let sender_guard = GESTURE_SENDER.lock().unwrap_or_else(recover_poisoned_mutex);
                    if let Some(sender) = sender_guard.as_ref() {
                        let _ = sender.send(event);
                    }
                }
            }
        }
    }

    CallNextHookEx(None, ncode, wparam, lparam)
}

// ============================================================================
// Focus Follows Mouse (Low-Level Mouse Hook)
// ============================================================================

/// Global sender for mouse enter events.
static MOUSE_EVENT_SENDER: std::sync::Mutex<Option<mpsc::Sender<WindowEvent>>> =
    std::sync::Mutex::new(None);

/// Track the window the mouse is currently over.
static CURRENT_MOUSE_WINDOW: std::sync::Mutex<Option<WindowId>> = std::sync::Mutex::new(None);

/// Handle for the low-level mouse hook.
///
/// Dropping this handle will unhook the mouse hook.
pub struct MouseHookHandle {
    hook: HHOOK,
}

impl Drop for MouseHookHandle {
    fn drop(&mut self) {
        unsafe {
            if !self.hook.is_invalid() {
                let _ = UnhookWindowsHookEx(self.hook);
            }
        }
        tracing::debug!("Mouse hook uninstalled");

        // Clear the global sender (recover from mutex poisoning)
        let mut sender = MOUSE_EVENT_SENDER
            .lock()
            .unwrap_or_else(recover_poisoned_mutex);
        *sender = None;
    }
}

/// Install a low-level mouse hook for focus-follows-mouse functionality.
///
/// Returns a handle that must be kept alive to receive mouse events,
/// and registers the given sender to receive MouseEnterWindow events.
///
/// # Arguments
/// * `event_sender` - Sender for WindowEvent (specifically MouseEnterWindow)
pub fn install_mouse_hook(
    event_sender: mpsc::Sender<WindowEvent>,
) -> Result<MouseHookHandle, Win32Error> {
    // Store sender globally
    {
        let mut sender = MOUSE_EVENT_SENDER.lock().map_err(|_| {
            Win32Error::HookInstallFailed("Mouse sender mutex poisoned".to_string())
        })?;
        if sender.is_some() {
            return Err(Win32Error::HookInstallFailed(
                "Mouse sender already initialized - drop existing MouseHookHandle first"
                    .to_string(),
            ));
        }
        *sender = Some(event_sender);
    }

    // Install low-level mouse hook
    let hook = unsafe {
        SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_ll_hook_proc), None, 0).map_err(|e| {
            Win32Error::HookInstallFailed(format!("SetWindowsHookExW failed: {}", e))
        })?
    };

    tracing::info!("Low-level mouse hook installed for focus-follows-mouse");

    Ok(MouseHookHandle { hook })
}

/// Low-level mouse hook callback.
///
/// Tracks mouse movement and sends MouseEnterWindow events when the cursor
/// enters a different window.
unsafe extern "system" fn mouse_ll_hook_proc(
    ncode: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    // If ncode < 0, we must call CallNextHookEx without processing
    if ncode < 0 {
        return CallNextHookEx(None, ncode, wparam, lparam);
    }

    // Only process mouse move events
    if wparam.0 as u32 == WM_MOUSEMOVE {
        // Get the mouse position from the hook struct
        let mouse_struct = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let point = mouse_struct.pt;

        // Find the window at the cursor position
        let raw_hwnd = WindowFromPoint(point);
        let candidate_hwnd = if raw_hwnd.is_invalid() {
            None
        } else {
            let normalized = normalize_to_root_window(raw_hwnd);
            if normalized.is_invalid() {
                None
            } else {
                Some(normalized)
            }
        };
        let candidate_window_id = candidate_hwnd.map(|hwnd| hwnd.0 as WindowId);

        // Check if this is a different top-level window than before (recover from mutex poisoning)
        let mut current = CURRENT_MOUSE_WINDOW
            .lock()
            .unwrap_or_else(recover_poisoned_mutex);
        if *current != candidate_window_id {
            *current = candidate_window_id;

            if let Some(hwnd) = candidate_hwnd {
                if should_emit_window_event(hwnd) {
                    let sender_guard = MOUSE_EVENT_SENDER
                        .lock()
                        .unwrap_or_else(recover_poisoned_mutex);
                    if let Some(sender) = sender_guard.as_ref() {
                        let _ = sender.send(WindowEvent::MouseEnterWindow(hwnd.0 as WindowId));
                    }
                }
            }
        }
    }

    // Always call next hook in the chain
    CallNextHookEx(None, ncode, wparam, lparam)
}
