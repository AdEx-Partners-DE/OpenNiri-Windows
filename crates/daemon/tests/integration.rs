//! Integration tests for the OpenNiri daemon IPC wire protocol.
//!
//! These tests pin the line-delimited JSON contract between the CLI, scripts
//! and the daemon. Per-variant serde details live in the `openniri-ipc` crate.
//!
//! Limits:
//! - These tests validate IPC serde and line-delimited wire shape only.
//! - They do not launch the daemon event loop or exercise real Win32 focus/desktop recovery.
//! - Lockout/recovery behavior must be verified in host/manual scenarios.

use openniri_ipc::{IpcCommand, IpcRect, IpcResponse, WindowInfo};

fn window(window_id: u64, title: &str, is_floating: bool) -> WindowInfo {
    WindowInfo {
        window_id,
        title: title.to_string(),
        class_name: "TestClass".to_string(),
        process_id: 1234,
        executable: "test.exe".to_string(),
        rect: IpcRect::new(0, 0, 800, 600),
        column_index: (!is_floating).then_some(0),
        window_index: (!is_floating).then_some(0),
        monitor_id: 1,
        is_floating,
        is_focused: !is_floating,
    }
}

/// Every command the daemon accepts survives a serialize/deserialize roundtrip.
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
        IpcCommand::Resize { delta: i32::MIN },
        IpcCommand::Resize { delta: i32::MAX },
        IpcCommand::Scroll { delta: -75.5 },
        IpcCommand::QueryWorkspace,
        IpcCommand::QueryFocused,
        IpcCommand::QueryAllWindows,
        IpcCommand::CloseWindow,
        IpcCommand::ToggleFloating,
        IpcCommand::ToggleFullscreen,
        IpcCommand::SetColumnWidth { fraction: 0.333 },
        IpcCommand::EqualizeColumnWidths,
        IpcCommand::QueryStatus,
        IpcCommand::HealthCheck,
        IpcCommand::Refresh,
        IpcCommand::Apply,
        IpcCommand::Reload,
        IpcCommand::Stop,
        IpcCommand::PanicRevert,
        IpcCommand::TogglePause,
    ];

    for cmd in commands {
        let json = serde_json::to_string(&cmd).expect("serialize");
        let parsed: IpcCommand = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, cmd, "Command roundtrip failed: {}", json);
    }
}

/// Every response the daemon sends survives a roundtrip, including payloads
/// with Unicode and JSON-special characters.
#[test]
fn test_all_responses_roundtrip() {
    let responses = vec![
        IpcResponse::Ok,
        IpcResponse::Error {
            message: "Failed to process: \"window\" with <special> & chars".to_string(),
        },
        IpcResponse::WorkspaceState {
            columns: 3,
            windows: 5,
            focused_column: 1,
            focused_window: 0,
            scroll_offset: -123.5,
            total_width: 2400,
        },
        IpcResponse::FocusedWindow {
            window_id: None,
            column_index: 0,
            window_index: 0,
        },
        IpcResponse::WindowList {
            windows: vec![
                window(100, "日本語タイトル 中文标题 🎉", false),
                window(300, "Floating Window", true),
            ],
        },
        IpcResponse::FocusedWindowInfo {
            window: Some(window(101, "Focused Window", false)),
        },
        IpcResponse::FocusedWindowInfo { window: None },
        IpcResponse::StatusInfo {
            version: "0.1.0-test".to_string(),
            monitors: 2,
            total_windows: 7,
            uptime_seconds: 3600,
        },
        IpcResponse::HealthInfo {
            healthy: true,
            uptime_seconds: 3600,
            total_windows: 7,
            monitors: 2,
            paused: false,
        },
    ];

    for resp in responses {
        let json = serde_json::to_string(&resp).expect("serialize");
        let parsed: IpcResponse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, resp, "Response roundtrip failed: {}", json);
    }
}

/// Messages are newline-delimited, so a serialized message must never contain
/// a raw newline, even when a payload string does.
#[test]
fn test_messages_stay_single_line() {
    let cmd_json = serde_json::to_string(&IpcCommand::FocusLeft).expect("serialize");
    let error_json = serde_json::to_string(&IpcResponse::Error {
        message: "line one\nline two".to_string(),
    })
    .expect("serialize");
    let list_json = serde_json::to_string(&IpcResponse::WindowList {
        windows: vec![window(100, "title\r\nwith break", false)],
    })
    .expect("serialize");

    for json in [&cmd_json, &error_json, &list_json] {
        assert!(
            !json.contains('\n'),
            "raw newline in wire message: {}",
            json
        );
        let line = format!("{}\n", json);
        let _: serde_json::Value =
            serde_json::from_str(line.trim_end()).expect("parse framed line");
    }
}

/// The exact JSON shapes that scripts and the emergency CLI paths depend on.
#[test]
fn test_wire_shapes_are_stable() {
    for (cmd, expected) in [
        (IpcCommand::Stop, serde_json::json!({ "type": "stop" })),
        (
            IpcCommand::PanicRevert,
            serde_json::json!({ "type": "panic_revert" }),
        ),
        (
            IpcCommand::TogglePause,
            serde_json::json!({ "type": "toggle_pause" }),
        ),
    ] {
        assert_eq!(serde_json::to_value(&cmd).expect("serialize"), expected);
        let parsed: IpcCommand = serde_json::from_value(expected).expect("deserialize");
        assert_eq!(parsed, cmd);
    }

    assert_eq!(
        serde_json::to_value(IpcResponse::Ok).expect("serialize"),
        serde_json::json!({ "status": "ok" })
    );
    let parsed: IpcResponse = serde_json::from_str(r#"{"status":"ok"}"#).expect("parse ok");
    assert_eq!(parsed, IpcResponse::Ok);
}

/// Malformed or unknown command input is rejected instead of being misread.
#[test]
fn test_malformed_commands_are_rejected() {
    let invalid_inputs = [
        "",
        "not json",
        "{",
        "{invalid}",
        "null",
        "123",
        "true",
        "\x00\x01\x02\x7f",
        r#"{"FocusLeft":"#,
        r#"{"UnknownCommand":{}}"#,
        r#"{"type":"no_such_command"}"#,
    ];

    for input in invalid_inputs {
        let result: Result<IpcCommand, _> = serde_json::from_str(input);
        assert!(result.is_err(), "Should fail to parse: {:?}", input);
    }
}

/// A newer daemon may send response types an older CLI does not know.
#[test]
fn test_unknown_response_type_is_tolerated() {
    let json = r#"{"status":"future_response","payload":{"v":1}}"#;
    let result: Result<IpcResponse, _> = serde_json::from_str(json);
    assert!(matches!(result, Ok(IpcResponse::Unknown)));
}
