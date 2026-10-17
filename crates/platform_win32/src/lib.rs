//! OpenNiri Platform Win32
//!
//! Windows-specific window manipulation using Win32 APIs.
//!
//! This crate handles:
//! - Window enumeration and filtering
//! - Window positioning via SetWindowPos (with DeferWindowPos batching)
//! - Window cloaking/uncloaking via DWM APIs
//! - WinEvent hooks for window lifecycle events
//! - Visual overlay for snap hints

pub mod overlay;
pub mod pipe_security;
mod shell_cloak;

mod cloak;
mod error;
mod events;
mod focus;
mod hotkeys;
mod input;
mod message_loop;
mod monitors;
mod offscreen;
mod placement;
#[cfg(test)]
mod test_support;
mod window;

// `shell_cloak` refers to `crate::WindowId`.
use openniri_core_layout::WindowId;

pub use cloak::{
    cloak_access_denied, cloak_window, restore_journaled_cloaked_windows,
    uncloak_all_managed_windows, uncloak_all_visible_windows, uncloak_window,
};
pub use error::Win32Error;
pub use events::{install_event_hooks, spawn_event_hook_thread, EventHookHandle, WindowEvent};
pub use focus::{
    close_window, reset_window_border_color, set_foreground_window, set_window_border_color,
};
pub use hotkeys::{
    clear_display_change_sender, parse_hotkey_string, parse_vk, register_hotkeys,
    set_display_change_sender, spawn_display_watcher, vk, Hotkey, HotkeyEvent, HotkeyHandle,
    HotkeyId, Modifiers,
};
pub use input::{
    install_mouse_hook, register_gestures, spawn_gesture_thread, spawn_input_hook_thread,
    spawn_mouse_hook_thread, GestureEvent, GestureHandle, MouseHookHandle,
};
pub use message_loop::{spawn_message_loop_thread, MessageLoopThread};
pub use monitors::{
    enumerate_monitors, find_monitor_by_id, find_monitor_for_rect, get_primary_monitor,
    monitor_to_left, monitor_to_right, monitors_by_position, set_dpi_awareness, MonitorId,
    MonitorInfo,
};
pub use offscreen::{
    bring_windows_into_view, is_move_offscreen_sentinel_position, is_move_offscreen_sentinel_rect,
    restore_all_windows_moved_offscreen_best_effort, restore_window_moved_offscreen,
    restore_windows_moved_offscreen, MOVE_OFFSCREEN_SENTINEL_COORD,
};
pub use placement::{apply_placements, apply_placements_with_focus, HideStrategy, PlatformConfig};
pub use window::{
    describe_window, enumerate_windows, get_process_executable, is_process_more_privileged,
    is_valid_window, WindowInfo,
};
