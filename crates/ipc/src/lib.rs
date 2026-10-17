//! OpenNiri IPC Protocol
//!
//! Shared types for daemon-CLI communication over Windows named pipes.

use serde::{Deserialize, Serialize};

/// Named pipe path for IPC communication.
pub const PIPE_NAME: &str = r"\\.\pipe\openniri";
/// Maximum length of sanitized user scope appended to the pipe path.
const MAX_PIPE_SCOPE_SEGMENT_LEN: usize = 64;

/// IPC protocol version for lightweight compatibility checks.
pub const IPC_PROTOCOL_VERSION: u32 = 1;
/// Minimum protocol version this crate supports.
pub const IPC_MIN_SUPPORTED_PROTOCOL_VERSION: u32 = 1;

/// Maximum IPC message size (64 KiB). Messages larger than this are rejected.
pub const MAX_IPC_MESSAGE_SIZE: usize = 64 * 1024;

fn sanitize_pipe_scope_segment(scope: &str) -> String {
    let mut sanitized = String::with_capacity(scope.len());
    for ch in scope.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            sanitized.push(ch.to_ascii_lowercase());
        } else {
            sanitized.push('_');
        }
        if sanitized.len() >= MAX_PIPE_SCOPE_SEGMENT_LEN {
            break;
        }
    }
    sanitized.trim_matches('_').to_string()
}

/// Build a user-scoped pipe name from an arbitrary user/domain scope string.
pub fn scoped_pipe_name_for_user(scope: &str) -> String {
    let segment = sanitize_pipe_scope_segment(scope);
    if segment.is_empty() {
        PIPE_NAME.to_string()
    } else {
        format!("{PIPE_NAME}_{segment}")
    }
}

/// Preferred pipe name for this process/user with legacy fallback available.
///
/// Resolution order:
/// 1. `OPENNIRI_PIPE_SCOPE` environment override
/// 2. `USERDOMAIN\\USERNAME`
/// 3. legacy global `PIPE_NAME`
pub fn preferred_pipe_name() -> String {
    if let Ok(scope) = std::env::var("OPENNIRI_PIPE_SCOPE") {
        let scoped = scoped_pipe_name_for_user(&scope);
        if scoped != PIPE_NAME {
            return scoped;
        }
    }

    let domain = std::env::var("USERDOMAIN").ok();
    let user = std::env::var("USERNAME").ok();
    match (domain, user) {
        (Some(domain), Some(user)) if !domain.trim().is_empty() && !user.trim().is_empty() => {
            scoped_pipe_name_for_user(&format!("{domain}\\{user}"))
        }
        _ => PIPE_NAME.to_string(),
    }
}

/// Pipe names a client tries, in order.
///
/// The legacy global `PIPE_NAME` is no longer tried when a per-user name is
/// available: any local user can create a pipe with that global name.
pub fn pipe_name_candidates() -> Vec<String> {
    vec![preferred_pipe_name()]
}

/// Return the protocol identifier for this crate version.
pub fn protocol_id() -> u32 {
    IPC_PROTOCOL_VERSION
}

/// Whether a remote protocol version is supported by this crate.
pub fn is_protocol_version_supported(version: u32) -> bool {
    (IPC_MIN_SUPPORTED_PROTOCOL_VERSION..=IPC_PROTOCOL_VERSION).contains(&version)
}

/// Rectangle for IPC serialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpcRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl IpcRect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Detailed information about a window for IPC queries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowInfo {
    /// The window handle as a unique identifier.
    pub window_id: u64,
    /// The window's current title.
    pub title: String,
    /// The window's class name.
    pub class_name: String,
    /// The process ID that owns this window.
    pub process_id: u32,
    /// The executable name (e.g., "notepad.exe").
    pub executable: String,
    /// The window's current rectangle (position and size).
    pub rect: IpcRect,
    /// The column index if tiled, None if floating.
    pub column_index: Option<usize>,
    /// The window index within its column, None if floating.
    pub window_index: Option<usize>,
    /// The monitor ID this window is on.
    pub monitor_id: i64,
    /// Whether this window is floating (not tiled).
    pub is_floating: bool,
    /// Whether this window currently has focus.
    pub is_focused: bool,
}

/// Commands that can be sent from the CLI to the daemon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcCommand {
    /// Focus the column to the left.
    FocusLeft,
    /// Focus the column to the right.
    FocusRight,
    /// Focus the window above (in stacked columns).
    FocusUp,
    /// Focus the window below (in stacked columns).
    FocusDown,

    /// Move the focused column left.
    MoveColumnLeft,
    /// Move the focused column right.
    MoveColumnRight,

    /// Focus the monitor to the left.
    FocusMonitorLeft,
    /// Focus the monitor to the right.
    FocusMonitorRight,
    /// Move the focused window to the monitor on the left.
    MoveWindowToMonitorLeft,
    /// Move the focused window to the monitor on the right.
    MoveWindowToMonitorRight,

    /// Resize the focused column.
    Resize {
        /// Width delta in pixels (positive to grow, negative to shrink).
        delta: i32,
    },

    /// Scroll the viewport.
    Scroll {
        /// Scroll delta (positive = right, negative = left).
        delta: f64,
    },

    /// Query the current workspace state.
    QueryWorkspace,
    /// Query the focused window.
    QueryFocused,

    /// Re-enumerate windows and add new ones.
    Refresh,
    /// Apply the current layout to windows.
    Apply,
    /// Reload configuration from file.
    Reload,
    /// Stop the daemon.
    Stop,
    /// Emergency recovery command to revert managed windows to a safe state.
    PanicRevert,
    /// Toggle paused state for tiling operations.
    TogglePause,

    /// Query detailed information about all managed windows.
    QueryAllWindows,

    /// Close the focused window.
    CloseWindow,
    /// Toggle floating state for the focused window.
    ToggleFloating,
    /// Toggle fullscreen for the focused window.
    ToggleFullscreen,
    /// Set the focused column width as a fraction of the viewport.
    SetColumnWidth {
        /// Fraction of viewport width (e.g., 0.333, 0.5, 0.667).
        fraction: f64,
    },
    /// Equalize all column widths.
    EqualizeColumnWidths,
    /// Query daemon status information.
    QueryStatus,
    /// Health check — returns uptime, window count, and error count.
    HealthCheck,
}

/// Responses from the daemon to the CLI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum IpcResponse {
    /// Command executed successfully.
    Ok,
    /// Command failed with an error.
    Error {
        /// Error message describing what went wrong.
        message: String,
    },
    /// Workspace state query response.
    WorkspaceState {
        /// Number of columns in the workspace.
        columns: usize,
        /// Total number of windows.
        windows: usize,
        /// Index of the currently focused column.
        focused_column: usize,
        /// Index of the focused window within its column.
        focused_window: usize,
        /// Current scroll offset.
        scroll_offset: f64,
        /// Total width of all columns.
        total_width: i32,
    },
    /// Focused window query response.
    FocusedWindow {
        /// Window ID of the focused window, if any.
        window_id: Option<u64>,
        /// Column index of the focused window.
        column_index: usize,
        /// Window index within the column.
        window_index: usize,
    },

    /// Response containing information about all windows.
    WindowList {
        /// List of all managed windows.
        windows: Vec<WindowInfo>,
    },

    /// Response containing detailed info about the focused window.
    FocusedWindowInfo {
        /// The focused window's info, if any.
        window: Option<WindowInfo>,
    },

    /// Daemon status information.
    StatusInfo {
        /// Daemon version.
        version: String,
        /// Number of monitors.
        monitors: usize,
        /// Total managed windows across all workspaces.
        total_windows: usize,
        /// Daemon uptime in seconds.
        uptime_seconds: u64,
    },
    /// Health check response.
    HealthInfo {
        /// Whether the daemon considers itself healthy.
        healthy: bool,
        /// Daemon uptime in seconds.
        uptime_seconds: u64,
        /// Total managed windows.
        total_windows: usize,
        /// Number of monitors detected.
        monitors: usize,
        /// Whether tiling is paused.
        paused: bool,
    },
    /// Forward-compatibility fallback for newer daemon responses unknown to this client.
    #[serde(other)]
    Unknown,
}

impl IpcResponse {
    /// Create an error response.
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_info(window_id: u64, column_index: Option<usize>) -> WindowInfo {
        WindowInfo {
            window_id,
            title: "Test Window".to_string(),
            class_name: "TestClass".to_string(),
            process_id: 100,
            executable: "test.exe".to_string(),
            rect: IpcRect::new(-10, 0, 800, 600),
            column_index,
            window_index: column_index,
            monitor_id: -1,
            is_floating: column_index.is_none(),
            is_focused: true,
        }
    }

    #[test]
    fn test_all_command_types_roundtrip() {
        let commands = vec![
            IpcCommand::FocusLeft,
            IpcCommand::FocusRight,
            IpcCommand::FocusUp,
            IpcCommand::FocusDown,
            IpcCommand::MoveColumnLeft,
            IpcCommand::MoveColumnRight,
            IpcCommand::FocusMonitorLeft,
            IpcCommand::FocusMonitorRight,
            IpcCommand::MoveWindowToMonitorLeft,
            IpcCommand::MoveWindowToMonitorRight,
            IpcCommand::Resize { delta: 100 },
            IpcCommand::Resize { delta: -50 },
            IpcCommand::Scroll { delta: 150.5 },
            IpcCommand::Scroll { delta: -75.0 },
            IpcCommand::QueryWorkspace,
            IpcCommand::QueryFocused,
            IpcCommand::QueryAllWindows,
            IpcCommand::Refresh,
            IpcCommand::Apply,
            IpcCommand::Reload,
            IpcCommand::Stop,
            IpcCommand::PanicRevert,
            IpcCommand::TogglePause,
            IpcCommand::CloseWindow,
            IpcCommand::ToggleFloating,
            IpcCommand::ToggleFullscreen,
            IpcCommand::SetColumnWidth { fraction: 0.5 },
            IpcCommand::SetColumnWidth { fraction: 0.333 },
            IpcCommand::EqualizeColumnWidths,
            IpcCommand::QueryStatus,
            IpcCommand::HealthCheck,
        ];

        for cmd in commands {
            let json = serde_json::to_string(&cmd).expect("Failed to serialize command");
            let roundtrip: IpcCommand =
                serde_json::from_str(&json).expect("Failed to deserialize command");
            assert_eq!(cmd, roundtrip, "Roundtrip failed for {:?}", cmd);
        }

        // Wire names are a contract with already-installed CLIs and scripts.
        let wire = [
            (IpcCommand::FocusLeft, r#"{"type":"focus_left"}"#),
            (IpcCommand::PanicRevert, r#"{"type":"panic_revert"}"#),
            (
                IpcCommand::Resize { delta: -50 },
                r#"{"type":"resize","delta":-50}"#,
            ),
        ];
        for (cmd, expected) in wire {
            assert_eq!(serde_json::to_string(&cmd).unwrap(), expected);
        }
    }

    #[test]
    fn test_all_response_types_roundtrip() {
        let responses = vec![
            IpcResponse::Ok,
            IpcResponse::error("Test error"),
            IpcResponse::WorkspaceState {
                columns: 5,
                windows: 10,
                focused_column: 2,
                focused_window: 1,
                scroll_offset: 200.5,
                total_width: 4000,
            },
            IpcResponse::FocusedWindow {
                window_id: Some(12345),
                column_index: 1,
                window_index: 0,
            },
            IpcResponse::FocusedWindow {
                window_id: None,
                column_index: 0,
                window_index: 0,
            },
            IpcResponse::WindowList {
                windows: vec![window_info(1, Some(0)), window_info(2, None)],
            },
            IpcResponse::WindowList { windows: vec![] },
            IpcResponse::FocusedWindowInfo {
                window: Some(window_info(42, Some(1))),
            },
            IpcResponse::FocusedWindowInfo { window: None },
            IpcResponse::StatusInfo {
                version: "0.1.0".to_string(),
                monitors: 2,
                total_windows: 5,
                uptime_seconds: 3600,
            },
            IpcResponse::HealthInfo {
                healthy: true,
                uptime_seconds: 120,
                total_windows: 3,
                monitors: 1,
                paused: false,
            },
        ];

        for resp in responses {
            let json = serde_json::to_string(&resp).expect("Failed to serialize response");
            let roundtrip: IpcResponse =
                serde_json::from_str(&json).expect("Failed to deserialize response");
            assert_eq!(resp, roundtrip, "Roundtrip failed for {:?}", resp);
        }
    }

    #[test]
    fn test_invalid_and_unknown_messages() {
        assert!(serde_json::from_str::<IpcCommand>("not valid json").is_err());
        assert!(serde_json::from_str::<IpcCommand>(r#"{"type": "unknown_command"}"#).is_err());

        // An older client must tolerate responses added by a newer daemon.
        let result: Result<IpcResponse, _> = serde_json::from_str(r#"{"status": "invalid"}"#);
        assert!(matches!(result, Ok(IpcResponse::Unknown)));
    }

    #[test]
    fn test_scoped_pipe_name_for_user_sanitizes_and_scopes() {
        assert_eq!(
            scoped_pipe_name_for_user("ACME\\Alice Example!"),
            format!("{PIPE_NAME}_acme_alice_example")
        );
        assert_eq!(
            scoped_pipe_name_for_user("dev-box.local\\bob_1"),
            format!("{PIPE_NAME}_dev-box.local_bob_1")
        );

        // Nothing usable left after sanitising: no scope segment.
        assert_eq!(scoped_pipe_name_for_user(""), PIPE_NAME);
        assert_eq!(scoped_pipe_name_for_user("!!!\\ "), PIPE_NAME);

        let long = scoped_pipe_name_for_user(&"x".repeat(500));
        assert_eq!(long.len(), PIPE_NAME.len() + 1 + MAX_PIPE_SCOPE_SEGMENT_LEN);
    }

    #[test]
    fn test_pipe_name_candidates_have_no_global_fallback() {
        let candidates = pipe_name_candidates();
        assert_eq!(candidates, vec![preferred_pipe_name()]);
        if preferred_pipe_name() != PIPE_NAME {
            assert!(!candidates.contains(&PIPE_NAME.to_string()));
        }
    }

    #[test]
    fn test_protocol_version_helpers() {
        assert_eq!(protocol_id(), IPC_PROTOCOL_VERSION);
        assert!(is_protocol_version_supported(IPC_PROTOCOL_VERSION));
        assert!(is_protocol_version_supported(
            IPC_MIN_SUPPORTED_PROTOCOL_VERSION
        ));
        assert!(!is_protocol_version_supported(0));
        assert!(!is_protocol_version_supported(IPC_PROTOCOL_VERSION + 1));
    }
}
