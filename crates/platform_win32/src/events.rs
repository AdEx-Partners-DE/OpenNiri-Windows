//! WinEvent hooks: window lifecycle events, their target and emit policies, and the
//! hook thread.

use crate::cloak::track_daemon_cloak;
use crate::error::{recover_poisoned_mutex, Win32Error};
use crate::message_loop::{spawn_message_loop_thread, MessageLoopThread};
use crate::window::{
    is_window_cloaked, normalize_to_root_window, should_skip_window_by_class,
    should_skip_window_by_title,
};
use openniri_core_layout::WindowId;
use std::sync::mpsc;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetWindow, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW,
    IsWindowVisible, GWL_EXSTYLE, GWL_STYLE, GW_OWNER, WS_EX_APPWINDOW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_VISIBLE,
};

// WinEvent constants (not all are exposed by windows-rs)
const EVENT_OBJECT_CREATE: u32 = 0x8000;
const EVENT_OBJECT_DESTROY: u32 = 0x8001;
/// Most applications create their top-level window hidden and show it afterwards,
/// so `EVENT_OBJECT_CREATE` alone usually sees an invisible window.
const EVENT_OBJECT_SHOW: u32 = 0x8002;
/// UWP frames become visible by being uncloaked rather than shown.
const EVENT_OBJECT_UNCLOAKED: u32 = 0x8018;
const EVENT_OBJECT_FOCUS: u32 = 0x8005;
const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x0016;
const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x0017;
/// End of an interactive move or resize. `EVENT_OBJECT_LOCATIONCHANGE` is not hooked:
/// it fires continuously while a window is dragged and for every programmatic move,
/// and snapping back on each one fights the user mid-drag.
const EVENT_SYSTEM_MOVESIZEEND: u32 = 0x000B;
const OBJID_WINDOW: i32 = 0;
const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;
/// `CHILDID_SELF`: the event is about the object itself, not one of its children.
const CHILDID_SELF: i32 = 0;
const WINEVENT_SKIPOWNPROCESS: u32 = 0x0002;

/// Apply manageability filters to a window handle for WinEvent callback emission.
///
/// This mirrors enumeration filters so event callbacks don't emit churn for
/// windows we would never manage.
fn should_emit_window_event_with_policy(
    hwnd: HWND,
    require_visible: bool,
    require_title: bool,
    filter_cloaked: bool,
) -> bool {
    // Keep callback work best-effort and non-panicking.
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) as u32 };
    let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 };

    if require_visible && !unsafe { IsWindowVisible(hwnd) }.as_bool() {
        return false;
    }

    if require_visible && style & WS_VISIBLE.0 == 0 {
        return false;
    }

    let is_tool_window = ex_style & WS_EX_TOOLWINDOW.0 != 0;
    let is_app_window = ex_style & WS_EX_APPWINDOW.0 != 0;
    if is_tool_window && !is_app_window {
        return false;
    }

    if ex_style & WS_EX_NOACTIVATE.0 != 0 {
        return false;
    }

    if let Ok(owner) = unsafe { GetWindow(hwnd, GW_OWNER) } {
        if !owner.is_invalid() {
            return false;
        }
    }

    if filter_cloaked && is_window_cloaked(hwnd) {
        return false;
    }

    if require_title {
        let title_len = unsafe { GetWindowTextLengthW(hwnd) };
        if title_len == 0 {
            return false;
        }

        let mut title_buf: Vec<u16> = vec![0; (title_len + 1) as usize];
        let actual_len = unsafe { GetWindowTextW(hwnd, &mut title_buf) };
        if actual_len == 0 {
            return false;
        }
        let title = String::from_utf16_lossy(&title_buf[..actual_len as usize]);
        if should_skip_window_by_title(&title) {
            return false;
        }
    }

    let mut class_buf: Vec<u16> = vec![0; 256];
    let class_len = unsafe { GetClassNameW(hwnd, &mut class_buf) };
    let class_name = if class_len > 0 {
        String::from_utf16_lossy(&class_buf[..class_len as usize])
    } else {
        String::new()
    };

    if should_skip_window_by_class(&class_name) {
        return false;
    }

    true
}

pub(crate) fn should_emit_window_event(hwnd: HWND) -> bool {
    should_emit_window_event_with_policy(
        hwnd, true, // visible only
        true, // non-empty title
        true, // filter cloaked windows
    )
}

fn should_emit_window_event_for(event: u32, hwnd: HWND) -> bool {
    match event {
        // Creation can happen before title is set; keep manageability checks but
        // allow empty title and cloaked transitional states.
        EVENT_OBJECT_CREATE | EVENT_OBJECT_SHOW | EVENT_OBJECT_UNCLOAKED => {
            should_emit_window_event_with_policy(hwnd, true, false, false)
        }
        // Focus must not be blocked by cloaked-state checks or empty titles.
        EVENT_SYSTEM_FOREGROUND | EVENT_OBJECT_FOCUS => {
            should_emit_window_event_with_policy(hwnd, true, false, false)
        }
        // Restore/minimize/destroy should still pass basic top-level filtering,
        // but visibility/title can be transient during these transitions.
        EVENT_SYSTEM_MINIMIZESTART | EVENT_SYSTEM_MINIMIZEEND | EVENT_OBJECT_DESTROY => {
            should_emit_window_event_with_policy(hwnd, false, false, false)
        }
        EVENT_SYSTEM_MOVESIZEEND => should_emit_window_event_with_policy(hwnd, true, true, true),
        _ => false,
    }
}

fn should_filter_window_event_by_manageability(event: u32) -> bool {
    matches!(
        event,
        EVENT_OBJECT_CREATE
            | EVENT_OBJECT_SHOW
            | EVENT_OBJECT_UNCLOAKED
            | EVENT_OBJECT_DESTROY
            | EVENT_SYSTEM_FOREGROUND
            | EVENT_SYSTEM_MINIMIZESTART
            | EVENT_SYSTEM_MINIMIZEEND
            | EVENT_SYSTEM_MOVESIZEEND
            | EVENT_OBJECT_FOCUS
    )
}

/// How a WinEvent's HWND maps to the window the daemon manages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventTargetPolicy {
    /// Focus-style events: a focused child control means its top-level window has focus.
    NormalizeToRoot,
    /// Create/show/move events for child windows (tabs, browser panes) are ignored.
    TopLevelOnly,
    /// Destroy events keep the original HWND. A destroyed window can no longer be
    /// mapped to its root, and normalizing a destroyed child used to remove its
    /// still-open top-level window from management.
    Exact,
}

fn event_target_policy(event: u32) -> EventTargetPolicy {
    match event {
        EVENT_OBJECT_CREATE
        | EVENT_OBJECT_SHOW
        | EVENT_OBJECT_UNCLOAKED
        | EVENT_SYSTEM_MOVESIZEEND => EventTargetPolicy::TopLevelOnly,
        EVENT_OBJECT_DESTROY => EventTargetPolicy::Exact,
        _ => EventTargetPolicy::NormalizeToRoot,
    }
}

/// Window event types that the daemon needs to handle.
#[derive(Debug, Clone)]
pub enum WindowEvent {
    /// A new window was created.
    Created(WindowId),
    /// A window was destroyed.
    Destroyed(WindowId),
    /// A window received focus.
    Focused(WindowId),
    /// A window was minimized.
    Minimized(WindowId),
    /// A window was restored from minimized state.
    Restored(WindowId),
    /// The user finished moving or resizing a window.
    MovedOrResized(WindowId),
    /// Display configuration changed (monitors added/removed/rearranged).
    DisplayChange,
    /// Mouse cursor entered a window (for focus-follows-mouse).
    MouseEnterWindow(WindowId),
}

/// Global sender for window events from WinEvent callbacks.
///
/// Out-of-context WinEvent callbacks run on the thread that installed the hooks,
/// during its message loop (see [`spawn_event_hook_thread`]). The channel
/// forwards events from that thread to the async runtime.
static EVENT_SENDER: std::sync::Mutex<Option<mpsc::Sender<WindowEvent>>> =
    std::sync::Mutex::new(None);

fn set_event_sender(sender: mpsc::Sender<WindowEvent>) -> Result<(), Win32Error> {
    let mut guard = EVENT_SENDER
        .lock()
        .map_err(|_| Win32Error::HookInstallFailed("Event sender mutex poisoned".to_string()))?;
    if guard.is_some() {
        return Err(Win32Error::HookInstallFailed(
            "Event sender already initialized - drop existing EventHookHandle first".to_string(),
        ));
    }
    *guard = Some(sender);
    Ok(())
}

fn clear_event_sender() {
    let mut guard = EVENT_SENDER.lock().unwrap_or_else(recover_poisoned_mutex);
    *guard = None;
}

fn clone_event_sender() -> Option<mpsc::Sender<WindowEvent>> {
    let guard = EVENT_SENDER.lock().unwrap_or_else(recover_poisoned_mutex);
    guard.as_ref().cloned()
}

/// Handle for installed event hooks.
///
/// Dropping this handle will unhook all installed event hooks.
pub struct EventHookHandle {
    hooks: Vec<HWINEVENTHOOK>,
}

impl Drop for EventHookHandle {
    fn drop(&mut self) {
        for hook in &self.hooks {
            unsafe {
                if !UnhookWinEvent(*hook).as_bool() {
                    tracing::warn!("Failed to unhook WinEvent: {:?}", hook);
                }
            }
        }
        clear_event_sender();
        tracing::debug!("Unhooked {} WinEvent hooks", self.hooks.len());
    }
}

/// Install WinEvent hooks to receive window lifecycle events.
///
/// Returns a handle that must be kept alive to receive events.
/// Also returns a receiver channel for the events.
///
/// # Events Hooked
/// - Window creation and first show (EVENT_OBJECT_CREATE, EVENT_OBJECT_SHOW,
///   EVENT_OBJECT_UNCLOAKED), all reported as `WindowEvent::Created`
/// - Window destruction (EVENT_OBJECT_DESTROY)
/// - Foreground change (EVENT_SYSTEM_FOREGROUND)
/// - Minimize/restore (EVENT_SYSTEM_MINIMIZESTART/END)
/// - End of a user move/resize (EVENT_SYSTEM_MOVESIZEEND)
pub fn install_event_hooks() -> Result<(EventHookHandle, mpsc::Receiver<WindowEvent>), Win32Error> {
    // Create channel for events
    let (tx, rx) = mpsc::channel();

    // Store sender globally for callback access
    set_event_sender(tx)?;

    let mut hooks = Vec::new();

    // Define events to hook: (min_event, max_event)
    let event_ranges = [
        (EVENT_OBJECT_CREATE, EVENT_OBJECT_SHOW), // Create/Destroy/Show
        (EVENT_OBJECT_UNCLOAKED, EVENT_OBJECT_UNCLOAKED), // UWP frames appearing
        (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND), // Foreground
        (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND), // Minimize
        (EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZEEND), // User finished move/resize
        (EVENT_OBJECT_FOCUS, EVENT_OBJECT_FOCUS), // Focus within app
    ];

    unsafe {
        for (min_event, max_event) in event_ranges {
            let hook = SetWinEventHook(
                min_event,
                max_event,
                None,                     // No DLL, use callback
                Some(win_event_callback), // Our callback
                0,                        // All processes
                0,                        // All threads
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );

            if hook.is_invalid() {
                // Clean up any hooks we've installed
                for h in &hooks {
                    let _ = UnhookWinEvent(*h);
                }
                clear_event_sender();
                return Err(Win32Error::HookInstallFailed(format!(
                    "SetWinEventHook failed for events {}-{}",
                    min_event, max_event
                )));
            }

            hooks.push(hook);
        }
    }

    tracing::info!("Installed {} WinEvent hooks", hooks.len());
    Ok((EventHookHandle { hooks }, rx))
}

/// Callback function for WinEvent hooks.
///
/// Runs on the hook thread's message loop; events are forwarded to the channel.
/// Wrapped with catch_unwind to prevent panics from crashing the application.
unsafe extern "system" fn win_event_callback(
    hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    id_event_thread: u32,
    dwms_event_time: u32,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        win_event_callback_inner(
            hook,
            event,
            hwnd,
            id_object,
            id_child,
            id_event_thread,
            dwms_event_time,
        )
    }));

    if let Err(e) = result {
        // Can't use tracing here safely in all contexts, use eprintln
        eprintln!("Panic in win_event_callback: {:?}", e);
    }
}

/// Inner implementation of WinEvent callback.
fn win_event_callback_inner(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    // Only handle events about the window itself (not child objects like menus)
    if id_object != OBJID_WINDOW || id_child != CHILDID_SELF {
        return;
    }

    // Ignore invalid HWNDs
    if hwnd.0.is_null() {
        return;
    }

    let hwnd = match event_target_policy(event) {
        EventTargetPolicy::NormalizeToRoot => normalize_to_root_window(hwnd),
        EventTargetPolicy::TopLevelOnly => {
            if normalize_to_root_window(hwnd) != hwnd {
                return;
            }
            hwnd
        }
        EventTargetPolicy::Exact => hwnd,
    };

    if should_filter_window_event_by_manageability(event)
        && !should_emit_window_event_for(event, hwnd)
    {
        return;
    }

    let window_id = hwnd.0 as WindowId;

    // Map event to our WindowEvent type
    let window_event = match event {
        // The daemon ignores Created for windows it already manages.
        EVENT_OBJECT_CREATE | EVENT_OBJECT_SHOW | EVENT_OBJECT_UNCLOAKED => {
            WindowEvent::Created(window_id)
        }
        EVENT_OBJECT_DESTROY => {
            track_daemon_cloak(window_id, false);
            WindowEvent::Destroyed(window_id)
        }
        EVENT_SYSTEM_FOREGROUND | EVENT_OBJECT_FOCUS => WindowEvent::Focused(window_id),
        EVENT_SYSTEM_MINIMIZESTART => WindowEvent::Minimized(window_id),
        EVENT_SYSTEM_MINIMIZEEND => WindowEvent::Restored(window_id),
        EVENT_SYSTEM_MOVESIZEEND => WindowEvent::MovedOrResized(window_id),
        _ => return,
    };

    // Send event through channel
    if let Some(sender) = clone_event_sender() {
        let _ = sender.send(window_event);
    }
}

/// Install WinEvent hooks on a dedicated message-loop thread.
///
/// Prefer this over [`install_event_hooks`] in long-running code: the hooks only
/// deliver events while their thread pumps messages.
pub fn spawn_event_hook_thread(
) -> Result<(MessageLoopThread, mpsc::Receiver<WindowEvent>), Win32Error> {
    spawn_message_loop_thread("openniri-winevent-hooks", install_event_hooks, |_| {})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::GLOBAL_SENDER_TEST_LOCK;

    #[test]
    fn test_event_sender_can_be_reinstalled_after_clear() {
        let _guard = GLOBAL_SENDER_TEST_LOCK
            .lock()
            .unwrap_or_else(recover_poisoned_mutex);
        clear_event_sender();

        let (first_tx, _first_rx) = mpsc::channel::<WindowEvent>();
        assert!(set_event_sender(first_tx).is_ok());

        let (second_tx, _second_rx) = mpsc::channel::<WindowEvent>();
        let err = set_event_sender(second_tx).unwrap_err();
        assert!(matches!(err, Win32Error::HookInstallFailed(_)));

        clear_event_sender();

        let (third_tx, _third_rx) = mpsc::channel::<WindowEvent>();
        assert!(set_event_sender(third_tx).is_ok());
        clear_event_sender();
    }

    #[test]
    fn test_event_target_policy_keeps_destroy_events_exact() {
        let cases = [
            (EVENT_OBJECT_DESTROY, EventTargetPolicy::Exact),
            (EVENT_OBJECT_CREATE, EventTargetPolicy::TopLevelOnly),
            (EVENT_OBJECT_SHOW, EventTargetPolicy::TopLevelOnly),
            (EVENT_OBJECT_UNCLOAKED, EventTargetPolicy::TopLevelOnly),
            (EVENT_SYSTEM_MOVESIZEEND, EventTargetPolicy::TopLevelOnly),
            (EVENT_SYSTEM_FOREGROUND, EventTargetPolicy::NormalizeToRoot),
            (EVENT_OBJECT_FOCUS, EventTargetPolicy::NormalizeToRoot),
        ];
        for (event, policy) in cases {
            assert_eq!(event_target_policy(event), policy, "event {event:#x}");
        }

        // Every hooked event goes through the manageability filter.
        for event in [
            EVENT_OBJECT_CREATE,
            EVENT_OBJECT_SHOW,
            EVENT_OBJECT_UNCLOAKED,
            EVENT_SYSTEM_MOVESIZEEND,
            EVENT_SYSTEM_FOREGROUND,
            EVENT_OBJECT_FOCUS,
            EVENT_OBJECT_DESTROY,
            EVENT_SYSTEM_MINIMIZESTART,
            EVENT_SYSTEM_MINIMIZEEND,
        ] {
            assert!(
                should_filter_window_event_by_manageability(event),
                "event {event:#x}"
            );
        }
    }
}
