//! WinEvent hooks: window lifecycle events, their target and emit policies, and the
//! hook thread.

use crate::cloak::untrack_daemon_cloak;
use crate::error::Win32Error;
use crate::message_loop::{spawn_message_loop_thread, MessageLoopThread};
use crate::window::{manageable_window_names, normalize_to_root_window, WindowFilter};
use openniri_core_layout::WindowId;
use std::cell::RefCell;
use std::sync::mpsc;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};

// WinEvent constants (not all are exposed by windows-rs)
const EVENT_OBJECT_CREATE: u32 = 0x8000;
const EVENT_OBJECT_DESTROY: u32 = 0x8001;
/// Most applications create their top-level window hidden and show it afterwards,
/// so `EVENT_OBJECT_CREATE` alone usually sees an invisible window.
const EVENT_OBJECT_SHOW: u32 = 0x8002;
/// UWP frames become visible by being uncloaked rather than shown.
const EVENT_OBJECT_UNCLOAKED: u32 = 0x8018;
// EVENT_OBJECT_FOCUS (0x8005) is deliberately not hooked: the system raises it
// with OBJID_CLIENT (also for top-level windows, verified on Windows 11), so the
// OBJID_WINDOW filter below discarded every one of them, while each keyboard
// focus change in every application still woke the hook thread. Top-level
// activation is reported by EVENT_SYSTEM_FOREGROUND.
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

/// Manageability checks a window must pass before an event about it is emitted,
/// or `None` for events that are not forwarded.
fn event_window_filter(event: u32) -> Option<WindowFilter> {
    let filter = |require_visible, require_title, filter_cloaked| WindowFilter {
        require_visible,
        require_title,
        filter_cloaked,
    };
    match event {
        // Creation can happen before the title is set, and UWP frames are still
        // cloaked while they appear. Focus must not be blocked by either.
        EVENT_OBJECT_CREATE
        | EVENT_OBJECT_SHOW
        | EVENT_OBJECT_UNCLOAKED
        | EVENT_SYSTEM_FOREGROUND => Some(filter(true, false, false)),
        // Visibility and title can be transient during these transitions.
        EVENT_SYSTEM_MINIMIZESTART | EVENT_SYSTEM_MINIMIZEEND | EVENT_OBJECT_DESTROY => {
            Some(filter(false, false, false))
        }
        EVENT_SYSTEM_MOVESIZEEND => Some(WindowFilter::MANAGEABLE),
        _ => None,
    }
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

thread_local! {
    /// Sender for events of the hooks installed on this thread.
    ///
    /// Out-of-context WinEvent callbacks run on the thread that installed the
    /// hooks, during its message loop (see [`spawn_event_hook_thread`]), so the
    /// sender lives in that thread's storage.
    static EVENT_SENDER: RefCell<Option<mpsc::Sender<WindowEvent>>> =
        const { RefCell::new(None) };
}

/// Handle for installed event hooks.
///
/// Dropping this handle (on the installing thread; the handle is not `Send`)
/// unhooks all installed event hooks.
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
        let _ = EVENT_SENDER.try_with(|sender| sender.borrow_mut().take());
        tracing::debug!("Unhooked {} WinEvent hooks", self.hooks.len());
    }
}

/// Install WinEvent hooks to receive window lifecycle events.
///
/// Returns a handle that must be kept alive to receive events.
/// Also returns a receiver channel for the events. Events are only delivered
/// while the calling thread pumps messages; prefer [`spawn_event_hook_thread`].
///
/// # Events Hooked
/// - Window creation and first show (EVENT_OBJECT_CREATE, EVENT_OBJECT_SHOW,
///   EVENT_OBJECT_UNCLOAKED), all reported as `WindowEvent::Created`
/// - Window destruction (EVENT_OBJECT_DESTROY)
/// - Foreground change (EVENT_SYSTEM_FOREGROUND)
/// - Minimize/restore (EVENT_SYSTEM_MINIMIZESTART/END)
/// - End of a user move/resize (EVENT_SYSTEM_MOVESIZEEND)
pub fn install_event_hooks() -> Result<(EventHookHandle, mpsc::Receiver<WindowEvent>), Win32Error> {
    if EVENT_SENDER.with(|sender| sender.borrow().is_some()) {
        return Err(Win32Error::HookInstallFailed(
            "WinEvent hooks are already installed on this thread".to_string(),
        ));
    }

    // Define events to hook: (min_event, max_event)
    let event_ranges = [
        (EVENT_OBJECT_CREATE, EVENT_OBJECT_SHOW), // Create/Destroy/Show
        (EVENT_OBJECT_UNCLOAKED, EVENT_OBJECT_UNCLOAKED), // UWP frames appearing
        (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND), // Foreground
        (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND), // Minimize
        (EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZEEND), // User finished move/resize
    ];

    // Callbacks are only delivered while this thread pumps messages, so the
    // sender can be stored after all hooks are in place.
    let mut handle = EventHookHandle { hooks: Vec::new() };
    for (min_event, max_event) in event_ranges {
        let hook = unsafe {
            SetWinEventHook(
                min_event,
                max_event,
                None,                     // No DLL, use callback
                Some(win_event_callback), // Our callback
                0,                        // All processes
                0,                        // All threads
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        if hook.is_invalid() {
            // Dropping the handle removes the hooks installed so far.
            return Err(Win32Error::HookInstallFailed(format!(
                "SetWinEventHook failed for events {}-{}",
                min_event, max_event
            )));
        }
        handle.hooks.push(hook);
    }

    let (tx, rx) = mpsc::channel();
    EVENT_SENDER.with(|sender| *sender.borrow_mut() = Some(tx));
    tracing::info!("Installed {} WinEvent hooks", handle.hooks.len());
    Ok((handle, rx))
}

/// Callback function for WinEvent hooks.
///
/// Runs on the hook thread's message loop; events are forwarded to the channel.
/// Wrapped with catch_unwind to prevent panics from crashing the application.
unsafe extern "system" fn win_event_callback(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    let result = std::panic::catch_unwind(|| {
        if let Some(window_event) = translate_win_event(event, hwnd, id_object, id_child) {
            let _ = EVENT_SENDER.try_with(|sender| {
                if let Some(sender) = sender.borrow().as_ref() {
                    let _ = sender.send(window_event);
                }
            });
        }
    });

    if let Err(e) = result {
        // Can't use tracing here safely in all contexts, use eprintln
        eprintln!("Panic in win_event_callback: {:?}", e);
    }
}

/// Map a raw WinEvent to the [`WindowEvent`] the daemon should see, if any.
fn translate_win_event(
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
) -> Option<WindowEvent> {
    // Only handle events about the window itself (not child objects like menus)
    if id_object != OBJID_WINDOW || id_child != CHILDID_SELF || hwnd.0.is_null() {
        return None;
    }

    let hwnd = match event_target_policy(event) {
        EventTargetPolicy::NormalizeToRoot => normalize_to_root_window(hwnd),
        EventTargetPolicy::TopLevelOnly => {
            if normalize_to_root_window(hwnd) != hwnd {
                return None;
            }
            hwnd
        }
        EventTargetPolicy::Exact => hwnd,
    };

    manageable_window_names(hwnd, event_window_filter(event)?)?;

    let window_id = hwnd.0 as WindowId;
    Some(match event {
        // The daemon ignores Created for windows it already manages.
        EVENT_OBJECT_CREATE | EVENT_OBJECT_SHOW | EVENT_OBJECT_UNCLOAKED => {
            WindowEvent::Created(window_id)
        }
        EVENT_OBJECT_DESTROY => {
            untrack_daemon_cloak(window_id);
            WindowEvent::Destroyed(window_id)
        }
        EVENT_SYSTEM_FOREGROUND => WindowEvent::Focused(window_id),
        EVENT_SYSTEM_MINIMIZESTART => WindowEvent::Minimized(window_id),
        EVENT_SYSTEM_MINIMIZEEND => WindowEvent::Restored(window_id),
        EVENT_SYSTEM_MOVESIZEEND => WindowEvent::MovedOrResized(window_id),
        _ => return None,
    })
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

    #[test]
    fn test_event_hooks_can_be_reinstalled_on_the_same_thread() {
        let (first, _first_rx) = install_event_hooks().expect("first install");
        assert!(
            matches!(install_event_hooks(), Err(Win32Error::HookInstallFailed(_))),
            "a second set of hooks on one thread would duplicate every event"
        );
        drop(first);
        let (second, _second_rx) = install_event_hooks().expect("reinstall after drop");
        drop(second);
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
        ];
        for (event, policy) in cases {
            assert_eq!(event_target_policy(event), policy, "event {event:#x}");
        }
    }
}
