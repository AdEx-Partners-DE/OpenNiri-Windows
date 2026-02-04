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
use openniri_ipc::{IpcCommand, IpcResponse, PIPE_NAME};
use openniri_platform_win32::{
    enumerate_monitors, enumerate_windows, find_monitor_for_rect, get_process_executable,
    install_event_hooks, monitor_to_left, monitor_to_right, overlay::OverlayWindow,
    parse_hotkey_string, register_gestures, register_hotkeys, GestureEvent, Hotkey, HotkeyEvent,
    HotkeyId, MonitorId, MonitorInfo, PlatformConfig, WindowEvent,
};
use std::collections::{HashMap, HashSet};
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
    /// A touchpad gesture was detected.
    Gesture(GestureEvent),
    /// A tray menu event.
    Tray(tray::TrayEvent),
    /// Animation tick (16ms intervals during animation).
    AnimationTick,
    /// Hide snap hint overlay after timeout.
    HideSnapHint,
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

    /// Reconcile workspaces after monitor configuration change.
    ///
    /// This handles:
    /// - Removing workspaces for disconnected monitors (migrating windows to primary)
    /// - Adding workspaces for newly connected monitors
    #[allow(dead_code)] // Infrastructure for display change handling (future use)
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

        // Handle removed monitors - migrate windows to primary
        for removed_id in old_ids.difference(&new_ids) {
            if let Some(old_workspace) = self.workspaces.remove(removed_id) {
                let window_ids = old_workspace.all_window_ids();
                if let Some(primary) = primary_id {
                    if let Some(primary_ws) = self.workspaces.get_mut(&primary) {
                        for window_id in &window_ids {
                            // Try to add as tiled, ignore errors (window might be gone)
                            let _ = primary_ws.insert_window(*window_id, None);
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

        // Handle added monitors - create new workspaces
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

        // Update monitor info
        self.monitors = new_monitors.into_iter().map(|m| (m.id, m)).collect();

        // Update focused monitor if it was removed
        if !self.monitors.contains_key(&self.focused_monitor) {
            self.focused_monitor = primary_id.unwrap_or(0);
        }
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
        for rule in &self.config.window_rules {
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
        for rule in &self.config.window_rules {
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
        }
    }

    /// Handle a window lifecycle event.
    fn handle_window_event(&mut self, event: WindowEvent) {
        // Get window_id from event for validation (DisplayChange has no window ID)
        let window_id = match &event {
            WindowEvent::Created(id) | WindowEvent::Destroyed(id) |
            WindowEvent::Focused(id) | WindowEvent::Minimized(id) |
            WindowEvent::Restored(id) | WindowEvent::MovedOrResized(id) => Some(*id),
            WindowEvent::DisplayChange => None,
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
                // This is handled by Phase 2 robustness code
                info!("Display configuration changed");
                // TODO: Re-enumerate monitors and redistribute workspaces
            }
        }
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
            std::thread::spawn(move || {
                while let Ok(event) = hotkey_receiver.recv() {
                    if event_tx.blocking_send(DaemonEvent::Hotkey(event)).is_err() {
                        break;
                    }
                }
            });

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
    // Load configuration first (needed for log level)
    let config = Config::load().unwrap_or_else(|e| {
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

    info!("OpenNiri daemon starting...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
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

    // Install WinEvent hooks for window lifecycle tracking (if enabled in config)
    let _hook_handle = if config.behavior.track_focus_changes {
        match install_event_hooks() {
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
        }
    } else {
        info!("WinEvent hooks disabled by config (track_focus_changes = false)");
        None
    };

    // Register global hotkeys (mutable to support reload)
    let mut hotkey_state = setup_hotkeys(&config, event_tx.clone());

    // Register gesture detection (if enabled)
    let _gesture_handle = if config.gestures.enabled {
        match register_gestures() {
            Ok((handle, gesture_receiver)) => {
                info!("Gesture detection enabled");

                // Spawn thread to forward gesture events
                let gesture_event_tx = event_tx.clone();
                std::thread::spawn(move || {
                    while let Ok(event) = gesture_receiver.recv() {
                        if gesture_event_tx.blocking_send(DaemonEvent::Gesture(event)).is_err() {
                            break;
                        }
                    }
                });

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
        let tray_event_tx = event_tx.clone();

        // Spawn task to forward tray events from sync channel to async channel
        std::thread::spawn(move || {
            while let Ok(event) = tray_sync_rx.recv() {
                if tray_event_tx.blocking_send(DaemonEvent::Tray(event)).is_err() {
                    break; // Channel closed
                }
            }
        });

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
    info!("Ready. Use openniri-cli to send commands.");

    // Animation timer handle - we'll spawn/cancel this as needed
    let mut animation_timer_handle: Option<tokio::task::JoinHandle<()>> = None;
    let animation_running = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Snap hint timer handle - cancels pending hide operation when new hint is shown
    let mut snap_hint_timer_handle: Option<tokio::task::JoinHandle<()>> = None;

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
                let mut state = state.lock().await;
                state.handle_window_event(win_event);
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
                        break;
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
            DaemonEvent::Shutdown => {
                info!("Shutdown signal received");
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
}
