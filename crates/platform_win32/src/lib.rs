//! OpenNiri Platform Win32
//!
//! Windows-specific window manipulation using Win32 APIs.
//!
//! This crate handles:
//! - Window enumeration and filtering
//! - Window positioning via SetWindowPos (with DeferWindowPos batching)
//! - Window cloaking/uncloaking via DWM APIs
//! - WinEvent hooks for window lifecycle events

use openniri_core_layout::{Rect, Visibility, WindowId, WindowPlacement};
use thiserror::Error;

/// Errors that can occur during Win32 operations.
#[derive(Debug, Error)]
pub enum Win32Error {
    #[error("Failed to enumerate windows: {0}")]
    EnumerationFailed(String),

    #[error("Failed to set window position: {0}")]
    SetPositionFailed(String),

    #[error("Failed to cloak window: {0}")]
    CloakFailed(String),

    #[error("Failed to install event hook: {0}")]
    HookInstallFailed(String),

    #[error("Window not found: {0}")]
    WindowNotFound(WindowId),
}

/// Information about a managed window.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// The window handle (HWND) as u64.
    pub hwnd: WindowId,
    /// Window title.
    pub title: String,
    /// Window class name.
    pub class_name: String,
    /// Process ID.
    pub process_id: u32,
    /// Current window rectangle.
    pub rect: Rect,
    /// Whether the window is visible.
    pub visible: bool,
}

/// Strategy for hiding off-screen windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HideStrategy {
    /// Use DWM cloaking (preferred, keeps window in Alt-Tab).
    #[default]
    Cloak,
    /// Minimize the window.
    Minimize,
    /// Move off-screen (may cause DWM to stop rendering).
    MoveOffScreen,
}

/// Configuration for the Win32 platform layer.
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    /// Strategy for hiding off-screen windows.
    pub hide_strategy: HideStrategy,
    /// Buffer zone size in pixels (windows in this zone are kept uncloaked).
    pub buffer_zone: i32,
    /// Whether to use DeferWindowPos for batched moves.
    pub use_deferred_positioning: bool,
}

impl Default for PlatformConfig {
    fn default() -> Self {
        Self {
            hide_strategy: HideStrategy::default(),
            buffer_zone: 1000,
            use_deferred_positioning: true,
        }
    }
}

/// Enumerate all top-level windows that should be managed.
///
/// Filters out:
/// - Invisible windows
/// - Tool windows
/// - Windows with empty titles
/// - System windows (taskbar, etc.)
pub fn enumerate_windows() -> Result<Vec<WindowInfo>, Win32Error> {
    // TODO: Implement using windows-rs
    // - EnumWindows callback
    // - Filter by WS_VISIBLE, !WS_EX_TOOLWINDOW
    // - GetWindowText, GetClassName
    // - GetWindowThreadProcessId
    // - GetWindowRect
    tracing::warn!("enumerate_windows not yet implemented");
    Ok(Vec::new())
}

/// Apply window placements from the layout engine.
///
/// This function:
/// 1. Groups placements by visibility
/// 2. Uses DeferWindowPos for visible windows (batched move)
/// 3. Applies cloaking/uncloaking based on visibility changes
pub fn apply_placements(
    placements: &[WindowPlacement],
    _config: &PlatformConfig,
) -> Result<(), Win32Error> {
    // TODO: Implement using windows-rs
    // - BeginDeferWindowPos
    // - DeferWindowPos for each visible window
    // - EndDeferWindowPos
    // - DwmSetWindowAttribute(DWMWA_CLOAK) for off-screen windows
    tracing::warn!("apply_placements not yet implemented");

    for placement in placements {
        match placement.visibility {
            Visibility::Visible => {
                tracing::debug!(
                    "Would move window {} to ({}, {})",
                    placement.window_id,
                    placement.rect.x,
                    placement.rect.y
                );
            }
            Visibility::OffScreenLeft | Visibility::OffScreenRight => {
                tracing::debug!("Would cloak window {}", placement.window_id);
            }
        }
    }

    Ok(())
}

/// Cloak a window (hide from view but keep in Alt-Tab).
pub fn cloak_window(_hwnd: WindowId) -> Result<(), Win32Error> {
    // TODO: Implement using DwmSetWindowAttribute with DWMWA_CLOAK
    tracing::warn!("cloak_window not yet implemented");
    Ok(())
}

/// Uncloak a window (make visible again).
pub fn uncloak_window(_hwnd: WindowId) -> Result<(), Win32Error> {
    // TODO: Implement using DwmSetWindowAttribute with DWMWA_CLOAK
    tracing::warn!("uncloak_window not yet implemented");
    Ok(())
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
    /// A window was moved or resized by the user.
    MovedOrResized(WindowId),
}

/// Handle for installed event hooks.
pub struct EventHookHandle {
    // TODO: Store HWINEVENTHOOK handles
    _private: (),
}

impl Drop for EventHookHandle {
    fn drop(&mut self) {
        // TODO: UnhookWinEvent
    }
}

/// Install WinEvent hooks to receive window lifecycle events.
///
/// Returns a handle that must be kept alive to receive events.
/// Events are sent to the provided callback.
pub fn install_event_hooks<F>(_callback: F) -> Result<EventHookHandle, Win32Error>
where
    F: Fn(WindowEvent) + Send + 'static,
{
    // TODO: Implement using SetWinEventHook
    // - EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY
    // - EVENT_SYSTEM_FOREGROUND
    // - EVENT_OBJECT_LOCATIONCHANGE
    // - EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND
    tracing::warn!("install_event_hooks not yet implemented");
    Ok(EventHookHandle { _private: () })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_config_default() {
        let config = PlatformConfig::default();
        assert_eq!(config.hide_strategy, HideStrategy::Cloak);
        assert_eq!(config.buffer_zone, 1000);
        assert!(config.use_deferred_positioning);
    }
}
