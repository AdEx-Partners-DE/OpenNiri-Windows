//! Integration tests for OpenNiri daemon IPC protocol.
//!
//! These tests verify the IPC protocol correctness without requiring
//! actual Win32 window management. They test:
//! - Command serialization/deserialization
//! - Response formatting
//! - Protocol flow

use openniri_ipc::{IpcCommand, IpcResponse, IpcRect, WindowInfo};

// ============================================================================
// IPC Command Roundtrip Tests
// ============================================================================

/// Test that all IPC commands can be serialized and deserialized correctly.
#[test]
fn test_all_commands_roundtrip() {
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
        IpcCommand::Resize { delta: 50 },
        IpcCommand::Resize { delta: -30 },
        IpcCommand::Scroll { delta: 100.0 },
        IpcCommand::Scroll { delta: -75.5 },
        IpcCommand::QueryWorkspace,
        IpcCommand::QueryFocused,
        IpcCommand::QueryAllWindows,
        IpcCommand::Refresh,
        IpcCommand::Apply,
        IpcCommand::Reload,
        IpcCommand::Stop,
    ];

    for cmd in commands {
        let json = serde_json::to_string(&cmd).expect("serialize");
        let parsed: IpcCommand = serde_json::from_str(&json).expect("deserialize");

        // Verify roundtrip by serializing again
        let json2 = serde_json::to_string(&parsed).expect("re-serialize");
        assert_eq!(json, json2, "Command roundtrip failed: {:?}", cmd);
    }
}

/// Test that all IPC responses can be serialized and deserialized correctly.
#[test]
fn test_all_responses_roundtrip() {
    let responses = vec![
        IpcResponse::Ok,
        IpcResponse::Error { message: "Test error".to_string() },
        IpcResponse::WorkspaceState {
            columns: 3,
            windows: 5,
            focused_column: 1,
            focused_window: 0,
            scroll_offset: 123.5,
            total_width: 2400,
        },
        IpcResponse::FocusedWindow {
            window_id: Some(12345),
            column_index: 2,
            window_index: 1,
        },
        IpcResponse::FocusedWindow {
            window_id: None,
            column_index: 0,
            window_index: 0,
        },
        IpcResponse::WindowList {
            windows: vec![
                WindowInfo {
                    window_id: 100,
                    title: "Test Window".to_string(),
                    class_name: "TestClass".to_string(),
                    process_id: 1234,
                    executable: "test.exe".to_string(),
                    rect: IpcRect::new(0, 0, 800, 600),
                    column_index: Some(0),
                    window_index: Some(0),
                    monitor_id: 1,
                    is_floating: false,
                    is_focused: true,
                },
            ],
        },
    ];

    for resp in responses {
        let json = serde_json::to_string(&resp).expect("serialize");
        let parsed: IpcResponse = serde_json::from_str(&json).expect("deserialize");

        // Verify roundtrip by serializing again
        let json2 = serde_json::to_string(&parsed).expect("re-serialize");
        assert_eq!(json, json2, "Response roundtrip failed");
    }
}

// ============================================================================
// Protocol Format Tests
// ============================================================================

/// Test that commands are newline-delimited in the protocol.
#[test]
fn test_protocol_newline_delimited() {
    let cmd = IpcCommand::FocusLeft;
    let json = serde_json::to_string(&cmd).expect("serialize");

    // Protocol expects newline-terminated messages
    let protocol_msg = format!("{}\n", json);
    assert!(protocol_msg.ends_with('\n'));
    assert!(!json.contains('\n'));

    // Should be parseable without the newline
    let trimmed = protocol_msg.trim();
    let _parsed: IpcCommand = serde_json::from_str(trimmed).expect("parse trimmed");
}

/// Test that responses are newline-delimited in the protocol.
#[test]
fn test_response_newline_delimited() {
    let resp = IpcResponse::Ok;
    let json = serde_json::to_string(&resp).expect("serialize");

    // Protocol expects newline-terminated messages
    let protocol_msg = format!("{}\n", json);
    assert!(protocol_msg.ends_with('\n'));

    // Should be parseable without the newline
    let trimmed = protocol_msg.trim();
    let _parsed: IpcResponse = serde_json::from_str(trimmed).expect("parse trimmed");
}

// ============================================================================
// Error Response Tests
// ============================================================================

/// Test error response contains meaningful message.
#[test]
fn test_error_response_message() {
    let error_msg = "Window not found: 12345";
    let resp = IpcResponse::Error { message: error_msg.to_string() };

    let json = serde_json::to_string(&resp).expect("serialize");
    assert!(json.contains(error_msg));

    let parsed: IpcResponse = serde_json::from_str(&json).expect("deserialize");
    match parsed {
        IpcResponse::Error { message } => assert_eq!(message, error_msg),
        _ => panic!("Expected Error response"),
    }
}

/// Test error response with special characters.
#[test]
fn test_error_response_special_chars() {
    let error_msg = "Failed to process: \"window\" with <special> & chars";
    let resp = IpcResponse::Error { message: error_msg.to_string() };

    let json = serde_json::to_string(&resp).expect("serialize");
    let parsed: IpcResponse = serde_json::from_str(&json).expect("deserialize");

    match parsed {
        IpcResponse::Error { message } => assert_eq!(message, error_msg),
        _ => panic!("Expected Error response"),
    }
}

// ============================================================================
// WorkspaceState Response Tests
// ============================================================================

/// Test workspace state with edge case values.
#[test]
fn test_workspace_state_edge_values() {
    // Test with zero values
    let resp = IpcResponse::WorkspaceState {
        columns: 0,
        windows: 0,
        focused_column: 0,
        focused_window: 0,
        scroll_offset: 0.0,
        total_width: 0,
    };

    let json = serde_json::to_string(&resp).expect("serialize");
    let parsed: IpcResponse = serde_json::from_str(&json).expect("deserialize");

    match parsed {
        IpcResponse::WorkspaceState { columns, windows, .. } => {
            assert_eq!(columns, 0);
            assert_eq!(windows, 0);
        }
        _ => panic!("Expected WorkspaceState"),
    }
}

/// Test workspace state with large values.
#[test]
fn test_workspace_state_large_values() {
    let resp = IpcResponse::WorkspaceState {
        columns: 100,
        windows: 500,
        focused_column: 50,
        focused_window: 10,
        scroll_offset: 50000.5,
        total_width: 100000,
    };

    let json = serde_json::to_string(&resp).expect("serialize");
    let parsed: IpcResponse = serde_json::from_str(&json).expect("deserialize");

    match parsed {
        IpcResponse::WorkspaceState { total_width, scroll_offset, .. } => {
            assert_eq!(total_width, 100000);
            assert!((scroll_offset - 50000.5).abs() < 0.001);
        }
        _ => panic!("Expected WorkspaceState"),
    }
}

/// Test workspace state with negative scroll offset.
#[test]
fn test_workspace_state_negative_scroll() {
    let resp = IpcResponse::WorkspaceState {
        columns: 3,
        windows: 3,
        focused_column: 0,
        focused_window: 0,
        scroll_offset: -100.0,
        total_width: 2400,
    };

    let json = serde_json::to_string(&resp).expect("serialize");
    let parsed: IpcResponse = serde_json::from_str(&json).expect("deserialize");

    match parsed {
        IpcResponse::WorkspaceState { scroll_offset, .. } => {
            assert!((scroll_offset - (-100.0)).abs() < 0.001);
        }
        _ => panic!("Expected WorkspaceState"),
    }
}

// ============================================================================
// WindowList Response Tests
// ============================================================================

/// Test window list with empty list.
#[test]
fn test_window_list_empty() {
    let resp = IpcResponse::WindowList { windows: vec![] };

    let json = serde_json::to_string(&resp).expect("serialize");
    let parsed: IpcResponse = serde_json::from_str(&json).expect("deserialize");

    match parsed {
        IpcResponse::WindowList { windows } => assert!(windows.is_empty()),
        _ => panic!("Expected WindowList"),
    }
}

/// Test window list with multiple windows.
#[test]
fn test_window_list_multiple_windows() {
    let windows = vec![
        WindowInfo {
            window_id: 100,
            title: "Window 1".to_string(),
            class_name: "Class1".to_string(),
            process_id: 1000,
            executable: "app1.exe".to_string(),
            rect: IpcRect::new(0, 0, 800, 600),
            column_index: Some(0),
            window_index: Some(0),
            monitor_id: 1,
            is_floating: false,
            is_focused: true,
        },
        WindowInfo {
            window_id: 200,
            title: "Window 2".to_string(),
            class_name: "Class2".to_string(),
            process_id: 2000,
            executable: "app2.exe".to_string(),
            rect: IpcRect::new(810, 0, 800, 600),
            column_index: Some(1),
            window_index: Some(0),
            monitor_id: 1,
            is_floating: false,
            is_focused: false,
        },
        WindowInfo {
            window_id: 300,
            title: "Floating Window".to_string(),
            class_name: "FloatClass".to_string(),
            process_id: 3000,
            executable: "float.exe".to_string(),
            rect: IpcRect::new(100, 100, 400, 300),
            column_index: None,
            window_index: None,
            monitor_id: 1,
            is_floating: true,
            is_focused: false,
        },
    ];

    let resp = IpcResponse::WindowList { windows };

    let json = serde_json::to_string(&resp).expect("serialize");
    let parsed: IpcResponse = serde_json::from_str(&json).expect("deserialize");

    match parsed {
        IpcResponse::WindowList { windows } => {
            assert_eq!(windows.len(), 3);
            assert!(windows[0].is_focused);
            assert!(!windows[1].is_focused);
            assert!(windows[2].is_floating);
        }
        _ => panic!("Expected WindowList"),
    }
}

/// Test window info with Unicode title.
#[test]
fn test_window_info_unicode_title() {
    let win = WindowInfo {
        window_id: 100,
        title: "日本語タイトル 中文标题 🎉".to_string(),
        class_name: "TestClass".to_string(),
        process_id: 1234,
        executable: "test.exe".to_string(),
        rect: IpcRect::new(0, 0, 800, 600),
        column_index: Some(0),
        window_index: Some(0),
        monitor_id: 1,
        is_floating: false,
        is_focused: false,
    };

    let json = serde_json::to_string(&win).expect("serialize");
    let parsed: WindowInfo = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(parsed.title, "日本語タイトル 中文标题 🎉");
}

// ============================================================================
// Command-Specific Tests
// ============================================================================

/// Test resize command with various deltas.
#[test]
fn test_resize_command_values() {
    let deltas = vec![0, 1, -1, 50, -50, 100, -100, i32::MAX, i32::MIN];

    for delta in deltas {
        let cmd = IpcCommand::Resize { delta };
        let json = serde_json::to_string(&cmd).expect("serialize");
        let parsed: IpcCommand = serde_json::from_str(&json).expect("deserialize");

        match parsed {
            IpcCommand::Resize { delta: d } => assert_eq!(d, delta),
            _ => panic!("Expected Resize command"),
        }
    }
}

/// Test scroll command with various deltas.
#[test]
fn test_scroll_command_values() {
    let deltas = vec![0.0, 1.0, -1.0, 100.5, -100.5, f64::MAX, f64::MIN];

    for delta in deltas {
        let cmd = IpcCommand::Scroll { delta };
        let json = serde_json::to_string(&cmd).expect("serialize");
        let parsed: IpcCommand = serde_json::from_str(&json).expect("deserialize");

        match parsed {
            IpcCommand::Scroll { delta: d } => {
                if delta.is_finite() {
                    assert!((d - delta).abs() < 0.001);
                }
            }
            _ => panic!("Expected Scroll command"),
        }
    }
}

// ============================================================================
// Invalid Input Tests
// ============================================================================

/// Test parsing invalid JSON.
#[test]
fn test_invalid_json_parsing() {
    let invalid_inputs = vec![
        "",
        "not json",
        "{",
        "{invalid}",
        "null",
        "123",
        "true",
    ];

    for input in invalid_inputs {
        let result: Result<IpcCommand, _> = serde_json::from_str(input);
        assert!(result.is_err(), "Should fail to parse: {}", input);
    }
}

/// Test parsing unknown command type.
#[test]
fn test_unknown_command_type() {
    let json = r#"{"UnknownCommand":{}}"#;
    let result: Result<IpcCommand, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

/// Test parsing unknown response type.
#[test]
fn test_unknown_response_type() {
    let json = r#"{"UnknownResponse":{}}"#;
    let result: Result<IpcResponse, _> = serde_json::from_str(json);
    assert!(result.is_err());
}
