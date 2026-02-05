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
//! - System tray icon and menu

mod config;
mod tray;

use anyhow::Result;
use config::Config;
use openniri_core_layout::{Rect, Workspace};
use serde::{Deserialize, Serialize};
use openniri_ipc::{IpcCommand, IpcResponse, MAX_IPC_MESSAGE_SIZE, PIPE_NAME};
use openniri_platform_win32::{
    enumerate_monitors, enumerate_windows, find_monitor_for_rect, get_process_executable,
    install_event_hooks, install_mouse_hook, monitor_to_left, monitor_to_right,
    overlay::OverlayWindow, parse_hotkey_string, register_gestures, register_hotkeys,
    set_display_change_sender, set_dpi_awareness, uncloak_all_managed_windows,
    uncloak_all_visible_windows, GestureEvent, Hotkey, HotkeyEvent, HotkeyId, MonitorId,
    MonitorInfo, PlatformConfig, WindowEvent,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
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
    /// A touchpad gesture was detected.
    Gesture(GestureEvent),
    /// A tray menu event.
    Tray(tray::TrayEvent),
    /// Animation tick (16ms intervals during animation).
    AnimationTick,
    /// Hide snap hint overlay after timeout.
    HideSnapHint,
    /// Apply focus-follows-mouse focus after delay.
    FocusFollowsMouse { window_id: u64 },
    /// Shutdown signal.
    Shutdown,
}

/// Animation tick interval in milliseconds (~60 FPS).
const ANIMATION_TICK_MS: u64 = 16;

/// IPC read timeout - clients must send within this period.
const IPC_READ_TIMEOUT: Duration = Duration::from_secs(5);

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
    /// Pre-compiled window rules for efficient matching.
    compiled_rules: Vec<config::CompiledWindowRule>,
    /// Previously focused window for border color tracking.
    previous_focused_hwnd: Option<u64>,
    /// Whether tiling is paused.
    paused: bool,
    /// Daemon start time for uptime reporting.
    start_time: std::time::Instant,
}

/// Snapshot of workspace state for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkspaceSnapshot {
    /// Monitor device name (stable across restarts, unlike MonitorId/HMONITOR).
    monitor_device_name: String,
    /// Saved workspace state.
    workspace: Workspace,
}

/// Full daemon state snapshot for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StateSnapshot {
    /// Timestamp when state was saved.
    saved_at: String,
    /// Per-monitor workspace snapshots.
    workspaces: Vec<WorkspaceSnapshot>,
    /// Which monitor was focused (by device name).
    focused_monitor_name: String,
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

        let platform_config = PlatformConfig {
            hide_strategy: if config.appearance.use_cloaking {
                openniri_platform_win32::HideStrategy::Cloak
            } else {
                openniri_platform_win32::HideStrategy::MoveOffScreen
            },
            use_deferred_positioning: config.appearance.use_deferred_positioning,
        };

        let compiled_rules = config.compile_window_rules();

        Self {
            workspaces,
            monitors: monitor_map,
            focused_monitor,
            platform_config,
            config,
            compiled_rules,
            previous_focused_hwnd: None,
            paused: false,
            start_time: std::time::Instant::now(),
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
        self.platform_config.hide_strategy = if config.appearance.use_cloaking {
            openniri_platform_win32::HideStrategy::Cloak
        } else {
            openniri_platform_win32::HideStrategy::MoveOffScreen
        };
        self.compiled_rules = config.compile_window_rules();
        self.config = config;
        info!("Configuration applied to all {} workspaces", self.workspaces.len());
    }

    /// Save current workspace state to disk.
    fn save_state(&self) -> Result<()> {
        let snapshots: Vec<WorkspaceSnapshot> = self
            .workspaces
            .iter()
            .filter_map(|(monitor_id, workspace)| {
                self.monitors.get(monitor_id).map(|monitor| WorkspaceSnapshot {
                    monitor_device_name: monitor.device_name.clone(),
                    workspace: workspace.clone(),
                })
            })
            .collect();

        let focused_name = self
            .monitors
            .get(&self.focused_monitor)
            .map(|m| m.device_name.clone())
            .unwrap_or_default();

        let saved_at = {
            let now = std::time::SystemTime::now();
            match now.duration_since(std::time::UNIX_EPOCH) {
                Ok(d) => format!("{}", d.as_secs()),
                Err(_) => "0".to_string(),
            }
        };

        let snapshot = StateSnapshot {
            saved_at,
            workspaces: snapshots,
            focused_monitor_name: focused_name,
        };

        let state_path = Self::state_file_path();
        if let Some(parent) = state_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&snapshot)?;
        std::fs::write(&state_path, json)?;
        info!("Workspace state saved to {:?}", state_path);
        Ok(())
    }

    /// Load saved workspace state from disk.
    fn load_state() -> Option<StateSnapshot> {
        let state_path = Self::state_file_path();
        match std::fs::read_to_string(&state_path) {
            Ok(json) => match serde_json::from_str(&json) {
                Ok(snapshot) => Some(snapshot),
                Err(e) => {
                    warn!("Failed to parse saved state: {}", e);
                    None
                }
            },
            Err(_) => None,
        }
    }

    /// Get the path for the state file.
    fn state_file_path() -> std::path::PathBuf {
        directories::ProjectDirs::from("", "", "openniri")
            .map(|dirs| dirs.data_dir().join("workspace-state.json"))
            .unwrap_or_else(|| std::path::PathBuf::from("workspace-state.json"))
    }

    /// Restore workspace state from a saved snapshot.
    ///
    /// This should be called AFTER monitors are set up but BEFORE windows are enumerated.
    /// It restores scroll offsets and focus positions. Actual windows will be
    /// re-added during enumeration since window IDs from a previous session are invalid.
    fn restore_state(&mut self, snapshot: &StateSnapshot) {
        for ws_snapshot in &snapshot.workspaces {
            // Find matching monitor by device name
            let monitor_id = self
                .monitors
                .iter()
                .find(|(_, m)| m.device_name == ws_snapshot.monitor_device_name)
                .map(|(&id, _)| id);

            if let Some(id) = monitor_id {
                // Restore scroll offset from saved workspace
                if let Some(workspace) = self.workspaces.get_mut(&id) {
                    let saved_offset = ws_snapshot.workspace.scroll_offset();
                    if saved_offset != 0.0 {
                        let viewport_width = self
                            .monitors
                            .get(&id)
                            .map(|m| m.work_area.width)
                            .unwrap_or(FALLBACK_VIEWPORT_WIDTH);
                        workspace.scroll_by(saved_offset, viewport_width);
                    }
                    info!(
                        "Restored workspace state for monitor '{}'",
                        ws_snapshot.monitor_device_name
                    );
                }
            } else {
                debug!(
                    "Skipping saved workspace for unknown monitor '{}'",
                    ws_snapshot.monitor_device_name
                );
            }
        }

        // Restore focused monitor
        if let Some((&id, _)) = self
            .monitors
            .iter()
            .find(|(_, m)| m.device_name == snapshot.focused_monitor_name)
        {
            self.focused_monitor = id;
        }
    }

    /// Reconcile workspaces after monitor configuration change.
    ///
    /// This handles:
    /// - Removing workspaces for disconnected monitors (migrating windows to primary)
    /// - Adding workspaces for newly connected monitors
    fn reconcile_monitors(&mut self, new_monitors: Vec<MonitorInfo>) {
        let new_ids: HashSet<MonitorId> =
            new_monitors.iter().map(|m| m.id).collect();
        let old_ids: HashSet<MonitorId> =
            self.monitors.keys().copied().collect();

        // Find primary monitor in new config (or first available)
        let primary_id = new_monitors
            .iter()
            .find(|m| m.is_primary)
            .or_else(|| new_monitors.first())
            .map(|m| m.id);

        // Handle added monitors - create new workspaces FIRST so migration
        // targets exist even when all old monitors are replaced with new ones.
        for monitor in &new_monitors {
            if !old_ids.contains(&monitor.id) {
                let mut workspace = Workspace::with_gaps(
                    self.config.layout.gap,
                    self.config.layout.outer_gap,
                );
                workspace.set_default_column_width(self.config.layout.default_column_width);
                workspace.set_centering_mode(self.config.layout.centering_mode.into());
                self.workspaces.insert(monitor.id, workspace);
                info!("Created workspace for new monitor {}", monitor.id);
            }
        }

        // Handle removed monitors - migrate windows to primary
        for removed_id in old_ids.difference(&new_ids) {
            if let Some(old_workspace) = self.workspaces.remove(removed_id) {
                let window_ids = old_workspace.all_window_ids();
                if let Some(primary) = primary_id {
                    if let Some(primary_ws) = self.workspaces.get_mut(&primary) {
                        for window_id in &window_ids {
                            if let Err(e) = primary_ws.insert_window(*window_id, None) {
                                warn!("Failed to migrate window {}: {}", window_id, e);
                            }
                        }
                        info!(
                            "Migrated {} windows from removed monitor {} to primary",
                            window_ids.len(),
                            removed_id
                        );
                    }
                }
            }
            self.monitors.remove(removed_id);
        }

        // Update monitor info
        self.monitors = new_monitors.into_iter().map(|m| (m.id, m)).collect();

        // Update focused monitor if it was removed
        if !self.monitors.contains_key(&self.focused_monitor) {
            self.focused_monitor = primary_id.unwrap_or(0);
        }
    }

    /// Collect all managed window IDs across all workspaces.
    ///
    /// Returns tiled and floating window IDs from every monitor's workspace.
    fn all_managed_window_ids(&self) -> Vec<u64> {
        let mut ids = Vec::new();
        for workspace in self.workspaces.values() {
            ids.extend(workspace.all_window_ids());
        }
        ids
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
    /// No-op when tiling is paused.
    fn apply_layout(&self) -> Result<()> {
        if self.paused {
            return Ok(());
        }
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

    /// Set the OS foreground window to match the workspace's focused window.
    /// Also updates active window border colors if configured.
    fn sync_foreground_window(&mut self) {
        let focused_hwnd = self.focused_workspace()
            .and_then(|ws| ws.focused_window());

        if let Some(hwnd) = focused_hwnd {
            // Update border colors if active_border is enabled
            if self.config.appearance.active_border {
                // Reset previous window's border
                if let Some(prev) = self.previous_focused_hwnd {
                    if prev != hwnd {
                        let _ = openniri_platform_win32::reset_window_border_color(prev);
                    }
                }

                // Set new window's border color
                if let Ok(color) = u32::from_str_radix(&self.config.appearance.active_border_color, 16) {
                    // Convert RGB to BGR for DWM (Windows uses COLORREF = 0x00BBGGRR)
                    let r = (color >> 16) & 0xFF;
                    let g = (color >> 8) & 0xFF;
                    let b = color & 0xFF;
                    let bgr = (b << 16) | (g << 8) | r;
                    let _ = openniri_platform_win32::set_window_border_color(hwnd, bgr);
                }
            }

            // Set foreground window
            let _ = openniri_platform_win32::set_foreground_window(hwnd);
            self.previous_focused_hwnd = Some(hwnd);
        }
    }

    /// Enumerate windows and add them to the appropriate workspace based on position.
    fn enumerate_and_add_windows(&mut self) -> Result<usize> {
        let windows = enumerate_windows()?;
        let monitors: Vec<_> = self.monitors.values().cloned().collect();
        let mut added = 0;

        for win_info in windows {
            // Get executable name for rule matching
            let executable = get_process_executable(win_info.process_id)
                .unwrap_or_default();

            // Check window rules
            let action = self.evaluate_window_rules(&win_info.class_name, &win_info.title, &executable);

            // Skip ignored windows
            if action == config::WindowAction::Ignore {
                debug!(
                    "Ignoring window by rule: {} ({})",
                    win_info.title, win_info.class_name
                );
                continue;
            }

            // Find which monitor this window is on
            let monitor_id = find_monitor_for_rect(&monitors, &win_info.rect)
                .map(|m| m.id)
                .unwrap_or(self.focused_monitor);

            // Get floating rect before borrowing workspace mutably (to avoid borrow conflict)
            let floating_rect = if action == config::WindowAction::Float {
                Some(self.get_floating_rect_from_rules(
                    &win_info.class_name,
                    &win_info.title,
                    &executable,
                    &win_info.rect,
                ))
            } else {
                None
            };

            if let Some(workspace) = self.workspaces.get_mut(&monitor_id) {
                match action {
                    config::WindowAction::Float => {
                        // Use rule dimensions or default to centered 800x600 window
                        let rule_rect = floating_rect.unwrap_or_else(|| {
                            let viewport = self.monitors.get(&monitor_id)
                                .map(|m| m.work_area)
                                .unwrap_or_else(|| Rect::new(0, 0, FALLBACK_VIEWPORT_WIDTH, FALLBACK_VIEWPORT_HEIGHT));
                            Rect::new(
                                viewport.x + (viewport.width - 800) / 2,
                                viewport.y + (viewport.height - 600) / 2,
                                800,
                                600,
                            )
                        });

                        match workspace.add_floating(win_info.hwnd, rule_rect) {
                            Ok(()) => {
                                info!(
                                    "Added floating window: {} ({}) to monitor {} - {}x{}",
                                    win_info.title, win_info.class_name, monitor_id,
                                    rule_rect.width, rule_rect.height
                                );
                                added += 1;
                            }
                            Err(e) => {
                                warn!("Failed to add floating window {}: {}", win_info.hwnd, e);
                            }
                        }
                    }
                    config::WindowAction::Tile => {
                        // Use a reasonable default width or the window's current width, respecting config bounds
                        let width = win_info.rect.width.clamp(
                            self.config.layout.min_column_width,
                            self.config.layout.max_column_width,
                        );

                        match workspace.insert_window(win_info.hwnd, Some(width)) {
                            Ok(()) => {
                                info!(
                                    "Added tiled window: {} ({}) to monitor {} - {}x{}",
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
                    config::WindowAction::Ignore => unreachable!(), // Handled above
                }
            }
        }

        Ok(added)
    }

    /// Evaluate window rules and return the action for a window.
    fn evaluate_window_rules(
        &self,
        class_name: &str,
        title: &str,
        executable: &str,
    ) -> config::WindowAction {
        for rule in &self.compiled_rules {
            if rule.matches(class_name, title, executable) {
                return rule.action;
            }
        }
        config::WindowAction::Tile // Default
    }

    /// Get the floating rect for a window based on rules.
    fn get_floating_rect_from_rules(
        &self,
        class_name: &str,
        title: &str,
        executable: &str,
        original_rect: &openniri_core_layout::Rect,
    ) -> openniri_core_layout::Rect {
        for rule in &self.compiled_rules {
            if rule.matches(class_name, title, executable) {
                let width = rule.width.unwrap_or(original_rect.width);
                let height = rule.height.unwrap_or(original_rect.height);
                return openniri_core_layout::Rect::new(
                    original_rect.x,
                    original_rect.y,
                    width,
                    height,
                );
            }
        }
        *original_rect
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

    /// Get the rectangle of the focused column for snap hint display.
    ///
    /// Returns the absolute screen position of the focused column.
    fn get_focused_column_rect(&self) -> Option<Rect> {
        let workspace = self.focused_workspace()?;
        let monitor = self.monitors.get(&self.focused_monitor)?;
        let placements = workspace.compute_placements(monitor.work_area);

        // Find the placement for the focused window
        let focused_hwnd = workspace.focused_window()?;
        placements
            .iter()
            .find(|p| p.window_id == focused_hwnd)
            .map(|p| p.rect)
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
                self.sync_foreground_window();
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
                self.sync_foreground_window();
                IpcResponse::Ok
            }
            IpcCommand::FocusUp => {
                if let Some(workspace) = self.focused_workspace_mut() {
                    workspace.focus_up();
                    info!("Focus up -> window {}", workspace.focused_window_index_in_column());
                }
                if let Err(e) = self.apply_layout() {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                self.sync_foreground_window();
                IpcResponse::Ok
            }
            IpcCommand::FocusDown => {
                if let Some(workspace) = self.focused_workspace_mut() {
                    workspace.focus_down();
                    info!("Focus down -> window {}", workspace.focused_window_index_in_column());
                }
                if let Err(e) = self.apply_layout() {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                self.sync_foreground_window();
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
                    self.sync_foreground_window();
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
                    self.sync_foreground_window();
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
                        self.sync_foreground_window();
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
                        self.sync_foreground_window();
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
            IpcCommand::QueryAllWindows => {
                let mut windows = Vec::new();

                // Get focused window for comparison
                let focused_hwnd = self.focused_workspace()
                    .and_then(|ws| ws.focused_window());

                // Enumerate all windows to get titles and other info
                let win_info_map: HashMap<u64, (String, String, u32)> =
                    match enumerate_windows() {
                        Ok(wins) => wins.into_iter()
                            .map(|w| (w.hwnd, (w.title, w.class_name, w.process_id)))
                            .collect(),
                        Err(_) => HashMap::new(),
                    };

                for (monitor_id, workspace) in &self.workspaces {
                    // Tiled windows
                    for (col_idx, column) in workspace.columns().iter().enumerate() {
                        for (win_idx, &window_id) in column.windows().iter().enumerate() {
                            let (title, class_name, process_id) = win_info_map
                                .get(&window_id)
                                .cloned()
                                .unwrap_or_else(|| ("Unknown".to_string(), "Unknown".to_string(), 0));

                            let executable = get_process_executable(process_id)
                                .unwrap_or_default();

                            // Get rect from computed placements
                            let rect = self.monitors.get(monitor_id)
                                .map(|m| workspace.compute_placements(m.work_area))
                                .and_then(|placements| placements.into_iter()
                                    .find(|p| p.window_id == window_id)
                                    .map(|p| p.rect))
                                .unwrap_or_else(|| Rect::new(0, 0, 0, 0));

                            windows.push(openniri_ipc::WindowInfo {
                                window_id,
                                title,
                                class_name,
                                process_id,
                                executable,
                                rect: openniri_ipc::IpcRect::new(rect.x, rect.y, rect.width, rect.height),
                                column_index: Some(col_idx),
                                window_index: Some(win_idx),
                                monitor_id: *monitor_id as i64,
                                is_floating: false,
                                is_focused: Some(window_id) == focused_hwnd,
                            });
                        }
                    }

                    // Floating windows
                    for floating in workspace.floating_windows() {
                        let (title, class_name, process_id) = win_info_map
                            .get(&floating.id)
                            .cloned()
                            .unwrap_or_else(|| ("Unknown".to_string(), "Unknown".to_string(), 0));

                        let executable = get_process_executable(process_id)
                            .unwrap_or_default();

                        windows.push(openniri_ipc::WindowInfo {
                            window_id: floating.id,
                            title,
                            class_name,
                            process_id,
                            executable,
                            rect: openniri_ipc::IpcRect::new(
                                floating.rect.x,
                                floating.rect.y,
                                floating.rect.width,
                                floating.rect.height
                            ),
                            column_index: None,
                            window_index: None,
                            monitor_id: *monitor_id as i64,
                            is_floating: true,
                            is_focused: Some(floating.id) == focused_hwnd,
                        });
                    }
                }

                IpcResponse::WindowList { windows }
            }
            IpcCommand::CloseWindow => {
                if let Some(hwnd) = self.focused_workspace().and_then(|ws| ws.focused_window()) {
                    if let Err(e) = openniri_platform_win32::close_window(hwnd) {
                        return IpcResponse::error(format!("Failed to close window: {}", e));
                    }
                    info!("Closed window {}", hwnd);
                } else {
                    info!("No focused window to close");
                }
                IpcResponse::Ok
            }
            IpcCommand::ToggleFloating => {
                let viewport = self.focused_viewport();
                if let Some(workspace) = self.focused_workspace_mut() {
                    if let Some(wid) = workspace.toggle_floating(viewport) {
                        info!("Toggled window {} to floating", wid);
                    }
                }
                if let Err(e) = self.apply_layout() {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                self.sync_foreground_window();
                IpcResponse::Ok
            }
            IpcCommand::ToggleFullscreen => {
                if let Some(workspace) = self.focused_workspace_mut() {
                    let entering = workspace.toggle_fullscreen();
                    info!("Fullscreen: {}", if entering { "on" } else { "off" });
                }
                if let Err(e) = self.apply_layout() {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                IpcResponse::Ok
            }
            IpcCommand::SetColumnWidth { fraction } => {
                if let Some(workspace) = self.focused_workspace_mut() {
                    workspace.set_focused_column_width_fraction(fraction, viewport_width);
                    info!("Set column width fraction to {:.3}", fraction);
                }
                if let Err(e) = self.apply_layout() {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                IpcResponse::Ok
            }
            IpcCommand::EqualizeColumnWidths => {
                if let Some(workspace) = self.focused_workspace_mut() {
                    workspace.equalize_column_widths(viewport_width);
                    info!("Equalized column widths");
                }
                if let Err(e) = self.apply_layout() {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                IpcResponse::Ok
            }
            IpcCommand::QueryStatus => {
                let uptime = self.start_time.elapsed().as_secs();
                let total_windows: usize = self.workspaces.values()
                    .map(|ws| ws.window_count() + ws.floating_count())
                    .sum();
                IpcResponse::StatusInfo {
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    monitors: self.monitors.len(),
                    total_windows,
                    uptime_seconds: uptime,
                }
            }
        }
    }

    /// Handle a window lifecycle event.
    fn handle_window_event(&mut self, event: WindowEvent) {
        // Get window_id from event for validation (DisplayChange and MouseEnterWindow have no validation needed)
        let window_id = match &event {
            WindowEvent::Created(id) | WindowEvent::Destroyed(id) |
            WindowEvent::Focused(id) | WindowEvent::Minimized(id) |
            WindowEvent::Restored(id) | WindowEvent::MovedOrResized(id) => Some(*id),
            WindowEvent::DisplayChange | WindowEvent::MouseEnterWindow(_) => None,
        };

        // Skip Destroyed events validation (window is already gone)
        // Skip DisplayChange (no window to validate)
        if let Some(wid) = window_id {
            if !matches!(event, WindowEvent::Destroyed(_)) && !openniri_platform_win32::is_valid_window(wid) {
                debug!("Ignoring event for invalid window {}", wid);
                return;
            }
        }

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
                        // Get executable name for rule matching
                        let executable = get_process_executable(win_info.process_id)
                            .unwrap_or_default();

                        // Check window rules
                        let action = self.evaluate_window_rules(
                            &win_info.class_name,
                            &win_info.title,
                            &executable,
                        );

                        // Skip ignored windows
                        if action == config::WindowAction::Ignore {
                            debug!(
                                "Ignoring window by rule: {} ({})",
                                win_info.title, win_info.class_name
                            );
                            return;
                        }

                        // Determine which monitor this window should be on
                        let monitors: Vec<_> = self.monitors.values().cloned().collect();
                        let monitor_id = find_monitor_for_rect(&monitors, &win_info.rect)
                            .map(|m| m.id)
                            .unwrap_or(self.focused_monitor);

                        // Get floating rect before borrowing workspace mutably
                        let floating_rect = if action == config::WindowAction::Float {
                            Some(self.get_floating_rect_from_rules(
                                &win_info.class_name,
                                &win_info.title,
                                &executable,
                                &win_info.rect,
                            ))
                        } else {
                            None
                        };

                        let viewport_width = self.monitors.get(&monitor_id)
                            .map(|m| m.work_area.width)
                            .unwrap_or(FALLBACK_VIEWPORT_WIDTH);

                        if let Some(workspace) = self.workspaces.get_mut(&monitor_id) {
                            let added = match action {
                                config::WindowAction::Float => {
                                    // Use rule dimensions or default to centered 800x600 window
                                    let rect = floating_rect.unwrap_or_else(|| {
                                        let viewport = self.monitors.get(&monitor_id)
                                            .map(|m| m.work_area)
                                            .unwrap_or_else(|| Rect::new(0, 0, FALLBACK_VIEWPORT_WIDTH, FALLBACK_VIEWPORT_HEIGHT));
                                        Rect::new(
                                            viewport.x + (viewport.width - 800) / 2,
                                            viewport.y + (viewport.height - 600) / 2,
                                            800,
                                            600,
                                        )
                                    });
                                    workspace.add_floating(hwnd, rect).is_ok()
                                }
                                config::WindowAction::Tile => {
                                    let width = win_info.rect.width.clamp(
                                        self.config.layout.min_column_width,
                                        self.config.layout.max_column_width,
                                    );
                                    workspace.insert_window(hwnd, Some(width)).is_ok()
                                }
                                config::WindowAction::Ignore => unreachable!(),
                            };

                            if added {
                                info!(
                                    "Window created: {} ({}) - added to monitor {} as {:?}",
                                    win_info.title, win_info.class_name, monitor_id, action
                                );
                                workspace.ensure_focused_visible_animated(viewport_width);
                                if let Err(e) = self.apply_layout() {
                                    warn!("Failed to apply layout after window create: {}", e);
                                }
                            } else {
                                debug!("Failed to add window {} to workspace", hwnd);
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
                        // Try to remove as floating window first
                        let was_floating = workspace.remove_floating(hwnd);

                        if was_floating {
                            info!("Floating window {} destroyed - removed from monitor {}", hwnd, monitor_id);
                        } else if let Err(e) = workspace.remove_window(hwnd) {
                            warn!("Failed to remove window {}: {}", hwnd, e);
                        } else {
                            info!("Window {} destroyed - removed from monitor {}", hwnd, monitor_id);
                            workspace.ensure_focused_visible_animated(viewport_width);
                        }

                        if let Err(e) = self.apply_layout() {
                            warn!("Failed to apply layout after window destroy: {}", e);
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
            WindowEvent::DisplayChange => {
                // Display configuration changed (monitors added/removed/rearranged)
                info!("Display configuration changed - reconciling monitors");

                // Re-enumerate monitors
                match enumerate_monitors() {
                    Ok(new_monitors) if !new_monitors.is_empty() => {
                        info!("Detected {} monitor(s) after display change", new_monitors.len());
                        for m in &new_monitors {
                            info!(
                                "  Monitor {}: {}x{} at ({},{}){} \"{}\"",
                                m.id,
                                m.work_area.width,
                                m.work_area.height,
                                m.work_area.x,
                                m.work_area.y,
                                if m.is_primary { " [PRIMARY]" } else { "" },
                                m.device_name
                            );
                        }

                        // Reconcile workspaces with new monitor configuration
                        self.reconcile_monitors(new_monitors);

                        // Re-apply layout with updated monitor configuration
                        if let Err(e) = self.apply_layout() {
                            warn!("Failed to apply layout after display change: {}", e);
                        }
                    }
                    Ok(_) => {
                        warn!("No monitors found after display change");
                    }
                    Err(e) => {
                        warn!("Failed to enumerate monitors after display change: {}", e);
                    }
                }
            }
            WindowEvent::MouseEnterWindow(_hwnd) => {
                // This is handled by the main event loop with debouncing
                // (focus_follows_mouse delay)
            }
        }
    }

    /// Apply focus to a window for focus-follows-mouse.
    /// Returns true if focus was applied, false if the window isn't managed.
    fn apply_focus_follows_mouse(&mut self, hwnd: u64) -> bool {
        if let Some(monitor_id) = self.find_window_workspace(hwnd) {
            // Update focused monitor to match the window's monitor
            self.focused_monitor = monitor_id;

            let viewport_width = self.monitors.get(&monitor_id)
                .map(|m| m.work_area.width)
                .unwrap_or(FALLBACK_VIEWPORT_WIDTH);

            if let Some(workspace) = self.workspaces.get_mut(&monitor_id) {
                if let Err(e) = workspace.focus_window(hwnd) {
                    debug!("Failed to focus window {} for focus-follows-mouse: {}", hwnd, e);
                    return false;
                }
                debug!("Focus-follows-mouse: focused window {} on monitor {}", hwnd, monitor_id);
                workspace.ensure_focused_visible_animated(viewport_width);
                if let Err(e) = self.apply_layout() {
                    warn!("Failed to apply layout after focus-follows-mouse: {}", e);
                }
                return true;
            }
        }
        false
    }
}

/// Hotkey registration result containing handle and mapping.
struct HotkeyState {
    /// Handle to unregister hotkeys on drop.
    handle: Option<openniri_platform_win32::HotkeyHandle>,
    /// Mapping of hotkey IDs to commands.
    mapping: HashMap<HotkeyId, IpcCommand>,
}

/// Register hotkeys from config and return state.
///
/// This function is called both at startup and on config reload.
fn setup_hotkeys(
    config: &Config,
    event_tx: mpsc::Sender<DaemonEvent>,
) -> HotkeyState {
    let config_hotkeys = &config.hotkeys.bindings;

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

    if hotkeys.is_empty() {
        info!("No hotkeys configured");
        return HotkeyState { handle: None, mapping };
    }

    match register_hotkeys(hotkeys) {
        Ok((handle, hotkey_receiver)) => {
            info!("Registered {} global hotkeys", handle.registered_count());

            // Spawn task to forward hotkey events
            match std::thread::Builder::new()
                .name("hotkey-fwd".to_string())
                .spawn(move || {
                    while let Ok(event) = hotkey_receiver.recv() {
                        if event_tx.blocking_send(DaemonEvent::Hotkey(event)).is_err() {
                            break;
                        }
                    }
                })
            {
                Ok(_) => {} // Thread is detached, we don't track it
                Err(e) => {
                    warn!("Failed to spawn hotkey-fwd thread: {}", e);
                }
            }

            HotkeyState { handle: Some(handle), mapping }
        }
        Err(e) => {
            warn!("Failed to register hotkeys: {}. Global shortcuts disabled.", e);
            HotkeyState { handle: None, mapping }
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
    let limited_reader = reader.take(MAX_IPC_MESSAGE_SIZE as u64);
    let mut reader = BufReader::new(limited_reader);
    let mut line = String::new();

    // Read command (single line of JSON) with timeout and size bound
    let read_result = tokio::time::timeout(IPC_READ_TIMEOUT, reader.read_line(&mut line)).await;
    let bytes_read = match read_result {
        Ok(Ok(n)) => n,
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => {
            // Timeout: client did not send in time, silently close
            return Ok(());
        }
    };
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
            let response_json = match serde_json::to_string(&response) {
                Ok(json) => json + "\n",
                Err(e) => {
                    warn!("Failed to serialize IPC error response: {}", e);
                    "{\"status\":\"error\",\"message\":\"Internal serialization error\"}\n".to_string()
                }
            };
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
        let response_json = match serde_json::to_string(&response) {
            Ok(json) => json + "\n",
            Err(e) => {
                warn!("Failed to serialize IPC error response: {}", e);
                "{\"status\":\"error\",\"message\":\"Internal serialization error\"}\n".to_string()
            }
        };
        writer.write_all(response_json.as_bytes()).await?;
        return Ok(());
    }

    // Wait for the response
    let response = match resp_rx.await {
        Ok(resp) => resp,
        Err(_) => IpcResponse::error("Failed to get response from daemon"),
    };

    // Send response back to client
    let response_json = match serde_json::to_string(&response) {
        Ok(json) => json + "\n",
        Err(e) => {
            warn!("Failed to serialize IPC response: {}", e);
            "{\"status\":\"error\",\"message\":\"Internal serialization error\"}\n".to_string()
        }
    };
    writer.write_all(response_json.as_bytes()).await?;

    // If this was a stop command, signal shutdown
    if is_stop {
        let _ = event_tx.send(DaemonEvent::Shutdown).await;
    }

    Ok(())
}

/// Spawn a named forwarding thread that receives events from a std::sync::mpsc channel
/// and forwards them to a tokio mpsc sender. Returns the JoinHandle for graceful shutdown.
fn spawn_forwarding_thread<T: Send + 'static>(
    name: &str,
    receiver: std::sync::mpsc::Receiver<T>,
    sender: mpsc::Sender<DaemonEvent>,
    map_fn: impl Fn(T) -> DaemonEvent + Send + 'static,
) -> Result<std::thread::JoinHandle<()>> {
    let thread_name = name.to_string();
    std::thread::Builder::new()
        .name(thread_name.clone())
        .spawn(move || {
            while let Ok(event) = receiver.recv() {
                if sender.blocking_send(map_fn(event)).is_err() {
                    break; // Channel closed, daemon shutting down
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("Failed to spawn {} thread: {}", thread_name, e))
}

/// Check if another daemon instance is already running by probing the named pipe.
async fn check_already_running() -> bool {
    tokio::net::windows::named_pipe::ClientOptions::new()
        .open(PIPE_NAME)
        .is_ok()
}

#[tokio::main]
async fn main() -> Result<()> {
    // Set DPI awareness before any window/GDI operations
    if set_dpi_awareness() {
        eprintln!("[openniri] DPI awareness set to Per-Monitor Aware V2");
    } else {
        eprintln!("[openniri] Warning: Failed to set DPI awareness (may already be set)");
    }

    // Load configuration first (needed for log level)
    let mut config = Config::load().unwrap_or_else(|e| {
        // Can't use tracing yet, fall back to eprintln
        eprintln!("Failed to load configuration: {}. Using defaults.", e);
        Config::default()
    });

    // Initialize logging with configured log level
    let log_level = match config.behavior.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO, // default fallback for invalid values
    };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Validate and clamp config values
    let config_warnings = config.validate();
    for w in &config_warnings {
        warn!("Config: {} - {}", w.field, w.message);
    }

    // Install panic hook to uncloak all windows on crash
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("[openniri] PANIC detected — emergency uncloaking all windows");
        uncloak_all_visible_windows();
        default_hook(info);
    }));

    info!("OpenNiri daemon starting...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Check if another instance is already running
    if check_already_running().await {
        error!("Another openniri-daemon instance is already running (pipe {} is active)", PIPE_NAME);
        return Ok(());
    }

    info!(
        "Configuration loaded: gap={}, outer_gap={}, default_column_width={}, log_level={}",
        config.layout.gap, config.layout.outer_gap, config.layout.default_column_width, config.behavior.log_level
    );

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
    let state = Arc::new(Mutex::new(AppState::new_with_config(config.clone(), monitors)));

    // Try to restore saved workspace state (before enumerating windows)
    {
        let mut state = state.lock().await;
        if let Some(snapshot) = AppState::load_state() {
            state.restore_state(&snapshot);
            info!("Restored workspace state from previous session");
        }
    }

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

    // Collect forwarding thread handles for graceful shutdown
    let mut thread_handles: Vec<std::thread::JoinHandle<()>> = Vec::new();

    // Install WinEvent hooks for window lifecycle tracking (if enabled in config)
    let _hook_handle = if config.behavior.track_focus_changes {
        match install_event_hooks() {
            Ok((handle, event_receiver)) => {
                info!("WinEvent hooks installed");

                // Spawn task to forward window events from std::sync::mpsc to tokio channel
                match spawn_forwarding_thread(
                    "winevent-fwd",
                    event_receiver,
                    event_tx.clone(),
                    DaemonEvent::WindowEvent,
                ) {
                    Ok(handle) => thread_handles.push(handle),
                    Err(e) => warn!("{}", e),
                }

                Some(handle)
            }
            Err(e) => {
                warn!("Failed to install WinEvent hooks: {}. Window tracking disabled.", e);
                None
            }
        }
    } else {
        info!("WinEvent hooks disabled by config (track_focus_changes = false)");
        None
    };

    // Register display change sender for WM_DISPLAYCHANGE events
    // This allows the hotkey window to forward display changes to our event loop
    {
        let (display_tx, display_rx) = std::sync::mpsc::channel::<WindowEvent>();
        if let Err(e) = set_display_change_sender(display_tx) {
            warn!("Failed to register display change sender: {}. Display changes may not be detected.", e);
        } else {
            // Forward display change events to the daemon event loop
            match spawn_forwarding_thread(
                "display-fwd",
                display_rx,
                event_tx.clone(),
                DaemonEvent::WindowEvent,
            ) {
                Ok(handle) => thread_handles.push(handle),
                Err(e) => warn!("{}", e),
            }
            info!("Display change detection enabled");
        }
    }

    // Register global hotkeys (mutable to support reload)
    let mut hotkey_state = setup_hotkeys(&config, event_tx.clone());

    // Install mouse hook for focus-follows-mouse (if enabled)
    let _mouse_hook_handle = if config.behavior.focus_follows_mouse {
        let (mouse_tx, mouse_rx) = std::sync::mpsc::channel::<WindowEvent>();
        match install_mouse_hook(mouse_tx) {
            Ok(handle) => {
                info!("Focus-follows-mouse enabled (delay: {}ms)", config.behavior.focus_follows_mouse_delay_ms);

                // Forward mouse events to the daemon event loop
                match spawn_forwarding_thread(
                    "mouse-fwd",
                    mouse_rx,
                    event_tx.clone(),
                    DaemonEvent::WindowEvent,
                ) {
                    Ok(handle) => thread_handles.push(handle),
                    Err(e) => warn!("{}", e),
                }

                Some(handle)
            }
            Err(e) => {
                warn!("Failed to install mouse hook: {}. Focus-follows-mouse disabled.", e);
                None
            }
        }
    } else {
        info!("Focus-follows-mouse disabled by config (focus_follows_mouse = false)");
        None
    };

    // Register gesture detection (if enabled)
    let _gesture_handle = if config.gestures.enabled {
        match register_gestures() {
            Ok((handle, gesture_receiver)) => {
                info!("Gesture detection enabled");

                // Spawn thread to forward gesture events
                match spawn_forwarding_thread(
                    "gesture-fwd",
                    gesture_receiver,
                    event_tx.clone(),
                    DaemonEvent::Gesture,
                ) {
                    Ok(handle) => thread_handles.push(handle),
                    Err(e) => warn!("{}", e),
                }

                Some(handle)
            }
            Err(e) => {
                warn!("Failed to register gestures: {}. Gesture support disabled.", e);
                None
            }
        }
    } else {
        info!("Gesture detection disabled by config (gestures.enabled = false)");
        None
    };

    // Initialize snap hint overlay (if enabled)
    let snap_hint_overlay: Option<OverlayWindow> = if config.snap_hints.enabled {
        match OverlayWindow::new() {
            Ok(overlay) => {
                info!("Snap hint overlay initialized");
                Some(overlay)
            }
            Err(e) => {
                warn!("Failed to create snap hint overlay: {}. Snap hints disabled.", e);
                None
            }
        }
    } else {
        info!("Snap hints disabled by config (snap_hints.enabled = false)");
        None
    };

    // Initialize system tray icon
    // Create an intermediate sync channel that bridges tray events to the async event loop
    let _tray_manager = {
        let (tray_sync_tx, tray_sync_rx) = std::sync::mpsc::channel();

        // Spawn task to forward tray events from sync channel to async channel
        match spawn_forwarding_thread(
            "tray-fwd",
            tray_sync_rx,
            event_tx.clone(),
            DaemonEvent::Tray,
        ) {
            Ok(handle) => thread_handles.push(handle),
            Err(e) => warn!("{}", e),
        }

        match tray::TrayManager::new(tray_sync_tx) {
            Ok(manager) => {
                info!("System tray icon initialized");
                Some(manager)
            }
            Err(e) => {
                warn!("Failed to create system tray icon: {}. Tray disabled.", e);
                None
            }
        }
    };

    // Spawn IPC server
    let ipc_tx = event_tx.clone();
    tokio::spawn(async move {
        run_ipc_server(ipc_tx).await;
    });

    info!("IPC server listening on {}", PIPE_NAME);

    // Install Ctrl+C handler so terminal kill triggers graceful shutdown
    {
        let shutdown_tx = event_tx.clone();
        tokio::spawn(async move {
            if let Ok(()) = tokio::signal::ctrl_c().await {
                info!("Ctrl+C received, initiating shutdown...");
                let _ = shutdown_tx.send(DaemonEvent::Shutdown).await;
            }
        });
    }

    info!("Ready. Use openniri-cli to send commands.");

    // Animation timer handle - we'll spawn/cancel this as needed
    let mut animation_timer_handle: Option<tokio::task::JoinHandle<()>> = None;
    let animation_running = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Snap hint timer handle - cancels pending hide operation when new hint is shown
    let mut snap_hint_timer_handle: Option<tokio::task::JoinHandle<()>> = None;

    // Focus-follows-mouse timer handle - debounces rapid mouse movements
    let mut focus_follows_mouse_timer: Option<tokio::task::JoinHandle<()>> = None;

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
                let is_reload = matches!(cmd, IpcCommand::Reload);
                let is_resize = matches!(cmd, IpcCommand::Resize { .. });

                let (response, should_animate, column_rect, hint_duration) = {
                    let mut state = state.lock().await;
                    let response = state.handle_command(cmd);
                    let animating = state.is_animating();

                    // Get column rect for snap hint if this is a resize
                    let rect = if is_resize && state.config.snap_hints.enabled {
                        state.get_focused_column_rect()
                    } else {
                        None
                    };
                    let duration = state.config.snap_hints.duration_ms;

                    (response, animating, rect, duration)
                };

                // If config was reloaded successfully, also reload hotkeys
                if is_reload && matches!(response, IpcResponse::Ok) {
                    // Drop old hotkey handle to unregister existing hotkeys
                    hotkey_state.handle = None;

                    // Re-register with new config
                    let new_config = {
                        let state = state.lock().await;
                        state.config.clone()
                    };
                    hotkey_state = setup_hotkeys(&new_config, event_tx.clone());
                    info!("Hotkeys reloaded after config reload");
                }

                // Log if client disconnected before receiving response
                if responder.send(response).is_err() {
                    debug!("Client disconnected before receiving IPC response");
                }

                // Show snap hint for resize operations
                if is_resize {
                    if let (Some(ref overlay), Some(rect)) = (&snap_hint_overlay, column_rect) {
                        // Cancel any pending hide timer
                        if let Some(handle) = snap_hint_timer_handle.take() {
                            handle.abort();
                        }

                        // Show the snap hint
                        overlay.show_snap_target(rect);

                        // Schedule hide after duration
                        let hide_tx = event_tx.clone();
                        let duration = hint_duration;
                        snap_hint_timer_handle = Some(tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(duration as u64)).await;
                            let _ = hide_tx.send(DaemonEvent::HideSnapHint).await;
                        }));
                    }
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
                // Handle MouseEnterWindow specially for focus-follows-mouse debouncing
                if let WindowEvent::MouseEnterWindow(hwnd) = win_event {
                    let (enabled, delay_ms) = {
                        let state = state.lock().await;
                        (
                            state.config.behavior.focus_follows_mouse,
                            state.config.behavior.focus_follows_mouse_delay_ms,
                        )
                    };

                    if enabled {
                        // Cancel any pending focus timer
                        if let Some(handle) = focus_follows_mouse_timer.take() {
                            handle.abort();
                        }

                        // Schedule focus after delay (debouncing)
                        let focus_tx = event_tx.clone();
                        let delay = delay_ms;
                        focus_follows_mouse_timer = Some(tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
                            let _ = focus_tx.send(DaemonEvent::FocusFollowsMouse { window_id: hwnd }).await;
                        }));
                    }
                } else {
                    let mut state = state.lock().await;
                    state.handle_window_event(win_event);
                }
            }
            DaemonEvent::Hotkey(hotkey_event) => {
                let (should_animate, is_resize, column_rect, hint_duration) = if let Some(cmd) = hotkey_state.mapping.get(&hotkey_event.id) {
                    debug!("Hotkey {} triggered, executing {:?}", hotkey_event.id, cmd);
                    let is_resize = matches!(cmd, IpcCommand::Resize { .. });
                    let mut state = state.lock().await;
                    let response = state.handle_command(cmd.clone());
                    if let IpcResponse::Error { message } = response {
                        warn!("Hotkey command failed: {}", message);
                    }
                    let animating = state.is_animating();

                    // Get column rect for snap hint if this is a resize
                    let rect = if is_resize && state.config.snap_hints.enabled {
                        state.get_focused_column_rect()
                    } else {
                        None
                    };
                    let duration = state.config.snap_hints.duration_ms;

                    (animating, is_resize, rect, duration)
                } else {
                    warn!("Unknown hotkey ID: {}", hotkey_event.id);
                    (false, false, None, 200)
                };

                // Show snap hint for resize operations
                if is_resize {
                    if let (Some(ref overlay), Some(rect)) = (&snap_hint_overlay, column_rect) {
                        // Cancel any pending hide timer
                        if let Some(handle) = snap_hint_timer_handle.take() {
                            handle.abort();
                        }

                        // Show the snap hint
                        overlay.show_snap_target(rect);

                        // Schedule hide after duration
                        let hide_tx = event_tx.clone();
                        let duration = hint_duration;
                        snap_hint_timer_handle = Some(tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(duration as u64)).await;
                            let _ = hide_tx.send(DaemonEvent::HideSnapHint).await;
                        }));
                    }
                }

                // Start animation timer if needed
                if should_animate && !animation_running.load(std::sync::atomic::Ordering::SeqCst) {
                    animation_timer_handle = Some(start_animation_timer(
                        event_tx.clone(),
                        animation_running.clone(),
                    ));
                }
            }
            DaemonEvent::Gesture(gesture_event) => {
                // Map gesture to command from config
                let gesture_config = {
                    let state = state.lock().await;
                    state.config.gestures.clone()
                };

                let cmd_str = match gesture_event {
                    GestureEvent::SwipeLeft => &gesture_config.swipe_left,
                    GestureEvent::SwipeRight => &gesture_config.swipe_right,
                    GestureEvent::SwipeUp => &gesture_config.swipe_up,
                    GestureEvent::SwipeDown => &gesture_config.swipe_down,
                };

                if let Some(cmd) = config::parse_command(cmd_str) {
                    debug!("Gesture {:?} triggered, executing {:?}", gesture_event, cmd);
                    let should_animate = {
                        let mut state = state.lock().await;
                        let response = state.handle_command(cmd);
                        if let IpcResponse::Error { message } = response {
                            warn!("Gesture command failed: {}", message);
                        }
                        state.is_animating()
                    };

                    // Start animation timer if needed
                    if should_animate && !animation_running.load(std::sync::atomic::Ordering::SeqCst) {
                        animation_timer_handle = Some(start_animation_timer(
                            event_tx.clone(),
                            animation_running.clone(),
                        ));
                    }
                } else {
                    warn!("Unknown command for gesture: {}", cmd_str);
                }
            }
            DaemonEvent::Tray(tray_event) => {
                match tray_event {
                    tray::TrayEvent::Refresh => {
                        info!("Tray: Refresh requested");
                        let mut state = state.lock().await;
                        let response = state.handle_command(IpcCommand::Refresh);
                        if let IpcResponse::Error { message } = response {
                            warn!("Refresh failed: {}", message);
                        }
                    }
                    tray::TrayEvent::Reload => {
                        info!("Tray: Reload config requested");
                        let response = {
                            let mut state = state.lock().await;
                            state.handle_command(IpcCommand::Reload)
                        };

                        // If config was reloaded successfully, also reload hotkeys
                        if matches!(response, IpcResponse::Ok) {
                            hotkey_state.handle = None;
                            let new_config = {
                                let state = state.lock().await;
                                state.config.clone()
                            };
                            hotkey_state = setup_hotkeys(&new_config, event_tx.clone());
                            info!("Hotkeys reloaded after tray config reload");
                        } else if let IpcResponse::Error { message } = response {
                            warn!("Reload failed: {}", message);
                        }
                    }
                    tray::TrayEvent::Exit => {
                        info!("Tray: Exit requested");
                        // Route tray exit through the unified shutdown path so all
                        // cleanup (save_state + uncloak/reset) stays consistent.
                        let _ = event_tx.send(DaemonEvent::Shutdown).await;
                    }
                    tray::TrayEvent::TogglePause => {
                        let mut state = state.lock().await;
                        state.paused = !state.paused;
                        info!("Tray: Tiling {}", if state.paused { "paused" } else { "resumed" });
                    }
                    tray::TrayEvent::OpenConfig => {
                        info!("Tray: Open config requested");
                        if let Some(dirs) = directories::ProjectDirs::from("", "", "openniri") {
                            let config_path = dirs.config_dir().join("config.toml");
                            let _ = std::process::Command::new("cmd")
                                .args(["/c", "start", "", &config_path.to_string_lossy()])
                                .spawn();
                        }
                    }
                    tray::TrayEvent::ViewLogs => {
                        info!("Tray: View logs requested");
                        let log_dir = std::env::temp_dir();
                        let _ = std::process::Command::new("cmd")
                            .args(["/c", "start", "", &log_dir.to_string_lossy()])
                            .spawn();
                    }
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
            DaemonEvent::HideSnapHint => {
                if let Some(ref overlay) = snap_hint_overlay {
                    overlay.hide();
                    debug!("Snap hint hidden");
                }
            }
            DaemonEvent::FocusFollowsMouse { window_id } => {
                let should_animate = {
                    let mut state = state.lock().await;
                    let applied = state.apply_focus_follows_mouse(window_id);
                    if applied {
                        state.is_animating()
                    } else {
                        false
                    }
                };

                // Start animation timer if needed
                if should_animate && !animation_running.load(std::sync::atomic::Ordering::SeqCst) {
                    animation_timer_handle = Some(start_animation_timer(
                        event_tx.clone(),
                        animation_running.clone(),
                    ));
                }
            }
            DaemonEvent::Shutdown => {
                info!("Shutdown signal received");
                // Save workspace state and uncloak all managed windows before shutting down
                {
                    let state = state.lock().await;
                    if let Err(e) = state.save_state() {
                        warn!("Failed to save workspace state: {}", e);
                    }
                    // Uncloak all managed windows so they remain visible after exit
                    let window_ids = state.all_managed_window_ids();
                    uncloak_all_managed_windows(&window_ids);
                }
                break;
            }
        }
    }

    // Clean up timers if running
    if let Some(handle) = animation_timer_handle {
        handle.abort();
    }
    if let Some(handle) = snap_hint_timer_handle {
        handle.abort();
    }
    if let Some(handle) = focus_follows_mouse_timer {
        handle.abort();
    }

    // Join forwarding threads (with timeout for graceful shutdown)
    info!("Waiting for forwarding threads to exit...");
    for handle in thread_handles {
        let _ = handle.join();
    }

    info!("OpenNiri daemon shutting down.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openniri_core_layout::Rect;

    fn test_config() -> Config {
        Config::default()
    }

    fn test_monitors() -> Vec<MonitorInfo> {
        vec![MonitorInfo {
            id: 1,
            rect: Rect::new(0, 0, 1920, 1080),
            work_area: Rect::new(0, 0, 1920, 1040),
            is_primary: true,
            device_name: "DISPLAY1".to_string(),
        }]
    }

    #[test]
    fn test_app_state_new() {
        let state = AppState::new_with_config(test_config(), test_monitors());
        assert_eq!(state.workspaces.len(), 1);
        assert_eq!(state.focused_monitor, 1);
    }

    #[test]
    fn test_app_state_focused_viewport() {
        let state = AppState::new_with_config(test_config(), test_monitors());
        let viewport = state.focused_viewport();
        assert_eq!(viewport.width, 1920);
        assert_eq!(viewport.height, 1040);
    }

    #[test]
    fn test_app_state_no_monitors_fallback() {
        let state = AppState::new_with_config(test_config(), vec![]);
        let viewport = state.focused_viewport();
        assert_eq!(viewport.width, FALLBACK_VIEWPORT_WIDTH);
        assert_eq!(viewport.height, FALLBACK_VIEWPORT_HEIGHT);
    }

    #[test]
    fn test_window_rule_matching_class() {
        let config = Config {
            window_rules: vec![config::WindowRule {
                match_class: Some("TestClass".to_string()),
                match_title: None,
                match_executable: None,
                action: config::WindowAction::Float,
                width: Some(800),
                height: Some(600),
            }],
            ..Default::default()
        };
        let state = AppState::new_with_config(config, test_monitors());
        let action = state.evaluate_window_rules("TestClass", "Any Title", "any.exe");
        assert_eq!(action, config::WindowAction::Float);
    }

    #[test]
    fn test_window_rule_matching_title() {
        let config = Config {
            window_rules: vec![config::WindowRule {
                match_class: None,
                match_title: Some(".*DevTools.*".to_string()),
                match_executable: None,
                action: config::WindowAction::Float,
                width: None,
                height: None,
            }],
            ..Default::default()
        };
        let state = AppState::new_with_config(config, test_monitors());
        let action = state.evaluate_window_rules("AnyClass", "DevTools - localhost", "chrome.exe");
        assert_eq!(action, config::WindowAction::Float);
    }

    #[test]
    fn test_window_rule_matching_executable() {
        let config = Config {
            window_rules: vec![config::WindowRule {
                match_class: None,
                match_title: None,
                match_executable: Some("spotify.exe".to_string()),
                action: config::WindowAction::Ignore,
                width: None,
                height: None,
            }],
            ..Default::default()
        };
        let state = AppState::new_with_config(config, test_monitors());
        let action = state.evaluate_window_rules("SpotifyClass", "Spotify", "spotify.exe");
        assert_eq!(action, config::WindowAction::Ignore);
    }

    #[test]
    fn test_window_rule_no_match_defaults_to_tile() {
        let state = AppState::new_with_config(test_config(), test_monitors());
        let action = state.evaluate_window_rules("SomeClass", "Some Title", "some.exe");
        assert_eq!(action, config::WindowAction::Tile);
    }

    #[test]
    fn test_floating_rect_uses_rule_dimensions() {
        let config = Config {
            window_rules: vec![config::WindowRule {
                match_class: Some("TestClass".to_string()),
                match_title: None,
                match_executable: None,
                action: config::WindowAction::Float,
                width: Some(1024),
                height: Some(768),
            }],
            ..Default::default()
        };
        let state = AppState::new_with_config(config, test_monitors());
        let original = Rect::new(100, 100, 640, 480);
        let result = state.get_floating_rect_from_rules("TestClass", "Title", "test.exe", &original);
        assert_eq!(result.width, 1024);
        assert_eq!(result.height, 768);
    }

    #[test]
    fn test_floating_rect_preserves_original_if_no_dimensions() {
        let config = Config {
            window_rules: vec![config::WindowRule {
                match_class: Some("TestClass".to_string()),
                match_title: None,
                match_executable: None,
                action: config::WindowAction::Float,
                width: None,
                height: None,
            }],
            ..Default::default()
        };
        let state = AppState::new_with_config(config, test_monitors());
        let original = Rect::new(100, 100, 640, 480);
        let result = state.get_floating_rect_from_rules("TestClass", "Title", "test.exe", &original);
        assert_eq!(result.width, 640);
        assert_eq!(result.height, 480);
    }

    #[test]
    fn test_find_window_workspace_not_found() {
        let state = AppState::new_with_config(test_config(), test_monitors());
        assert!(state.find_window_workspace(99999).is_none());
    }

    #[test]
    fn test_app_state_apply_config() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let mut new_config = test_config();
        new_config.layout.gap = 20;
        new_config.layout.outer_gap = 15;
        state.apply_config(new_config.clone());
        assert_eq!(state.config.layout.gap, 20);
        assert_eq!(state.config.layout.outer_gap, 15);
    }

    #[test]
    fn test_state_file_path() {
        let path = AppState::state_file_path();
        assert!(path.to_str().unwrap().contains("openniri"));
        assert!(path.to_str().unwrap().ends_with("workspace-state.json"));
    }

    #[test]
    fn test_state_snapshot_serialization() {
        let snapshot = StateSnapshot {
            saved_at: "2026-02-04T12:00:00".to_string(),
            workspaces: vec![],
            focused_monitor_name: "DISPLAY1".to_string(),
        };
        let json = serde_json::to_string(&snapshot).expect("serialize");
        let parsed: StateSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.focused_monitor_name, "DISPLAY1");
        assert!(parsed.workspaces.is_empty());
    }

    #[test]
    fn test_workspace_snapshot_serialization() {
        let workspace = Workspace::new();
        let snapshot = WorkspaceSnapshot {
            monitor_device_name: "DISPLAY1".to_string(),
            workspace,
        };
        let json = serde_json::to_string(&snapshot).expect("serialize");
        let parsed: WorkspaceSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.monitor_device_name, "DISPLAY1");
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        // Create a snapshot and verify it roundtrips through serialization
        let snapshot = StateSnapshot {
            saved_at: "2026-02-04T12:00:00".to_string(),
            workspaces: vec![WorkspaceSnapshot {
                monitor_device_name: "DISPLAY1".to_string(),
                workspace: Workspace::with_gaps(10, 10),
            }],
            focused_monitor_name: "DISPLAY1".to_string(),
        };
        let json = serde_json::to_string_pretty(&snapshot).expect("serialize");
        let parsed: StateSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.workspaces.len(), 1);
        assert_eq!(parsed.workspaces[0].monitor_device_name, "DISPLAY1");
    }

    #[test]
    fn test_spawn_forwarding_thread_forwards_events() {
        let (tx, rx) = std::sync::mpsc::channel::<u32>();
        let (async_tx, mut async_rx) = mpsc::channel::<DaemonEvent>(10);

        let _handle = spawn_forwarding_thread("test", rx, async_tx, |_n| {
            DaemonEvent::AnimationTick // Use a simple variant for testing
        }).unwrap();

        tx.send(42).unwrap();
        drop(tx); // Close channel so thread exits

        // Use a runtime to receive
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let event = rt.block_on(async { async_rx.recv().await });
        assert!(event.is_some());
    }

    #[test]
    fn test_spawn_forwarding_thread_stops_on_channel_close() {
        let (tx, rx) = std::sync::mpsc::channel::<u32>();
        let (async_tx, _async_rx) = mpsc::channel::<DaemonEvent>(10);

        let handle = spawn_forwarding_thread("test-close", rx, async_tx, |_| {
            DaemonEvent::AnimationTick
        }).unwrap();

        drop(tx); // Close sender immediately
        // Thread should exit when recv() returns Err
        handle.join().expect("Thread should exit cleanly");
    }

    #[ignore] // Depends on no daemon running; fails when daemon is active
    #[test]
    fn test_check_already_running_returns_false_when_no_daemon() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();
        let result = rt.block_on(check_already_running());
        // No daemon is running during tests, so this should be false
        assert!(!result);
    }

    #[test]
    fn test_ipc_read_timeout_is_reasonable() {
        assert!(IPC_READ_TIMEOUT.as_secs() >= 1);
        assert!(IPC_READ_TIMEOUT.as_secs() <= 30);
    }

    #[test]
    fn test_max_ipc_message_size_is_reasonable() {
        const { assert!(openniri_ipc::MAX_IPC_MESSAGE_SIZE >= 1024) };
        const { assert!(openniri_ipc::MAX_IPC_MESSAGE_SIZE <= 1024 * 1024) };
    }

    // ========================================================================
    // handle_command() Unit Tests
    // ========================================================================

    #[test]
    fn test_cmd_query_workspace_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::QueryWorkspace);
        match resp {
            IpcResponse::WorkspaceState { columns, windows, .. } => {
                assert_eq!(columns, 0);
                assert_eq!(windows, 0);
            }
            _ => panic!("Expected WorkspaceState, got {:?}", resp),
        }
    }

    #[test]
    fn test_cmd_query_focused_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::QueryFocused);
        match resp {
            IpcResponse::FocusedWindow { window_id, column_index, window_index } => {
                assert!(window_id.is_none());
                assert_eq!(column_index, 0);
                assert_eq!(window_index, 0);
            }
            _ => panic!("Expected FocusedWindow, got {:?}", resp),
        }
    }

    #[test]
    fn test_cmd_focus_up_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::FocusUp);
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_focus_down_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::FocusDown);
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_stop() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::Stop);
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_focus_left_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::FocusLeft);
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_focus_right_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::FocusRight);
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_move_left_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::MoveColumnLeft);
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_move_right_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::MoveColumnRight);
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_resize_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::Resize { delta: 100 });
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_scroll_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::Scroll { delta: 50.0 });
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_apply() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::Apply);
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_focus_monitor_left_single() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        // With only one monitor, FocusMonitorLeft is a no-op, returns Ok without calling apply_layout
        let resp = state.handle_command(IpcCommand::FocusMonitorLeft);
        assert_eq!(resp, IpcResponse::Ok);
        assert_eq!(state.focused_monitor, 1); // unchanged
    }

    #[test]
    fn test_cmd_focus_monitor_right_single() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::FocusMonitorRight);
        assert_eq!(resp, IpcResponse::Ok);
        assert_eq!(state.focused_monitor, 1); // unchanged
    }

    #[test]
    fn test_cmd_move_to_monitor_left_single() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::MoveWindowToMonitorLeft);
        assert_eq!(resp, IpcResponse::Ok); // no-op: no monitor to the left
    }

    #[test]
    fn test_cmd_move_to_monitor_right_single() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::MoveWindowToMonitorRight);
        assert_eq!(resp, IpcResponse::Ok); // no-op: no monitor to the right
    }

    // ========================================================================
    // reconcile_monitors() Unit Tests
    // ========================================================================

    fn two_monitors() -> Vec<MonitorInfo> {
        vec![
            MonitorInfo {
                id: 1,
                rect: Rect::new(0, 0, 1920, 1080),
                work_area: Rect::new(0, 0, 1920, 1040),
                is_primary: true,
                device_name: "DISPLAY1".to_string(),
            },
            MonitorInfo {
                id: 2,
                rect: Rect::new(1920, 0, 1920, 1080),
                work_area: Rect::new(1920, 0, 1920, 1040),
                is_primary: false,
                device_name: "DISPLAY2".to_string(),
            },
        ]
    }

    #[test]
    fn test_reconcile_no_change() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let monitors_before = state.workspaces.len();
        state.reconcile_monitors(test_monitors());
        assert_eq!(state.workspaces.len(), monitors_before);
        assert_eq!(state.focused_monitor, 1);
    }

    #[test]
    fn test_reconcile_add_monitor() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        assert_eq!(state.workspaces.len(), 1);
        state.reconcile_monitors(two_monitors());
        assert_eq!(state.workspaces.len(), 2);
        assert!(state.workspaces.contains_key(&2));
    }

    #[test]
    fn test_reconcile_remove_monitor() {
        let mut state = AppState::new_with_config(test_config(), two_monitors());
        assert_eq!(state.workspaces.len(), 2);
        // Remove second monitor, keep only primary
        state.reconcile_monitors(test_monitors());
        assert_eq!(state.workspaces.len(), 1);
        assert!(state.workspaces.contains_key(&1));
        assert!(!state.workspaces.contains_key(&2));
    }

    #[test]
    fn test_reconcile_remove_focused_monitor() {
        let mut state = AppState::new_with_config(test_config(), two_monitors());
        state.focused_monitor = 2; // Focus on secondary
        // Remove secondary, keep primary
        state.reconcile_monitors(test_monitors());
        // Focus should fall back to primary
        assert_eq!(state.focused_monitor, 1);
    }

    #[test]
    fn test_reconcile_primary_always_exists() {
        let mut state = AppState::new_with_config(test_config(), two_monitors());
        // Remove secondary, keep primary
        state.reconcile_monitors(test_monitors());
        assert!(state.workspaces.contains_key(&1));
    }

    #[test]
    fn test_reconcile_empty_to_multi() {
        let mut state = AppState::new_with_config(test_config(), vec![]);
        assert_eq!(state.workspaces.len(), 0);
        state.reconcile_monitors(two_monitors());
        assert_eq!(state.workspaces.len(), 2);
    }

    #[test]
    fn test_reconcile_preserves_windows() {
        let mut state = AppState::new_with_config(test_config(), two_monitors());
        // Add windows to workspace on monitor 2
        if let Some(ws) = state.workspaces.get_mut(&2) {
            ws.insert_window(1001, None).unwrap();
            ws.insert_window(1002, None).unwrap();
        }
        assert_eq!(state.workspaces.get(&2).unwrap().window_count(), 2);

        // Remove monitor 2 - windows should migrate to primary
        state.reconcile_monitors(test_monitors());
        let primary_ws = state.workspaces.get(&1).unwrap();
        assert_eq!(primary_ws.window_count(), 2);
    }

    #[test]
    fn test_reconcile_full_monitor_churn() {
        // Start with monitors 1 and 2, add windows to both
        let mut state = AppState::new_with_config(test_config(), two_monitors());
        state.workspaces.get_mut(&1).unwrap().insert_window(100, None).unwrap();
        state.workspaces.get_mut(&1).unwrap().insert_window(101, None).unwrap();
        state.workspaces.get_mut(&2).unwrap().insert_window(200, None).unwrap();

        // Replace ALL monitors with entirely new ones (ids 3 and 4)
        let new_monitors = vec![
            MonitorInfo {
                id: 3,
                rect: Rect::new(0, 0, 2560, 1440),
                work_area: Rect::new(0, 0, 2560, 1400),
                is_primary: true,
                device_name: "DISPLAY3".to_string(),
            },
            MonitorInfo {
                id: 4,
                rect: Rect::new(2560, 0, 1920, 1080),
                work_area: Rect::new(2560, 0, 1920, 1040),
                is_primary: false,
                device_name: "DISPLAY4".to_string(),
            },
        ];
        state.reconcile_monitors(new_monitors);

        // All 3 windows must have been migrated to the new primary (id 3)
        assert_eq!(state.workspaces.len(), 2);
        let primary_ws = state.workspaces.get(&3).unwrap();
        assert_eq!(primary_ws.window_count(), 3);
        assert!(state.workspaces.contains_key(&4));
        // Old monitors must be gone
        assert!(!state.workspaces.contains_key(&1));
        assert!(!state.workspaces.contains_key(&2));
    }

    // ========================================================================
    // Additional Command Tests
    // ========================================================================

    #[test]
    fn test_cmd_refresh() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::Refresh);
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_reload() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::Reload);
        assert_eq!(resp, IpcResponse::Ok);
        // Config was reloaded (default since no config file in test env)
        assert_eq!(state.config.layout.gap, Config::default().layout.gap);
    }

    #[test]
    fn test_cmd_query_all_windows() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::QueryAllWindows);
        match resp {
            IpcResponse::WindowList { windows } => {
                assert!(windows.is_empty());
            }
            other => panic!("Expected WindowList, got {:?}", other),
        }
    }

    // ========================================================================
    // New command tests (Iteration 29)
    // ========================================================================

    #[test]
    fn test_cmd_close_window_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::CloseWindow);
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_toggle_floating_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::ToggleFloating);
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_toggle_fullscreen_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::ToggleFullscreen);
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_set_column_width_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::SetColumnWidth { fraction: 0.5 });
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_equalize_column_widths_empty() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::EqualizeColumnWidths);
        assert_eq!(resp, IpcResponse::Ok);
    }

    #[test]
    fn test_cmd_query_status() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let resp = state.handle_command(IpcCommand::QueryStatus);
        match resp {
            IpcResponse::StatusInfo { version, monitors, total_windows, uptime_seconds: _ } => {
                assert!(!version.is_empty());
                assert_eq!(monitors, 1);
                assert_eq!(total_windows, 0);
            }
            other => panic!("Expected StatusInfo, got {:?}", other),
        }
    }

    #[test]
    fn test_paused_apply_layout_is_noop() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        state.paused = true;
        // apply_layout should succeed without actually doing anything
        assert!(state.apply_layout().is_ok());
    }

    #[test]
    fn test_start_time_initialized() {
        let state = AppState::new_with_config(test_config(), test_monitors());
        // start_time should be very recent
        assert!(state.start_time.elapsed().as_secs() < 1);
    }

    #[test]
    fn test_all_managed_window_ids_empty() {
        let state = AppState::new_with_config(test_config(), test_monitors());
        let ids = state.all_managed_window_ids();
        assert!(ids.is_empty(), "No windows should exist in a fresh state");
    }

    #[test]
    fn test_all_managed_window_ids_with_windows() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());

        // Add tiled windows
        if let Some(ws) = state.focused_workspace_mut() {
            ws.insert_window(100, Some(800)).unwrap();
            ws.insert_window(200, Some(800)).unwrap();
            // Add a floating window
            ws.add_floating(300, Rect::new(0, 0, 400, 300)).unwrap();
        }

        let ids = state.all_managed_window_ids();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&100));
        assert!(ids.contains(&200));
        assert!(ids.contains(&300));
    }

    #[test]
    fn test_all_managed_window_ids_multi_monitor() {
        let monitors = vec![
            MonitorInfo {
                id: 1,
                rect: Rect::new(0, 0, 1920, 1080),
                work_area: Rect::new(0, 0, 1920, 1040),
                is_primary: true,
                device_name: "DISPLAY1".to_string(),
            },
            MonitorInfo {
                id: 2,
                rect: Rect::new(1920, 0, 1920, 1080),
                work_area: Rect::new(1920, 0, 1920, 1040),
                is_primary: false,
                device_name: "DISPLAY2".to_string(),
            },
        ];

        let mut state = AppState::new_with_config(test_config(), monitors);

        // Add windows to both workspaces
        if let Some(ws) = state.workspaces.get_mut(&1) {
            ws.insert_window(100, Some(800)).unwrap();
        }
        if let Some(ws) = state.workspaces.get_mut(&2) {
            ws.insert_window(200, Some(800)).unwrap();
        }

        let ids = state.all_managed_window_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&100));
        assert!(ids.contains(&200));
    }
}
