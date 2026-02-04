//! OpenNiri Daemon
//!
//! Main daemon process for the OpenNiri window manager.
//!
//! Responsibilities:
//! - Maintain workspace state
//! - Process window events from the platform layer
//! - Handle IPC commands from the CLI
//! - Trigger layout recalculations
//! - Apply window placements

mod config;

use anyhow::Result;
use config::Config;
use openniri_core_layout::{Rect, Workspace};
use openniri_ipc::{IpcCommand, IpcResponse, PIPE_NAME};
use openniri_platform_win32::{
    enumerate_monitors, enumerate_windows, find_monitor_for_rect, install_event_hooks,
    monitor_to_left, monitor_to_right, parse_hotkey_string, register_hotkeys, Hotkey, HotkeyEvent,
    HotkeyId, MonitorId, MonitorInfo, PlatformConfig, WindowEvent,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{PipeMode, ServerOptions};
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

/// Events that the daemon event loop processes.
enum DaemonEvent {
    /// An IPC command from a CLI client.
    IpcCommand {
        cmd: IpcCommand,
        responder: oneshot::Sender<IpcResponse>,
    },
    /// A window lifecycle event from Win32.
    WindowEvent(WindowEvent),
    /// A global hotkey was pressed.
    Hotkey(HotkeyEvent),
    /// Animation tick (16ms intervals during animation).
    AnimationTick,
    /// Shutdown signal.
    Shutdown,
}

/// Animation tick interval in milliseconds (~60 FPS).
const ANIMATION_TICK_MS: u64 = 16;

/// Fallback viewport dimensions when no monitor is detected.
const FALLBACK_VIEWPORT_WIDTH: i32 = 1920;
const FALLBACK_VIEWPORT_HEIGHT: i32 = 1080;
const FALLBACK_WORK_AREA_HEIGHT: i32 = 1040;

/// Application state supporting multiple monitors.
struct AppState {
    /// Workspaces indexed by monitor ID.
    workspaces: HashMap<MonitorId, Workspace>,
    /// Monitor info indexed by monitor ID.
    monitors: HashMap<MonitorId, MonitorInfo>,
    /// Currently focused monitor.
    focused_monitor: MonitorId,
    /// Platform configuration.
    platform_config: PlatformConfig,
    /// User configuration.
    config: Config,
}

impl AppState {
    /// Create new state with config and monitors.
    fn new_with_config(config: Config, monitors: Vec<MonitorInfo>) -> Self {
        let mut workspaces = HashMap::new();
        let mut monitor_map = HashMap::new();
        let mut focused_monitor = 0;

        for monitor in monitors {
            let mut workspace = Workspace::with_gaps(config.layout.gap, config.layout.outer_gap);
            workspace.set_default_column_width(config.layout.default_column_width);
            workspace.set_centering_mode(config.layout.centering_mode.into());

            if monitor.is_primary {
                focused_monitor = monitor.id;
            }

            workspaces.insert(monitor.id, workspace);
            monitor_map.insert(monitor.id, monitor);
        }

        // If no primary found, use first monitor (defensive pattern avoids unwrap)
        if focused_monitor == 0 {
            if let Some(&first_id) = monitor_map.keys().next() {
                focused_monitor = first_id;
            }
            // If map is empty, focused_monitor stays 0; focused_workspace() returns None
        }

        // TODO: Implement MoveOffscreen as alternative hide strategy when use_cloaking is false
        let platform_config = PlatformConfig {
            hide_strategy: openniri_platform_win32::HideStrategy::Cloak,
            use_deferred_positioning: config.appearance.use_deferred_positioning,
        };

        Self {
            workspaces,
            monitors: monitor_map,
            focused_monitor,
            platform_config,
            config,
        }
    }

    /// Get the currently focused workspace.
    fn focused_workspace(&self) -> Option<&Workspace> {
        self.workspaces.get(&self.focused_monitor)
    }

    /// Get the currently focused workspace mutably.
    fn focused_workspace_mut(&mut self) -> Option<&mut Workspace> {
        self.workspaces.get_mut(&self.focused_monitor)
    }

    /// Get the focused monitor's viewport.
    fn focused_viewport(&self) -> Rect {
        self.monitors
            .get(&self.focused_monitor)
            .map(|m| m.work_area)
            .unwrap_or_else(|| Rect::new(0, 0, FALLBACK_VIEWPORT_WIDTH, FALLBACK_VIEWPORT_HEIGHT))
    }

    /// Apply configuration to all workspaces.
    fn apply_config(&mut self, config: Config) {
        for workspace in self.workspaces.values_mut() {
            workspace.set_gap(config.layout.gap);
            workspace.set_outer_gap(config.layout.outer_gap);
            workspace.set_default_column_width(config.layout.default_column_width);
            workspace.set_centering_mode(config.layout.centering_mode.into());
        }
        self.platform_config.use_deferred_positioning = config.appearance.use_deferred_positioning;
        self.config = config;
        info!("Configuration applied to all {} workspaces", self.workspaces.len());
    }

    /// Check if any workspace has an active animation.
    fn is_animating(&self) -> bool {
        self.workspaces.values().any(|w| w.is_animating())
    }

    /// Tick all active animations by the given delta time.
    /// Returns true if any animation is still running.
    fn tick_animations(&mut self, delta_ms: u64) -> bool {
        let mut still_animating = false;
        for workspace in self.workspaces.values_mut() {
            if workspace.tick_animation(delta_ms) {
                still_animating = true;
            }
        }
        still_animating
    }

    /// Recalculate layout and apply placements for all monitors.
    /// Uses animated offsets if any workspace has an active animation.
    fn apply_layout(&self) -> Result<()> {
        let mut all_placements = Vec::new();

        for (monitor_id, workspace) in &self.workspaces {
            if let Some(monitor) = self.monitors.get(monitor_id) {
                // Use animated placements to support smooth scrolling
                let placements = workspace.compute_placements_animated(monitor.work_area);
                debug!(
                    "Monitor {}: {} placements for viewport {}x{} (animating: {})",
                    monitor_id,
                    placements.len(),
                    monitor.work_area.width,
                    monitor.work_area.height,
                    workspace.is_animating()
                );
                all_placements.extend(placements);
            }
        }

        openniri_platform_win32::apply_placements(&all_placements, &self.platform_config)?;
        Ok(())
    }

    /// Enumerate windows and add them to the appropriate workspace based on position.
    fn enumerate_and_add_windows(&mut self) -> Result<usize> {
        let windows = enumerate_windows()?;
        let monitors: Vec<_> = self.monitors.values().cloned().collect();
        let mut added = 0;

        for win_info in windows {
            // Find which monitor this window is on
            let monitor_id = find_monitor_for_rect(&monitors, &win_info.rect)
                .map(|m| m.id)
                .unwrap_or(self.focused_monitor);

            // Use a reasonable default width or the window's current width, respecting config bounds
            let width = win_info.rect.width.clamp(
                self.config.layout.min_column_width,
                self.config.layout.max_column_width,
            );

            if let Some(workspace) = self.workspaces.get_mut(&monitor_id) {
                match workspace.insert_window(win_info.hwnd, Some(width)) {
                    Ok(()) => {
                        info!(
                            "Added window: {} ({}) to monitor {} - {}x{}",
                            win_info.title, win_info.class_name, monitor_id,
                            win_info.rect.width, win_info.rect.height
                        );
                        added += 1;
                    }
                    Err(e) => {
                        warn!("Failed to add window {}: {}", win_info.hwnd, e);
                    }
                }
            }
        }

        Ok(added)
    }

    /// Find which workspace contains a window.
    fn find_window_workspace(&self, window_id: u64) -> Option<MonitorId> {
        for (monitor_id, workspace) in &self.workspaces {
            if workspace.contains_window(window_id) {
                return Some(*monitor_id);
            }
        }
        None
    }

    /// Process an IPC command and return a response.
    fn handle_command(&mut self, cmd: IpcCommand) -> IpcResponse {
        let viewport_width = self.focused_viewport().width;

        match cmd {
            IpcCommand::FocusLeft => {
                if let Some(workspace) = self.focused_workspace_mut() {
                    workspace.focus_left();
                    workspace.ensure_focused_visible_animated(viewport_width);
                    info!("Focus left -> column {}", workspace.focused_column_index());
                }
                if let Err(e) = self.apply_layout() {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                IpcResponse::Ok
            }
            IpcCommand::FocusRight => {
                if let Some(workspace) = self.focused_workspace_mut() {
                    workspace.focus_right();
                    workspace.ensure_focused_visible_animated(viewport_width);
                    info!("Focus right -> column {}", workspace.focused_column_index());
                }
                if let Err(e) = self.apply_layout() {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                IpcResponse::Ok
            }
            IpcCommand::FocusUp => {
                if let Some(workspace) = self.focused_workspace_mut() {
                    workspace.focus_up();
                    info!("Focus up -> window {}", workspace.focused_window_index_in_column());
                }
                IpcResponse::Ok
            }
            IpcCommand::FocusDown => {
                if let Some(workspace) = self.focused_workspace_mut() {
                    workspace.focus_down();
                    info!("Focus down -> window {}", workspace.focused_window_index_in_column());
                }
                IpcResponse::Ok
            }
            IpcCommand::MoveColumnLeft => {
                if let Some(workspace) = self.focused_workspace_mut() {
                    workspace.move_column_left();
                    workspace.ensure_focused_visible_animated(viewport_width);
                    info!("Moved column left");
                }
                if let Err(e) = self.apply_layout() {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                IpcResponse::Ok
            }
            IpcCommand::MoveColumnRight => {
                if let Some(workspace) = self.focused_workspace_mut() {
                    workspace.move_column_right();
                    workspace.ensure_focused_visible_animated(viewport_width);
                    info!("Moved column right");
                }
                if let Err(e) = self.apply_layout() {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                IpcResponse::Ok
            }
            IpcCommand::FocusMonitorLeft => {
                let monitors: Vec<_> = self.monitors.values().cloned().collect();
                if let Some(target) = monitor_to_left(&monitors, self.focused_monitor) {
                    let target_id = target.id;
                    self.focused_monitor = target_id;
                    info!("Focused monitor left -> {}", target_id);
                    if let Err(e) = self.apply_layout() {
                        return IpcResponse::error(format!("Failed to apply layout: {}", e));
                    }
                } else {
                    info!("No monitor to the left");
                }
                IpcResponse::Ok
            }
            IpcCommand::FocusMonitorRight => {
                let monitors: Vec<_> = self.monitors.values().cloned().collect();
                if let Some(target) = monitor_to_right(&monitors, self.focused_monitor) {
                    let target_id = target.id;
                    self.focused_monitor = target_id;
                    info!("Focused monitor right -> {}", target_id);
                    if let Err(e) = self.apply_layout() {
                        return IpcResponse::error(format!("Failed to apply layout: {}", e));
                    }
                } else {
                    info!("No monitor to the right");
                }
                IpcResponse::Ok
            }
            IpcCommand::MoveWindowToMonitorLeft => {
                let monitors: Vec<_> = self.monitors.values().cloned().collect();
                if let Some(target) = monitor_to_left(&monitors, self.focused_monitor) {
                    let target_id = target.id;
                    // Get the focused window from current workspace
                    let window_to_move = self.focused_workspace()
                        .and_then(|ws| ws.focused_window());

                    if let Some(hwnd) = window_to_move {
                        // Remove from current workspace
                        if let Some(workspace) = self.focused_workspace_mut() {
                            if let Err(e) = workspace.remove_window(hwnd) {
                                return IpcResponse::error(format!("Failed to remove window: {}", e));
                            }
                        }

                        // Add to target workspace
                        if let Some(target_ws) = self.workspaces.get_mut(&target_id) {
                            if let Err(e) = target_ws.insert_window(hwnd, None) {
                                return IpcResponse::error(format!("Failed to add window to target: {}", e));
                            }
                            let target_viewport = self.monitors.get(&target_id)
                                .map(|m| m.work_area.width)
                                .unwrap_or(FALLBACK_VIEWPORT_WIDTH);
                            target_ws.ensure_focused_visible(target_viewport);
                        }

                        // Follow the window
                        self.focused_monitor = target_id;
                        info!("Moved window {} to monitor {}", hwnd, target_id);

                        if let Err(e) = self.apply_layout() {
                            return IpcResponse::error(format!("Failed to apply layout: {}", e));
                        }
                    } else {
                        info!("No focused window to move");
                    }
                } else {
                    info!("No monitor to the left");
                }
                IpcResponse::Ok
            }
            IpcCommand::MoveWindowToMonitorRight => {
                let monitors: Vec<_> = self.monitors.values().cloned().collect();
                if let Some(target) = monitor_to_right(&monitors, self.focused_monitor) {
                    let target_id = target.id;
                    // Get the focused window from current workspace
                    let window_to_move = self.focused_workspace()
                        .and_then(|ws| ws.focused_window());

                    if let Some(hwnd) = window_to_move {
                        // Remove from current workspace
                        if let Some(workspace) = self.focused_workspace_mut() {
                            if let Err(e) = workspace.remove_window(hwnd) {
                                return IpcResponse::error(format!("Failed to remove window: {}", e));
                            }
                        }

                        // Add to target workspace
                        if let Some(target_ws) = self.workspaces.get_mut(&target_id) {
                            if let Err(e) = target_ws.insert_window(hwnd, None) {
                                return IpcResponse::error(format!("Failed to add window to target: {}", e));
                            }
                            let target_viewport = self.monitors.get(&target_id)
                                .map(|m| m.work_area.width)
                                .unwrap_or(FALLBACK_VIEWPORT_WIDTH);
                            target_ws.ensure_focused_visible(target_viewport);
                        }

                        // Follow the window
                        self.focused_monitor = target_id;
                        info!("Moved window {} to monitor {}", hwnd, target_id);

                        if let Err(e) = self.apply_layout() {
                            return IpcResponse::error(format!("Failed to apply layout: {}", e));
                        }
                    } else {
                        info!("No focused window to move");
                    }
                } else {
                    info!("No monitor to the right");
                }
                IpcResponse::Ok
            }
            IpcCommand::Resize { delta } => {
                if let Some(workspace) = self.focused_workspace_mut() {
                    workspace.resize_focused_column(delta);
                    info!("Resized column by {}", delta);
                }
                if let Err(e) = self.apply_layout() {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                IpcResponse::Ok
            }
            IpcCommand::Scroll { delta } => {
                if let Some(workspace) = self.focused_workspace_mut() {
                    workspace.scroll_by(delta, viewport_width);
                    info!("Scrolled by {}", delta);
                }
                if let Err(e) = self.apply_layout() {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                IpcResponse::Ok
            }
            IpcCommand::QueryWorkspace => {
                if let Some(workspace) = self.focused_workspace() {
                    IpcResponse::WorkspaceState {
                        columns: workspace.column_count(),
                        windows: workspace.window_count(),
                        focused_column: workspace.focused_column_index(),
                        focused_window: workspace.focused_window_index_in_column(),
                        scroll_offset: workspace.scroll_offset(),
                        total_width: workspace.total_width(),
                    }
                } else {
                    IpcResponse::error("No focused workspace")
                }
            }
            IpcCommand::QueryFocused => {
                if let Some(workspace) = self.focused_workspace() {
                    IpcResponse::FocusedWindow {
                        window_id: workspace.focused_window(),
                        column_index: workspace.focused_column_index(),
                        window_index: workspace.focused_window_index_in_column(),
                    }
                } else {
                    IpcResponse::error("No focused workspace")
                }
            }
            IpcCommand::Refresh => {
                match self.enumerate_and_add_windows() {
                    Ok(added) => {
                        info!("Refreshed: added {} new windows across all monitors", added);
                        if let Err(e) = self.apply_layout() {
                            return IpcResponse::error(format!("Failed to apply layout: {}", e));
                        }
                        IpcResponse::Ok
                    }
                    Err(e) => IpcResponse::error(format!("Failed to enumerate windows: {}", e)),
                }
            }
            IpcCommand::Apply => {
                if let Err(e) = self.apply_layout() {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                info!("Applied layout");
                IpcResponse::Ok
            }
            IpcCommand::Reload => {
                match Config::load() {
                    Ok(new_config) => {
                        self.apply_config(new_config);
                        if let Err(e) = self.apply_layout() {
                            return IpcResponse::error(format!("Failed to apply layout: {}", e));
                        }
                        IpcResponse::Ok
                    }
                    Err(e) => IpcResponse::error(format!("Failed to reload config: {}", e)),
                }
            }
            IpcCommand::Stop => {
                // This is handled specially in the event loop
                IpcResponse::Ok
            }
        }
    }

    /// Handle a window lifecycle event.
    fn handle_window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::Created(hwnd) => {
                // Check if any workspace already manages this window
                if self.find_window_workspace(hwnd).is_some() {
                    debug!("Window {} already managed, ignoring create event", hwnd);
                    return;
                }

                // Try to get window info for filtering and monitor assignment
                if let Ok(windows) = enumerate_windows() {
                    if let Some(win_info) = windows.into_iter().find(|w| w.hwnd == hwnd) {
                        // Determine which monitor this window should be on
                        let monitors: Vec<_> = self.monitors.values().cloned().collect();
                        let monitor_id = find_monitor_for_rect(&monitors, &win_info.rect)
                            .map(|m| m.id)
                            .unwrap_or(self.focused_monitor);

                        let width = win_info.rect.width.clamp(
                            self.config.layout.min_column_width,
                            self.config.layout.max_column_width,
                        );

                        if let Some(workspace) = self.workspaces.get_mut(&monitor_id) {
                            match workspace.insert_window(hwnd, Some(width)) {
                                Ok(()) => {
                                    info!(
                                        "Window created: {} ({}) - added to monitor {}",
                                        win_info.title, win_info.class_name, monitor_id
                                    );
                                    let viewport_width = self.monitors.get(&monitor_id)
                                        .map(|m| m.work_area.width)
                                        .unwrap_or(FALLBACK_VIEWPORT_WIDTH);
                                    workspace.ensure_focused_visible_animated(viewport_width);
                                    if let Err(e) = self.apply_layout() {
                                        warn!("Failed to apply layout after window create: {}", e);
                                    }
                                }
                                Err(e) => {
                                    debug!("Failed to add window {}: {}", hwnd, e);
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::Destroyed(hwnd) => {
                // Find which workspace contains this window
                if let Some(monitor_id) = self.find_window_workspace(hwnd) {
                    let viewport_width = self.monitors.get(&monitor_id)
                        .map(|m| m.work_area.width)
                        .unwrap_or(FALLBACK_VIEWPORT_WIDTH);

                    if let Some(workspace) = self.workspaces.get_mut(&monitor_id) {
                        if let Err(e) = workspace.remove_window(hwnd) {
                            warn!("Failed to remove window {}: {}", hwnd, e);
                        } else {
                            info!("Window {} destroyed - removed from monitor {}", hwnd, monitor_id);
                            workspace.ensure_focused_visible_animated(viewport_width);
                            if let Err(e) = self.apply_layout() {
                                warn!("Failed to apply layout after window destroy: {}", e);
                            }
                        }
                    }
                }
            }
            WindowEvent::Focused(hwnd) => {
                // Update focus to match what Windows says is focused
                if let Some(monitor_id) = self.find_window_workspace(hwnd) {
                    // Update focused monitor to match the window's monitor
                    self.focused_monitor = monitor_id;

                    let viewport_width = self.monitors.get(&monitor_id)
                        .map(|m| m.work_area.width)
                        .unwrap_or(FALLBACK_VIEWPORT_WIDTH);

                    if let Some(workspace) = self.workspaces.get_mut(&monitor_id) {
                        if let Err(e) = workspace.focus_window(hwnd) {
                            debug!("Failed to focus window {}: {}", hwnd, e);
                        } else {
                            debug!("Focus changed to window {} on monitor {}", hwnd, monitor_id);
                            workspace.ensure_focused_visible_animated(viewport_width);
                            if let Err(e) = self.apply_layout() {
                                warn!("Failed to apply layout after focus change: {}", e);
                            }
                        }
                    }
                }
            }
            WindowEvent::Minimized(hwnd) => {
                debug!("Window {} minimized", hwnd);
                // Could remove from workspace or mark as minimized
                // For now, just log it
            }
            WindowEvent::Restored(hwnd) => {
                debug!("Window {} restored", hwnd);
                // Apply layout if we manage this window
                if self.find_window_workspace(hwnd).is_some() {
                    if let Err(e) = self.apply_layout() {
                        warn!("Failed to apply layout after window restore: {}", e);
                    }
                }
            }
            WindowEvent::MovedOrResized(hwnd) => {
                // User manually moved/resized a window - could update our state
                // For now, we don't track user-initiated moves
                debug!("Window {} moved/resized by user", hwnd);
            }
        }
    }
}

/// Run the IPC server, accepting connections and dispatching commands.
async fn run_ipc_server(event_tx: mpsc::Sender<DaemonEvent>) {
    let mut is_first_instance = true;

    loop {
        // Create a new pipe server instance
        let server = match ServerOptions::new()
            .first_pipe_instance(is_first_instance)
            .pipe_mode(PipeMode::Byte)
            .create(PIPE_NAME)
        {
            Ok(s) => {
                is_first_instance = false; // Subsequent instances don't need this flag
                s
            }
            Err(e) => {
                error!("Failed to create named pipe server: {}", e);
                if is_first_instance {
                    // If we can't create the first instance, maybe another daemon is running
                    error!("Is another openniri daemon already running?");
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }
        };

        debug!("Waiting for client connection on {}", PIPE_NAME);

        // Wait for a client to connect
        if let Err(e) = server.connect().await {
            error!("Failed to accept client connection: {}", e);
            continue;
        }

        debug!("Client connected");

        // Handle this client
        let event_tx = event_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(server, event_tx).await {
                warn!("Client handler error: {}", e);
            }
        });
    }
}

/// Handle a single client connection.
async fn handle_client(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    event_tx: mpsc::Sender<DaemonEvent>,
) -> Result<()> {
    let (reader, mut writer) = tokio::io::split(pipe);
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // Read command (single line of JSON)
    let bytes_read = reader.read_line(&mut line).await?;
    if bytes_read == 0 {
        return Ok(()); // Client disconnected
    }

    let line = line.trim();
    debug!("Received command: {}", line);

    // Parse the command
    let cmd: IpcCommand = match serde_json::from_str(line) {
        Ok(cmd) => cmd,
        Err(e) => {
            let response = IpcResponse::error(format!("Invalid command: {}", e));
            let response_json = serde_json::to_string(&response)? + "\n";
            writer.write_all(response_json.as_bytes()).await?;
            return Ok(());
        }
    };

    // Check for stop command (special handling)
    let is_stop = matches!(cmd, IpcCommand::Stop);

    // Create a oneshot channel for the response
    let (resp_tx, resp_rx) = oneshot::channel();

    // Send the command to the event loop
    if event_tx
        .send(DaemonEvent::IpcCommand {
            cmd,
            responder: resp_tx,
        })
        .await
        .is_err()
    {
        let response = IpcResponse::error("Daemon is shutting down");
        let response_json = serde_json::to_string(&response)? + "\n";
        writer.write_all(response_json.as_bytes()).await?;
        return Ok(());
    }

    // Wait for the response
    let response = match resp_rx.await {
        Ok(resp) => resp,
        Err(_) => IpcResponse::error("Failed to get response from daemon"),
    };

    // Send response back to client
    let response_json = serde_json::to_string(&response)? + "\n";
    writer.write_all(response_json.as_bytes()).await?;

    // If this was a stop command, signal shutdown
    if is_stop {
        let _ = event_tx.send(DaemonEvent::Shutdown).await;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("OpenNiri daemon starting...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = match Config::load() {
        Ok(cfg) => {
            info!(
                "Configuration loaded: gap={}, outer_gap={}, default_column_width={}",
                cfg.layout.gap, cfg.layout.outer_gap, cfg.layout.default_column_width
            );
            cfg
        }
        Err(e) => {
            warn!("Failed to load configuration: {}. Using defaults.", e);
            Config::default()
        }
    };

    // Detect all monitors
    let monitors = match enumerate_monitors() {
        Ok(monitors) if !monitors.is_empty() => {
            info!("Detected {} monitor(s):", monitors.len());
            for m in &monitors {
                info!(
                    "  Monitor {}: {}x{} (work area: {}x{} at {},{}){} \"{}\"",
                    m.id,
                    m.rect.width,
                    m.rect.height,
                    m.work_area.width,
                    m.work_area.height,
                    m.work_area.x,
                    m.work_area.y,
                    if m.is_primary { " [PRIMARY]" } else { "" },
                    m.device_name
                );
            }
            monitors
        }
        Ok(_) | Err(_) => {
            warn!(
                "Failed to detect monitors, using fallback {}x{}",
                FALLBACK_VIEWPORT_WIDTH, FALLBACK_VIEWPORT_HEIGHT
            );
            vec![MonitorInfo {
                id: 1,
                rect: Rect::new(0, 0, FALLBACK_VIEWPORT_WIDTH, FALLBACK_VIEWPORT_HEIGHT),
                work_area: Rect::new(0, 0, FALLBACK_VIEWPORT_WIDTH, FALLBACK_WORK_AREA_HEIGHT),
                is_primary: true,
                device_name: "Fallback".to_string(),
            }]
        }
    };

    // Initialize state with config and monitors
    let state = Arc::new(Mutex::new(AppState::new_with_config(config, monitors)));

    // Enumerate existing windows
    info!("Enumerating windows...");
    {
        let mut state = state.lock().await;
        match state.enumerate_and_add_windows() {
            Ok(count) => {
                info!("Found and added {} manageable windows", count);
            }
            Err(e) => {
                error!("Failed to enumerate windows: {}", e);
            }
        }

        // Log workspace state for all monitors
        let total_windows: usize = state.workspaces.values().map(|w| w.window_count()).sum();
        let total_columns: usize = state.workspaces.values().map(|w| w.column_count()).sum();
        info!(
            "Workspaces initialized across {} monitors: {} total columns, {} total windows",
            state.workspaces.len(),
            total_columns,
            total_windows
        );

        // Collect viewport widths first to avoid borrow issues
        let monitor_widths: HashMap<MonitorId, i32> = state.monitors
            .iter()
            .map(|(id, m)| (*id, m.work_area.width))
            .collect();

        // Center each workspace on its first column if it has windows
        for (monitor_id, workspace) in state.workspaces.iter_mut() {
            if workspace.column_count() > 0 {
                let width = monitor_widths.get(monitor_id).copied().unwrap_or(FALLBACK_VIEWPORT_WIDTH);
                workspace.ensure_focused_visible(width);
            }
        }
    }

    // Create event channel
    let (event_tx, mut event_rx) = mpsc::channel::<DaemonEvent>(100);

    // Install WinEvent hooks for window lifecycle tracking
    let _hook_handle = match install_event_hooks() {
        Ok((handle, event_receiver)) => {
            info!("WinEvent hooks installed");

            // Spawn task to forward window events from std::sync::mpsc to tokio channel
            let window_event_tx = event_tx.clone();
            std::thread::spawn(move || {
                while let Ok(event) = event_receiver.recv() {
                    // Use blocking_send since we're in a sync thread
                    if window_event_tx.blocking_send(DaemonEvent::WindowEvent(event)).is_err() {
                        break; // Channel closed, daemon shutting down
                    }
                }
            });

            Some(handle)
        }
        Err(e) => {
            warn!("Failed to install WinEvent hooks: {}. Window tracking disabled.", e);
            None
        }
    };

    // Register global hotkeys
    let hotkey_mapping: HashMap<HotkeyId, IpcCommand>;
    let _hotkey_handle = {
        let state = state.lock().await;
        let config_hotkeys = &state.config.hotkeys.bindings;

        // Build hotkey definitions and command mapping
        let mut hotkeys = Vec::new();
        let mut mapping = HashMap::new();
        let mut next_id: HotkeyId = 1;

        for (key_str, cmd_str) in config_hotkeys {
            if let Some((modifiers, vk)) = parse_hotkey_string(key_str) {
                if let Some(cmd) = config::parse_command(cmd_str) {
                    hotkeys.push(Hotkey::new(next_id, modifiers, vk));
                    mapping.insert(next_id, cmd);
                    debug!("Configured hotkey {}: {} -> {:?}", next_id, key_str, cmd_str);
                    next_id += 1;
                } else {
                    warn!("Unknown command in hotkey config: {} -> {}", key_str, cmd_str);
                }
            } else {
                warn!("Invalid hotkey string in config: {}", key_str);
            }
        }

        hotkey_mapping = mapping;

        if hotkeys.is_empty() {
            info!("No hotkeys configured");
            None
        } else {
            match register_hotkeys(hotkeys) {
                Ok((handle, hotkey_receiver)) => {
                    info!("Registered {} global hotkeys", handle.registered_count());

                    // Spawn task to forward hotkey events
                    let hotkey_tx = event_tx.clone();
                    std::thread::spawn(move || {
                        while let Ok(event) = hotkey_receiver.recv() {
                            if hotkey_tx.blocking_send(DaemonEvent::Hotkey(event)).is_err() {
                                break;
                            }
                        }
                    });

                    Some(handle)
                }
                Err(e) => {
                    warn!("Failed to register hotkeys: {}. Global shortcuts disabled.", e);
                    None
                }
            }
        }
    };

    // Spawn IPC server
    let ipc_tx = event_tx.clone();
    tokio::spawn(async move {
        run_ipc_server(ipc_tx).await;
    });

    info!("IPC server listening on {}", PIPE_NAME);
    info!("Ready. Use openniri-cli to send commands.");

    // Animation timer handle - we'll spawn/cancel this as needed
    let mut animation_timer_handle: Option<tokio::task::JoinHandle<()>> = None;
    let animation_running = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Helper function to start animation timer if not already running
    fn start_animation_timer(
        animation_tx: mpsc::Sender<DaemonEvent>,
        animation_running: Arc<std::sync::atomic::AtomicBool>,
    ) -> tokio::task::JoinHandle<()> {
        animation_running.store(true, std::sync::atomic::Ordering::SeqCst);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(ANIMATION_TICK_MS));
            loop {
                interval.tick().await;
                if !animation_running.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                if animation_tx.send(DaemonEvent::AnimationTick).await.is_err() {
                    break; // Channel closed
                }
            }
        })
    }

    // Main event loop
    loop {
        let event = match event_rx.recv().await {
            Some(e) => e,
            None => break,
        };

        match event {
            DaemonEvent::IpcCommand { cmd, responder } => {
                let (response, should_animate) = {
                    let mut state = state.lock().await;
                    let response = state.handle_command(cmd);
                    let animating = state.is_animating();
                    (response, animating)
                };
                // Log if client disconnected before receiving response
                if responder.send(response).is_err() {
                    debug!("Client disconnected before receiving IPC response");
                }

                // Start animation timer if needed
                if should_animate && !animation_running.load(std::sync::atomic::Ordering::SeqCst) {
                    animation_timer_handle = Some(start_animation_timer(
                        event_tx.clone(),
                        animation_running.clone(),
                    ));
                }
            }
            DaemonEvent::WindowEvent(win_event) => {
                let mut state = state.lock().await;
                state.handle_window_event(win_event);
            }
            DaemonEvent::Hotkey(hotkey_event) => {
                let should_animate = if let Some(cmd) = hotkey_mapping.get(&hotkey_event.id) {
                    debug!("Hotkey {} triggered, executing {:?}", hotkey_event.id, cmd);
                    let mut state = state.lock().await;
                    let response = state.handle_command(cmd.clone());
                    if let IpcResponse::Error { message } = response {
                        warn!("Hotkey command failed: {}", message);
                    }
                    state.is_animating()
                } else {
                    warn!("Unknown hotkey ID: {}", hotkey_event.id);
                    false
                };

                // Start animation timer if needed
                if should_animate && !animation_running.load(std::sync::atomic::Ordering::SeqCst) {
                    animation_timer_handle = Some(start_animation_timer(
                        event_tx.clone(),
                        animation_running.clone(),
                    ));
                }
            }
            DaemonEvent::AnimationTick => {
                let still_animating = {
                    let mut state = state.lock().await;
                    let running = state.tick_animations(ANIMATION_TICK_MS);
                    if running || state.is_animating() {
                        // Apply layout with current animation state
                        if let Err(e) = state.apply_layout() {
                            warn!("Animation layout failed: {}", e);
                        }
                    }
                    running
                };

                // Stop animation timer if all animations complete
                if !still_animating {
                    animation_running.store(false, std::sync::atomic::Ordering::SeqCst);
                    if let Some(handle) = animation_timer_handle.take() {
                        handle.abort();
                    }
                    debug!("All animations complete");
                }
            }
            DaemonEvent::Shutdown => {
                info!("Shutdown signal received");
                break;
            }
        }
    }

    // Clean up animation timer if running
    if let Some(handle) = animation_timer_handle {
        handle.abort();
    }

    info!("OpenNiri daemon shutting down.");
    Ok(())
}
