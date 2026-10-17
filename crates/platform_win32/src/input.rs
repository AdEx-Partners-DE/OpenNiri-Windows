//! Low-level mouse hook: touchpad gesture detection and focus-follows-mouse.
//!
//! Every `WH_MOUSE_LL` hook adds latency to each mouse event system-wide, so both
//! features share one hook per thread. [`spawn_input_hook_thread`] runs both on
//! one thread with a single hook; [`spawn_mouse_hook_thread`] and
//! [`spawn_gesture_thread`] each start their own thread (and hook).

use crate::error::Win32Error;
use crate::events::WindowEvent;
use crate::message_loop::{spawn_message_loop_thread, MessageLoopThread};
use crate::window::{manageable_window_names, normalize_to_root_window, WindowFilter};
use openniri_core_layout::WindowId;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_XBUTTON1, VK_XBUTTON2,
};
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

/// Run focus-follows-mouse (when `focus_sender` is set) and gesture detection
/// (when `gestures` is true) on one message-loop thread with a single
/// low-level mouse hook.
///
/// Returns the gesture receiver when gestures are enabled.
pub fn spawn_input_hook_thread(
    focus_sender: Option<mpsc::Sender<WindowEvent>>,
    gestures: bool,
) -> Result<(MessageLoopThread, Option<mpsc::Receiver<GestureEvent>>), Win32Error> {
    spawn_message_loop_thread(
        "openniri-input-hook",
        move || {
            let focus = focus_sender.map(install_mouse_hook).transpose()?;
            let (gesture, receiver) = if gestures {
                let (handle, receiver) = register_gestures()?;
                (Some(handle), Some(receiver))
            } else {
                (None, None)
            };
            Ok(((focus, gesture), receiver))
        },
        |_| {},
    )
}

// ============================================================================
// Shared hook state
// ============================================================================

/// State of the low-level mouse hook installed on this thread.
///
/// The hook callback runs on the installing thread while it pumps messages, so
/// the state is thread-local. The hook is installed with the first feature and
/// removed with the last.
struct InputHookState {
    hook: Option<HHOOK>,
    focus: Option<FocusFollowsMouse>,
    gestures: Option<GestureDetector>,
}

thread_local! {
    static INPUT_HOOK: RefCell<InputHookState> = const {
        RefCell::new(InputHookState {
            hook: None,
            focus: None,
            gestures: None,
        })
    };
}

impl InputHookState {
    fn ensure_hook(&mut self) -> Result<(), Win32Error> {
        if self.hook.is_none() {
            let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), None, 0) }
                .map_err(|e| {
                    Win32Error::HookInstallFailed(format!(
                        "SetWindowsHookExW(WH_MOUSE_LL) failed: {}",
                        e
                    ))
                })?;
            self.hook = Some(hook);
        }
        Ok(())
    }

    fn release_hook_if_unused(&mut self) {
        if self.focus.is_none() && self.gestures.is_none() {
            if let Some(hook) = self.hook.take() {
                let _ = unsafe { UnhookWindowsHookEx(hook) };
            }
        }
    }

    fn handle(&mut self, message: u32, info: &MSLLHOOKSTRUCT) {
        if message == WM_MOUSEMOVE {
            if let Some(focus) = self.focus.as_mut() {
                focus.on_mouse_move(info.pt);
            }
        } else if message == WM_MOUSEWHEEL || message == WM_MOUSEHWHEEL {
            if let Some(gestures) = self.gestures.as_mut() {
                // The high word of mouseData contains the wheel delta (signed).
                let delta = (info.mouseData >> 16) as i16 as i32;
                if let Some(event) =
                    gestures.on_wheel(message == WM_MOUSEHWHEEL, delta, Instant::now())
                {
                    let _ = gestures.sender.send(event);
                }
            }
        }
    }
}

/// Install a feature into this thread's hook state, installing the hook if needed.
fn add_feature(install: impl FnOnce(&mut InputHookState) -> bool) -> Result<(), Win32Error> {
    INPUT_HOOK.with(|state| {
        let mut state = state.borrow_mut();
        if !install(&mut state) {
            return Err(Win32Error::HookInstallFailed(
                "this input hook is already installed on this thread".to_string(),
            ));
        }
        // Without a hook no other feature was active, so clearing both leaves no
        // state behind and a later install can succeed.
        state.ensure_hook().inspect_err(|_| {
            state.focus = None;
            state.gestures = None;
        })
    })
}

/// Remove a feature from this thread's hook state, removing the hook if unused.
fn remove_feature(remove: impl FnOnce(&mut InputHookState)) {
    let _ = INPUT_HOOK.try_with(|state| {
        let mut state = state.borrow_mut();
        remove(&mut state);
        state.release_hook_if_unused();
    });
}

/// Low-level mouse hook callback shared by focus-follows-mouse and gestures.
unsafe extern "system" fn mouse_hook_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if ncode >= 0 {
        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let message = wparam.0 as u32;
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = INPUT_HOOK.try_with(|state| {
                // A nested call (while this thread waits in a cross-thread
                // SendMessage) skips the event instead of panicking.
                if let Ok(mut state) = state.try_borrow_mut() {
                    state.handle(message, info);
                }
            });
        }));
    }
    CallNextHookEx(None, ncode, wparam, lparam)
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

/// If no scroll event arrives within this window, accumulators are reset.
const GESTURE_TIMEOUT: Duration = Duration::from_millis(300);

/// Wheel-delta accumulator that turns touchpad scrolling into swipes.
struct GestureDetector {
    sender: mpsc::Sender<GestureEvent>,
    accum_x: i32,
    accum_y: i32,
    last_scroll_time: Option<Instant>,
}

impl GestureDetector {
    fn on_wheel(&mut self, horizontal: bool, delta: i32, now: Instant) -> Option<GestureEvent> {
        if self
            .last_scroll_time
            .is_some_and(|last| now.duration_since(last) > GESTURE_TIMEOUT)
        {
            self.accum_x = 0;
            self.accum_y = 0;
        }
        self.last_scroll_time = Some(now);

        if horizontal {
            self.accum_x += delta;
        } else {
            self.accum_y += delta;
        }

        if self.accum_x.abs() >= GESTURE_SCROLL_THRESHOLD {
            let event = if self.accum_x > 0 {
                GestureEvent::SwipeRight
            } else {
                GestureEvent::SwipeLeft
            };
            self.accum_x = 0;
            Some(event)
        } else if self.accum_y.abs() >= GESTURE_SCROLL_THRESHOLD {
            let event = if self.accum_y > 0 {
                GestureEvent::SwipeDown
            } else {
                GestureEvent::SwipeUp
            };
            self.accum_y = 0;
            Some(event)
        } else {
            None
        }
    }
}

/// Handle for gesture detection.
///
/// Dropping this handle (on the registering thread; it is not `Send`) stops
/// gesture detection and removes the hook if focus-follows-mouse does not use it.
pub struct GestureHandle {
    _thread_affine: PhantomData<*const ()>,
}

impl Drop for GestureHandle {
    fn drop(&mut self) {
        remove_feature(|state| state.gestures = None);
        tracing::debug!("Gesture detection stopped");
    }
}

/// Register a low-level mouse hook for gesture detection via wheel events.
///
/// Returns a handle that must be kept alive to receive gesture events,
/// and a channel receiver for gesture events. Events are only detected while
/// the calling thread pumps messages; prefer [`spawn_gesture_thread`].
///
/// Touchpad scroll gestures are delivered as WM_MOUSEWHEEL (vertical) and
/// WM_MOUSEHWHEEL (horizontal) messages. The hook accumulates wheel deltas
/// and fires swipe events when the threshold is exceeded.
pub fn register_gestures() -> Result<(GestureHandle, mpsc::Receiver<GestureEvent>), Win32Error> {
    let (tx, rx) = mpsc::channel();
    add_feature(|state| {
        if state.gestures.is_some() {
            return false;
        }
        state.gestures = Some(GestureDetector {
            sender: tx,
            accum_x: 0,
            accum_y: 0,
            last_scroll_time: None,
        });
        true
    })?;
    tracing::info!("Gesture detection registered (low-level mouse hook)");
    Ok((
        GestureHandle {
            _thread_affine: PhantomData,
        },
        rx,
    ))
}

// ============================================================================
// Focus Follows Mouse
// ============================================================================

/// Tracks the top-level window under the cursor.
struct FocusFollowsMouse {
    sender: mpsc::Sender<WindowEvent>,
    current_window: Option<WindowId>,
    last_point: Option<(i32, i32)>,
}

/// Whether a mouse-move report should be hit-tested for focus-follows-mouse.
///
/// Reports at an unchanged point are noise: they also arrive while the layout
/// moves windows under a still cursor, and hit-testing them focused whichever
/// neighbour slid under the cursor (#63). While a button is held the user is
/// dragging or selecting, and focus must not follow.
fn should_hit_test_mouse_move(
    last_point: Option<(i32, i32)>,
    point: (i32, i32),
    button_held: bool,
) -> bool {
    !button_held && last_point != Some(point)
}

fn any_mouse_button_held() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2]
        .iter()
        .any(|key| unsafe { GetAsyncKeyState(i32::from(key.0)) } < 0)
}

impl FocusFollowsMouse {
    fn on_mouse_move(&mut self, pt: POINT) {
        let point = (pt.x, pt.y);
        let last_point = self.last_point.replace(point);
        if !should_hit_test_mouse_move(last_point, point, any_mouse_button_held()) {
            return;
        }

        let raw_hwnd = unsafe { WindowFromPoint(pt) };
        let candidate = if raw_hwnd.is_invalid() {
            None
        } else {
            Some(normalize_to_root_window(raw_hwnd)).filter(|hwnd| !hwnd.is_invalid())
        };
        let candidate_id = candidate.map(|hwnd| hwnd.0 as WindowId);
        if self.current_window == candidate_id {
            return;
        }
        self.current_window = candidate_id;

        if let Some(hwnd) = candidate {
            if manageable_window_names(hwnd, WindowFilter::MANAGEABLE).is_some() {
                let _ = self
                    .sender
                    .send(WindowEvent::MouseEnterWindow(hwnd.0 as WindowId));
            }
        }
    }
}

/// Handle for the focus-follows-mouse hook.
///
/// Dropping this handle (on the installing thread; it is not `Send`) stops
/// focus-follows-mouse and removes the hook if gestures do not use it.
pub struct MouseHookHandle {
    _thread_affine: PhantomData<*const ()>,
}

impl Drop for MouseHookHandle {
    fn drop(&mut self) {
        remove_feature(|state| state.focus = None);
        tracing::debug!("Mouse hook uninstalled");
    }
}

/// Install a low-level mouse hook for focus-follows-mouse functionality.
///
/// Returns a handle that must be kept alive to receive mouse events, and
/// sends `MouseEnterWindow` events to `event_sender`. Events are only produced
/// while the calling thread pumps messages; prefer [`spawn_mouse_hook_thread`].
pub fn install_mouse_hook(
    event_sender: mpsc::Sender<WindowEvent>,
) -> Result<MouseHookHandle, Win32Error> {
    add_feature(|state| {
        if state.focus.is_some() {
            return false;
        }
        state.focus = Some(FocusFollowsMouse {
            sender: event_sender,
            current_window: None,
            last_point: None,
        });
        true
    })?;
    tracing::info!("Low-level mouse hook installed for focus-follows-mouse");
    Ok(MouseHookHandle {
        _thread_affine: PhantomData,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_moves_at_an_unchanged_point_are_ignored() {
        assert!(should_hit_test_mouse_move(None, (10, 10), false));
        assert!(should_hit_test_mouse_move(Some((10, 10)), (11, 10), false));
        // The layout moved windows under a still cursor (#63).
        assert!(!should_hit_test_mouse_move(Some((10, 10)), (10, 10), false));
        // Dragging or selecting with a button held.
        assert!(!should_hit_test_mouse_move(Some((10, 10)), (50, 10), true));
    }
}
