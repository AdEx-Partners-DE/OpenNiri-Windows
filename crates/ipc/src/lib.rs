//! OpenNiri IPC Protocol
//!
//! Shared types for daemon-CLI communication over Windows named pipes.

use serde::{Deserialize, Serialize};

/// Named pipe path for IPC communication.
pub const PIPE_NAME: &str = r"\\.\pipe\openniri";

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

    #[test]
    fn test_command_serialization() {
        let cmd = IpcCommand::FocusLeft;
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("focus_left"));

        let cmd2: IpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, cmd2);
    }

    #[test]
    fn test_resize_command_serialization() {
        let cmd = IpcCommand::Resize { delta: -50 };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("resize"));
        assert!(json.contains("-50"));

        let cmd2: IpcCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, cmd2);
    }

    #[test]
    fn test_response_serialization() {
        let resp = IpcResponse::Ok;
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("ok"));

        let resp2: IpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, resp2);
    }

    #[test]
    fn test_workspace_state_serialization() {
        let resp = IpcResponse::WorkspaceState {
            columns: 3,
            windows: 5,
            focused_column: 1,
            focused_window: 0,
            scroll_offset: 100.5,
            total_width: 2400,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("workspace_state"));
        assert!(json.contains("\"columns\":3"));

        let resp2: IpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, resp2);
    }

    #[test]
    fn test_error_response() {
        let resp = IpcResponse::error("Something went wrong");
        if let IpcResponse::Error { message } = resp {
            assert_eq!(message, "Something went wrong");
        } else {
            panic!("Expected Error response");
        }
    }

    #[test]
    fn test_all_command_types_roundtrip() {
        // Verify all command variants serialize and deserialize correctly
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
            IpcCommand::Refresh,
            IpcCommand::Apply,
            IpcCommand::Reload,
            IpcCommand::Stop,
        ];

        for cmd in commands {
            let json = serde_json::to_string(&cmd).expect("Failed to serialize command");
            let roundtrip: IpcCommand =
                serde_json::from_str(&json).expect("Failed to deserialize command");
            assert_eq!(cmd, roundtrip, "Roundtrip failed for {:?}", cmd);
        }
    }

    #[test]
    fn test_all_response_types_roundtrip() {
        // Verify all response variants serialize and deserialize correctly
        let responses = vec![
            IpcResponse::Ok,
            IpcResponse::Error {
                message: "Test error".to_string(),
            },
            IpcResponse::WorkspaceState {
                columns: 5,
                windows: 10,
                focused_column: 2,
                focused_window: 1,
                scroll_offset: 200.0,
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
        ];

        for resp in responses {
            let json = serde_json::to_string(&resp).expect("Failed to serialize response");
            let roundtrip: IpcResponse =
                serde_json::from_str(&json).expect("Failed to deserialize response");
            assert_eq!(resp, roundtrip, "Roundtrip failed for {:?}", resp);
        }
    }

    #[test]
    fn test_line_delimited_protocol() {
        // Simulate the actual IPC protocol: JSON + newline
        let cmd = IpcCommand::QueryWorkspace;
        let wire_format = serde_json::to_string(&cmd).unwrap() + "\n";

        // Parse as if reading from pipe (trim newline)
        let parsed: IpcCommand = serde_json::from_str(wire_format.trim()).unwrap();
        assert_eq!(cmd, parsed);

        // Same for response
        let resp = IpcResponse::WorkspaceState {
            columns: 2,
            windows: 3,
            focused_column: 0,
            focused_window: 0,
            scroll_offset: 0.0,
            total_width: 1600,
        };
        let wire_format = serde_json::to_string(&resp).unwrap() + "\n";
        let parsed: IpcResponse = serde_json::from_str(wire_format.trim()).unwrap();
        assert_eq!(resp, parsed);
    }

    #[test]
    fn test_invalid_json_handling() {
        // Verify that invalid JSON produces clear errors
        let result: Result<IpcCommand, _> = serde_json::from_str("not valid json");
        assert!(result.is_err());

        let result: Result<IpcCommand, _> = serde_json::from_str("{\"type\": \"unknown_command\"}");
        assert!(result.is_err());

        let result: Result<IpcResponse, _> = serde_json::from_str("{\"status\": \"invalid\"}");
        assert!(result.is_err());
    }

    #[test]
    fn test_pipe_name_format() {
        // Verify pipe name follows Windows named pipe convention
        assert!(PIPE_NAME.starts_with(r"\\.\pipe\"));
        assert_eq!(PIPE_NAME, r"\\.\pipe\openniri");
    }
}
