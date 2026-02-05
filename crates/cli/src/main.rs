//! OpenNiri CLI
//!
//! Command-line interface for controlling the OpenNiri window manager.
//!
//! Commands are sent to the daemon via IPC (named pipe).

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use openniri_ipc::{IpcCommand, IpcResponse, PIPE_NAME};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ClientOptions;
use tokio::time::timeout;

/// Connection timeout for IPC commands.
const IPC_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Parser)]
#[command(name = "openniri-cli")]
#[command(author, version, about = "Control the OpenNiri window manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Focus commands
    Focus {
        #[command(subcommand)]
        direction: FocusDirection,
    },
    /// Scroll the viewport
    Scroll {
        #[command(subcommand)]
        direction: ScrollDirection,
    },
    /// Move the focused column
    Move {
        #[command(subcommand)]
        direction: MoveDirection,
    },
    /// Resize the focused column
    Resize {
        /// Width delta in pixels (positive to grow, negative to shrink)
        #[arg(short, long)]
        delta: i32,
    },
    /// Focus a different monitor
    FocusMonitor {
        #[command(subcommand)]
        direction: MonitorDirection,
    },
    /// Move the focused window to a different monitor
    MoveToMonitor {
        #[command(subcommand)]
        direction: MonitorDirection,
    },
    /// Query workspace state
    Query {
        #[command(subcommand)]
        what: QueryType,
    },
    /// Re-enumerate windows
    Refresh,
    /// Apply current layout to windows
    Apply,
    /// Reload configuration from file
    Reload,
    /// Generate default configuration file
    Init {
        /// Output path (default: %APPDATA%/openniri/config/config.toml)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Overwrite existing config file
        #[arg(short, long)]
        force: bool,
    },
    /// Stop the daemon
    Stop,
}

#[derive(Subcommand)]
enum FocusDirection {
    /// Focus the column to the left
    Left,
    /// Focus the column to the right
    Right,
    /// Focus the window above (in stacked columns)
    Up,
    /// Focus the window below (in stacked columns)
    Down,
}

#[derive(Subcommand)]
enum ScrollDirection {
    /// Scroll viewport left
    Left {
        /// Pixels to scroll (default: 100)
        #[arg(short, long, default_value = "100")]
        pixels: i32,
    },
    /// Scroll viewport right
    Right {
        /// Pixels to scroll (default: 100)
        #[arg(short, long, default_value = "100")]
        pixels: i32,
    },
}

#[derive(Subcommand)]
enum MoveDirection {
    /// Move focused column left
    Left,
    /// Move focused column right
    Right,
}

#[derive(Subcommand)]
enum MonitorDirection {
    /// Focus/move to the monitor on the left
    Left,
    /// Focus/move to the monitor on the right
    Right,
}

#[derive(Subcommand)]
enum QueryType {
    /// Get current workspace state
    Workspace,
    /// Get focused window info
    Focused,
    /// List all managed windows
    All,
}

/// Convert CLI command to IPC command.
fn to_ipc_command(cmd: &Commands) -> IpcCommand {
    match cmd {
        Commands::Focus { direction } => match direction {
            FocusDirection::Left => IpcCommand::FocusLeft,
            FocusDirection::Right => IpcCommand::FocusRight,
            FocusDirection::Up => IpcCommand::FocusUp,
            FocusDirection::Down => IpcCommand::FocusDown,
        },
        Commands::Scroll { direction } => match direction {
            ScrollDirection::Left { pixels } => IpcCommand::Scroll {
                delta: -(*pixels as f64),
            },
            ScrollDirection::Right { pixels } => IpcCommand::Scroll {
                delta: *pixels as f64,
            },
        },
        Commands::Move { direction } => match direction {
            MoveDirection::Left => IpcCommand::MoveColumnLeft,
            MoveDirection::Right => IpcCommand::MoveColumnRight,
        },
        Commands::Resize { delta } => IpcCommand::Resize { delta: *delta },
        Commands::FocusMonitor { direction } => match direction {
            MonitorDirection::Left => IpcCommand::FocusMonitorLeft,
            MonitorDirection::Right => IpcCommand::FocusMonitorRight,
        },
        Commands::MoveToMonitor { direction } => match direction {
            MonitorDirection::Left => IpcCommand::MoveWindowToMonitorLeft,
            MonitorDirection::Right => IpcCommand::MoveWindowToMonitorRight,
        },
        Commands::Query { what } => match what {
            QueryType::Workspace => IpcCommand::QueryWorkspace,
            QueryType::Focused => IpcCommand::QueryFocused,
            QueryType::All => IpcCommand::QueryAllWindows,
        },
        Commands::Refresh => IpcCommand::Refresh,
        Commands::Apply => IpcCommand::Apply,
        Commands::Reload => IpcCommand::Reload,
        Commands::Init { .. } => unreachable!("Init is handled separately"),
        Commands::Stop => IpcCommand::Stop,
    }
}

/// Send a command to the daemon and return the response (with timeout).
async fn send_command(cmd: IpcCommand) -> Result<IpcResponse> {
    timeout(IPC_TIMEOUT, send_command_inner(cmd))
        .await
        .context("Timed out waiting for daemon response")?
}

/// Inner implementation without timeout.
async fn send_command_inner(cmd: IpcCommand) -> Result<IpcResponse> {
    // Connect to the named pipe
    let client = ClientOptions::new()
        .open(PIPE_NAME)
        .context("Failed to connect to daemon. Is openniri running?")?;

    let (reader, mut writer) = tokio::io::split(client);

    // Send command as JSON line
    let json = serde_json::to_string(&cmd)? + "\n";
    writer
        .write_all(json.as_bytes())
        .await
        .context("Failed to send command")?;

    // Read response
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .await
        .context("Failed to read response")?;

    let response: IpcResponse =
        serde_json::from_str(line.trim()).context("Failed to parse response")?;

    Ok(response)
}

/// Print a response in a human-readable format.
fn print_response(response: &IpcResponse) {
    match response {
        IpcResponse::Ok => {
            println!("OK");
        }
        IpcResponse::Error { message } => {
            eprintln!("Error: {}", message);
        }
        IpcResponse::WorkspaceState {
            columns,
            windows,
            focused_column,
            focused_window,
            scroll_offset,
            total_width,
        } => {
            println!("Workspace State:");
            println!("  Columns: {}", columns);
            println!("  Windows: {}", windows);
            println!("  Focused column: {}", focused_column);
            println!("  Focused window in column: {}", focused_window);
            println!("  Scroll offset: {:.1}", scroll_offset);
            println!("  Total width: {}", total_width);
        }
        IpcResponse::FocusedWindow {
            window_id,
            column_index,
            window_index,
        } => {
            println!("Focused Window:");
            match window_id {
                Some(id) => println!("  Window ID: {}", id),
                None => println!("  No window focused"),
            }
            println!("  Column index: {}", column_index);
            println!("  Window index: {}", window_index);
        }
        IpcResponse::WindowList { windows } => {
            println!("Managed Windows ({} total):", windows.len());
            for win in windows {
                let location = if win.is_floating {
                    "floating".to_string()
                } else {
                    format!("col {} win {}", win.column_index.unwrap_or(0), win.window_index.unwrap_or(0))
                };
                let focus_marker = if win.is_focused { " [FOCUSED]" } else { "" };
                println!("  {} - {} ({}) [{}]{}", win.window_id, win.title, win.executable, location, focus_marker);
            }
        }
        IpcResponse::FocusedWindowInfo { window } => {
            match window {
                Some(win) => {
                    println!("Focused Window Info:");
                    println!("  Window ID: {}", win.window_id);
                    println!("  Title: {}", win.title);
                    println!("  Class: {}", win.class_name);
                    println!("  Executable: {}", win.executable);
                    println!("  Position: ({}, {})", win.rect.x, win.rect.y);
                    println!("  Size: {}x{}", win.rect.width, win.rect.height);
                    println!("  Monitor: {}", win.monitor_id);
                    if win.is_floating {
                        println!("  Layout: floating");
                    } else {
                        println!("  Layout: tiled (col {}, win {})",
                            win.column_index.unwrap_or(0),
                            win.window_index.unwrap_or(0));
                    }
                }
                None => {
                    println!("No window is currently focused");
                }
            }
        }
    }
}

/// Generate default configuration content.
fn generate_default_config() -> String {
    r#"# OpenNiri Windows Configuration
# https://github.com/AdEx-Partners-DE/OpenNiri-Windows

[layout]
# Gap between columns in pixels
gap = 10

# Gap at the edges of the viewport in pixels
outer_gap = 10

# Default width for new columns in pixels
default_column_width = 800

# Minimum column width in pixels
min_column_width = 400

# Maximum column width in pixels
max_column_width = 1600

# Centering mode: "center" or "just_in_view"
# - center: Always center the focused column
# - just_in_view: Only scroll if focused column would be outside viewport
centering_mode = "center"

[appearance]
# Use DWM cloaking for off-screen windows (keeps them in Alt-Tab)
use_cloaking = true

# Use batched window positioning for smoother updates
use_deferred_positioning = true

[behavior]
# Automatically focus new windows when they appear
focus_new_windows = true

# Track focus changes from Windows (sync with Alt-Tab, etc.)
track_focus_changes = true

# Log level: trace, debug, info, warn, error
log_level = "info"
"#
    .to_string()
}

/// Get the default config file path.
fn default_config_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "openniri")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

/// Handle the init command (generate default config).
fn handle_init(output: Option<PathBuf>, force: bool) -> Result<()> {
    let path = output.or_else(default_config_path).context(
        "Could not determine config path. Use --output to specify a path.",
    )?;

    // Check if file exists
    if path.exists() && !force {
        anyhow::bail!(
            "Config file already exists at: {}\nUse --force to overwrite.",
            path.display()
        );
    }

    // Create parent directories
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Write config file
    let config_content = generate_default_config();
    fs::write(&path, config_content)
        .with_context(|| format!("Failed to write config file: {}", path.display()))?;

    println!("Created config file: {}", path.display());
    println!("\nEdit this file to customize OpenNiri settings.");
    println!("Run 'openniri-cli reload' to apply changes while daemon is running.");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle init command separately (doesn't need IPC)
    if let Commands::Init { output, force } = cli.command {
        return handle_init(output, force);
    }

    let ipc_cmd = to_ipc_command(&cli.command);
    let response = send_command(ipc_cmd).await?;
    print_response(&response);

    // Exit with error code if response was an error
    if matches!(response, IpcResponse::Error { .. }) {
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // to_ipc_command tests
    // =========================================================================

    #[test]
    fn test_to_ipc_command_focus_left() {
        let cmd = Commands::Focus { direction: FocusDirection::Left };
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::FocusLeft));
    }

    #[test]
    fn test_to_ipc_command_focus_right() {
        let cmd = Commands::Focus { direction: FocusDirection::Right };
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::FocusRight));
    }

    #[test]
    fn test_to_ipc_command_focus_up() {
        let cmd = Commands::Focus { direction: FocusDirection::Up };
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::FocusUp));
    }

    #[test]
    fn test_to_ipc_command_focus_down() {
        let cmd = Commands::Focus { direction: FocusDirection::Down };
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::FocusDown));
    }

    #[test]
    fn test_to_ipc_command_scroll_left() {
        let cmd = Commands::Scroll { direction: ScrollDirection::Left { pixels: 100 } };
        match to_ipc_command(&cmd) {
            IpcCommand::Scroll { delta } => assert_eq!(delta, -100.0),
            _ => panic!("Expected Scroll command"),
        }
    }

    #[test]
    fn test_to_ipc_command_scroll_right() {
        let cmd = Commands::Scroll { direction: ScrollDirection::Right { pixels: 150 } };
        match to_ipc_command(&cmd) {
            IpcCommand::Scroll { delta } => assert_eq!(delta, 150.0),
            _ => panic!("Expected Scroll command"),
        }
    }

    #[test]
    fn test_to_ipc_command_move_left() {
        let cmd = Commands::Move { direction: MoveDirection::Left };
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::MoveColumnLeft));
    }

    #[test]
    fn test_to_ipc_command_move_right() {
        let cmd = Commands::Move { direction: MoveDirection::Right };
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::MoveColumnRight));
    }

    #[test]
    fn test_to_ipc_command_resize() {
        let cmd = Commands::Resize { delta: 50 };
        match to_ipc_command(&cmd) {
            IpcCommand::Resize { delta } => assert_eq!(delta, 50),
            _ => panic!("Expected Resize command"),
        }
    }

    #[test]
    fn test_to_ipc_command_resize_negative() {
        let cmd = Commands::Resize { delta: -30 };
        match to_ipc_command(&cmd) {
            IpcCommand::Resize { delta } => assert_eq!(delta, -30),
            _ => panic!("Expected Resize command"),
        }
    }

    #[test]
    fn test_to_ipc_command_focus_monitor_left() {
        let cmd = Commands::FocusMonitor { direction: MonitorDirection::Left };
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::FocusMonitorLeft));
    }

    #[test]
    fn test_to_ipc_command_focus_monitor_right() {
        let cmd = Commands::FocusMonitor { direction: MonitorDirection::Right };
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::FocusMonitorRight));
    }

    #[test]
    fn test_to_ipc_command_move_to_monitor_left() {
        let cmd = Commands::MoveToMonitor { direction: MonitorDirection::Left };
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::MoveWindowToMonitorLeft));
    }

    #[test]
    fn test_to_ipc_command_move_to_monitor_right() {
        let cmd = Commands::MoveToMonitor { direction: MonitorDirection::Right };
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::MoveWindowToMonitorRight));
    }

    #[test]
    fn test_to_ipc_command_query_workspace() {
        let cmd = Commands::Query { what: QueryType::Workspace };
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::QueryWorkspace));
    }

    #[test]
    fn test_to_ipc_command_query_focused() {
        let cmd = Commands::Query { what: QueryType::Focused };
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::QueryFocused));
    }

    #[test]
    fn test_to_ipc_command_query_all() {
        let cmd = Commands::Query { what: QueryType::All };
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::QueryAllWindows));
    }

    #[test]
    fn test_to_ipc_command_refresh() {
        let cmd = Commands::Refresh;
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::Refresh));
    }

    #[test]
    fn test_to_ipc_command_apply() {
        let cmd = Commands::Apply;
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::Apply));
    }

    #[test]
    fn test_to_ipc_command_reload() {
        let cmd = Commands::Reload;
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::Reload));
    }

    #[test]
    fn test_to_ipc_command_stop() {
        let cmd = Commands::Stop;
        assert!(matches!(to_ipc_command(&cmd), IpcCommand::Stop));
    }

    // =========================================================================
    // generate_default_config tests
    // =========================================================================

    #[test]
    fn test_generate_default_config_contains_layout_section() {
        let config = generate_default_config();
        assert!(config.contains("[layout]"));
        assert!(config.contains("gap"));
        assert!(config.contains("outer_gap"));
        assert!(config.contains("default_column_width"));
    }

    #[test]
    fn test_generate_default_config_contains_appearance_section() {
        let config = generate_default_config();
        assert!(config.contains("[appearance]"));
        assert!(config.contains("use_cloaking"));
        assert!(config.contains("use_deferred_positioning"));
    }

    #[test]
    fn test_generate_default_config_contains_behavior_section() {
        let config = generate_default_config();
        assert!(config.contains("[behavior]"));
        assert!(config.contains("focus_new_windows"));
        assert!(config.contains("track_focus_changes"));
        assert!(config.contains("log_level"));
    }

    #[test]
    fn test_generate_default_config_contains_centering_mode() {
        let config = generate_default_config();
        assert!(config.contains("centering_mode"));
        assert!(config.contains("center") || config.contains("just_in_view"));
    }

    // =========================================================================
    // default_config_path tests
    // =========================================================================

    #[test]
    fn test_default_config_path_returns_some() {
        // This may return None in certain CI environments without home dirs
        // but on most systems it should return Some
        let path = default_config_path();
        // Just verify the function runs without panicking
        if let Some(p) = path {
            assert!(p.ends_with("config.toml"));
        }
    }

    #[test]
    fn test_default_config_path_contains_openniri() {
        if let Some(path) = default_config_path() {
            let path_str = path.to_string_lossy();
            assert!(
                path_str.contains("openniri"),
                "Path should contain 'openniri': {}",
                path_str
            );
        }
    }

    // =========================================================================
    // IPC timeout constant test
    // =========================================================================

    #[test]
    fn test_ipc_timeout_is_reasonable() {
        // Timeout should be between 1 and 30 seconds
        assert!(IPC_TIMEOUT >= Duration::from_secs(1));
        assert!(IPC_TIMEOUT <= Duration::from_secs(30));
    }
}
