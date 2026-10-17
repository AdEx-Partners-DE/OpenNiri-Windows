//! IPC command dispatch: `AppState::handle_command` and its helpers.

use crate::config::Config;
use crate::state::{AppState, FALLBACK_VIEWPORT_WIDTH};
use openniri_core_layout::Rect;
use openniri_ipc::{IpcCommand, IpcResponse};
use openniri_platform_win32::{
    enumerate_windows, get_process_executable, monitor_to_left, monitor_to_right, MonitorId,
};
use std::collections::HashMap;
use tracing::info;

const MIN_SET_WIDTH_FRACTION: f64 = 0.1;
const MAX_SET_WIDTH_FRACTION: f64 = 1.0;

fn validate_set_width_fraction(fraction: f64) -> std::result::Result<(), String> {
    if !fraction.is_finite() {
        return Err("Invalid set-width fraction: value must be finite".to_string());
    }
    if !(MIN_SET_WIDTH_FRACTION..=MAX_SET_WIDTH_FRACTION).contains(&fraction) {
        return Err(format!(
            "Invalid set-width fraction ({}): expected value in [{:.1}, {:.1}]",
            fraction, MIN_SET_WIDTH_FRACTION, MAX_SET_WIDTH_FRACTION
        ));
    }
    Ok(())
}

impl AppState {
    /// Move the focused window to another monitor using an all-or-nothing update.
    ///
    /// The source and target workspaces are mutated on cloned snapshots first and
    /// committed only if both remove and insert operations succeed.
    fn move_focused_window_to_monitor_transactional(
        &mut self,
        target_monitor: MonitorId,
    ) -> std::result::Result<Option<u64>, String> {
        let source_monitor = self.focused_monitor;
        if source_monitor == target_monitor {
            return Ok(None);
        }

        let Some(window_id) = self.focused_workspace().and_then(|ws| ws.focused_window()) else {
            return Ok(None);
        };

        let Some(mut source_workspace) = self.workspaces.get(&source_monitor).cloned() else {
            return Err(format!(
                "Source workspace missing for monitor {}",
                source_monitor
            ));
        };
        let Some(mut target_workspace) = self.workspaces.get(&target_monitor).cloned() else {
            return Err(format!(
                "Target workspace missing for monitor {}",
                target_monitor
            ));
        };

        source_workspace
            .remove_window(window_id)
            .map_err(|e| format!("Failed to remove window: {}", e))?;
        target_workspace
            .insert_window(window_id, None)
            .map_err(|e| format!("Failed to add window to target: {}", e))?;

        let target_viewport = self
            .monitors
            .get(&target_monitor)
            .map(|m| m.work_area.width)
            .unwrap_or(FALLBACK_VIEWPORT_WIDTH);
        target_workspace.ensure_focused_visible(target_viewport);

        self.workspaces.insert(source_monitor, source_workspace);
        self.workspaces.insert(target_monitor, target_workspace);
        self.focused_monitor = target_monitor;
        Ok(Some(window_id))
    }

    /// Process an IPC command and return a response.
    pub(crate) fn handle_command(&mut self, cmd: IpcCommand) -> IpcResponse {
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
                    info!(
                        "Focus up -> window {}",
                        workspace.focused_window_index_in_column()
                    );
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
                    info!(
                        "Focus down -> window {}",
                        workspace.focused_window_index_in_column()
                    );
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
                    match self.move_focused_window_to_monitor_transactional(target_id) {
                        Ok(Some(hwnd)) => {
                            info!("Moved window {} to monitor {}", hwnd, target_id);
                            if let Err(e) = self.apply_layout() {
                                return IpcResponse::error(format!(
                                    "Failed to apply layout: {}",
                                    e
                                ));
                            }
                            self.sync_foreground_window();
                        }
                        Ok(None) => info!("No focused window to move"),
                        Err(message) => return IpcResponse::error(message),
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
                    match self.move_focused_window_to_monitor_transactional(target_id) {
                        Ok(Some(hwnd)) => {
                            info!("Moved window {} to monitor {}", hwnd, target_id);
                            if let Err(e) = self.apply_layout() {
                                return IpcResponse::error(format!(
                                    "Failed to apply layout: {}",
                                    e
                                ));
                            }
                            self.sync_foreground_window();
                        }
                        Ok(None) => info!("No focused window to move"),
                        Err(message) => return IpcResponse::error(message),
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
            IpcCommand::Refresh => match self.enumerate_and_add_windows() {
                Ok(added) => {
                    info!("Refreshed: added {} new windows across all monitors", added);
                    if let Err(e) = self.apply_layout() {
                        return IpcResponse::error(format!("Failed to apply layout: {}", e));
                    }
                    IpcResponse::Ok
                }
                Err(e) => IpcResponse::error(format!("Failed to enumerate windows: {}", e)),
            },
            IpcCommand::Apply => {
                if let Err(e) = self.apply_layout() {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                info!("Applied layout");
                IpcResponse::Ok
            }
            IpcCommand::Reload => match Config::load() {
                Ok(new_config) => {
                    self.apply_config(new_config);
                    if let Err(e) = self.apply_layout() {
                        return IpcResponse::error(format!("Failed to apply layout: {}", e));
                    }
                    IpcResponse::Ok
                }
                Err(e) => IpcResponse::error(format!("Failed to reload config: {}", e)),
            },
            IpcCommand::TogglePause => {
                if let Err(e) = self.toggle_pause("IPC toggle") {
                    return IpcResponse::error(format!("Failed to apply layout: {}", e));
                }
                IpcResponse::Ok
            }
            IpcCommand::Stop => {
                // This is handled specially in the event loop
                IpcResponse::Ok
            }
            IpcCommand::PanicRevert => {
                // This is handled specially in the event loop
                IpcResponse::Ok
            }
            IpcCommand::QueryAllWindows => {
                let mut windows = Vec::new();

                // Get focused window for comparison
                let focused_hwnd = self.focused_workspace().and_then(|ws| ws.focused_window());

                // Enumerate all windows to get titles and other info
                let win_info_map: HashMap<u64, (String, String, u32)> = match enumerate_windows() {
                    Ok(wins) => wins
                        .into_iter()
                        .map(|w| (w.hwnd, (w.title, w.class_name, w.process_id)))
                        .collect(),
                    Err(_) => HashMap::new(),
                };

                for (monitor_id, workspace) in &self.workspaces {
                    // Tiled windows
                    for (col_idx, column) in workspace.columns().iter().enumerate() {
                        for (win_idx, &window_id) in column.windows().iter().enumerate() {
                            // Enumeration skips cloaked windows; read those directly.
                            let (title, class_name, process_id) = win_info_map
                                .get(&window_id)
                                .cloned()
                                .or_else(|| openniri_platform_win32::describe_window(window_id))
                                .unwrap_or_else(|| {
                                    ("Unknown".to_string(), "Unknown".to_string(), 0)
                                });

                            let executable = get_process_executable(process_id).unwrap_or_default();

                            // Get rect from computed placements
                            let rect = self
                                .monitors
                                .get(monitor_id)
                                .map(|m| workspace.compute_placements(m.work_area))
                                .and_then(|placements| {
                                    placements
                                        .into_iter()
                                        .find(|p| p.window_id == window_id)
                                        .map(|p| p.rect)
                                })
                                .unwrap_or_else(|| Rect::new(0, 0, 0, 0));

                            windows.push(openniri_ipc::WindowInfo {
                                window_id,
                                title,
                                class_name,
                                process_id,
                                executable,
                                rect: openniri_ipc::IpcRect::new(
                                    rect.x,
                                    rect.y,
                                    rect.width,
                                    rect.height,
                                ),
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
                            .or_else(|| openniri_platform_win32::describe_window(floating.id))
                            .unwrap_or_else(|| ("Unknown".to_string(), "Unknown".to_string(), 0));

                        let executable = get_process_executable(process_id).unwrap_or_default();

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
                                floating.rect.height,
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
                let prev_hwnd = self.previous_focused_hwnd;
                if let Some(workspace) = self.focused_workspace_mut() {
                    // Check if the OS-foreground window is floating — unfloat it
                    let foreground_is_floating = prev_hwnd
                        .map(|hwnd| workspace.is_floating(hwnd))
                        .unwrap_or(false);
                    if foreground_is_floating {
                        let hwnd = prev_hwnd.unwrap();
                        if workspace.unfloat_window(hwnd) {
                            info!("Unfloated window {} back to tiling", hwnd);
                        }
                    } else if let Some(wid) = workspace.toggle_floating(viewport) {
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
                if let Err(message) = validate_set_width_fraction(fraction) {
                    return IpcResponse::error(message);
                }
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
                let total_windows: usize = self
                    .workspaces
                    .values()
                    .map(|ws| ws.window_count() + ws.floating_count())
                    .sum();
                IpcResponse::StatusInfo {
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    monitors: self.monitors.len(),
                    total_windows,
                    uptime_seconds: uptime,
                }
            }
            IpcCommand::HealthCheck => {
                let uptime = self.start_time.elapsed().as_secs();
                let total_windows: usize = self
                    .workspaces
                    .values()
                    .map(|ws| ws.window_count() + ws.floating_count())
                    .sum();
                IpcResponse::HealthInfo {
                    healthy: true,
                    uptime_seconds: uptime,
                    total_windows,
                    monitors: self.monitors.len(),
                    paused: self.paused,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::TestApplyPlacementsBehavior;
    use crate::test_support::*;
    use openniri_platform_win32::WindowEvent;
    use std::time::Duration;

    // ========================================================================
    // IPC command handling
    // ========================================================================

    #[test]
    fn test_noop_safe_commands_succeed_on_empty_state() {
        let commands = [
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
            IpcCommand::Scroll { delta: 50.0 },
            IpcCommand::QueryWorkspace,
            IpcCommand::QueryFocused,
            IpcCommand::QueryAllWindows,
            IpcCommand::QueryStatus,
            IpcCommand::HealthCheck,
            IpcCommand::Apply,
            IpcCommand::Stop,
            IpcCommand::PanicRevert,
            IpcCommand::CloseWindow,
            IpcCommand::ToggleFloating,
            IpcCommand::ToggleFullscreen,
            IpcCommand::SetColumnWidth { fraction: 0.5 },
            IpcCommand::EqualizeColumnWidths,
        ];
        for cmd in commands {
            let mut state = AppState::new_with_config(test_config(), test_monitors());
            let resp = state.handle_command(cmd.clone());
            assert!(
                !matches!(resp, IpcResponse::Error { .. }),
                "{:?} on an empty state returned {:?}",
                cmd,
                resp
            );
            assert_eq!(state.focused_monitor, 1, "{:?} changed monitor focus", cmd);
            assert!(
                state.all_managed_window_ids().is_empty(),
                "{:?} added windows",
                cmd
            );
        }
    }

    #[test]
    fn test_set_column_width_rejects_invalid_fractions() {
        for (fraction, expected) in [
            (0.05, "Invalid set-width fraction"),
            (1.1, "Invalid set-width fraction"),
            (f64::NAN, "must be finite"),
            (f64::INFINITY, "must be finite"),
        ] {
            let mut state = AppState::new_with_config(test_config(), test_monitors());
            match state.handle_command(IpcCommand::SetColumnWidth { fraction }) {
                IpcResponse::Error { message } => assert!(
                    message.contains(expected),
                    "fraction {fraction}: unexpected message {message}"
                ),
                other => panic!("fraction {fraction}: expected error, got {:?}", other),
            }
        }
    }

    #[test]
    fn test_status_and_health_count_tiled_and_floating_windows() {
        let mut state = paused_state(two_monitors());
        state
            .workspaces
            .get_mut(&1)
            .unwrap()
            .insert_window(100, Some(800))
            .unwrap();
        state
            .workspaces
            .get_mut(&2)
            .unwrap()
            .add_floating(200, Rect::new(2000, 100, 400, 300))
            .unwrap();

        match state.handle_command(IpcCommand::QueryStatus) {
            IpcResponse::StatusInfo {
                monitors,
                total_windows,
                ..
            } => {
                assert_eq!(monitors, 2);
                assert_eq!(total_windows, 2);
            }
            other => panic!("Expected StatusInfo, got {:?}", other),
        }
        match state.handle_command(IpcCommand::HealthCheck) {
            IpcResponse::HealthInfo {
                healthy,
                total_windows,
                monitors,
                paused,
                ..
            } => {
                assert!(healthy);
                assert_eq!(total_windows, 2);
                assert_eq!(monitors, 2);
                assert!(paused, "health check must report the paused state");
            }
            other => panic!("Expected HealthInfo, got {:?}", other),
        }
    }

    #[test]
    fn test_toggle_pause_rolls_back_on_failed_resume() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        assert_eq!(
            state.handle_command(IpcCommand::TogglePause),
            IpcResponse::Ok
        );
        assert!(state.paused, "first toggle_pause should pause tiling");

        state.injected_apply_placements_behavior = Some(TestApplyPlacementsBehavior::SleepAndFail(
            Duration::from_millis(1),
        ));
        match state.handle_command(IpcCommand::TogglePause) {
            IpcResponse::Error { message } => {
                assert!(message.contains("injected apply_placements failure"));
            }
            other => panic!("Expected Error response, got {:?}", other),
        }
        assert!(
            state.paused,
            "failed resume should restore paused state to avoid false resumed status"
        );

        state.injected_apply_placements_behavior = Some(
            TestApplyPlacementsBehavior::SleepAndSucceed(Duration::from_millis(1)),
        );
        assert_eq!(
            state.handle_command(IpcCommand::TogglePause),
            IpcResponse::Ok
        );
        assert!(!state.paused, "successful resume should unpause tiling");
    }

    #[test]
    fn test_toggle_floating_roundtrip() {
        let mut state = paused_state(test_monitors());
        let ws = state.focused_workspace_mut().unwrap();
        ws.insert_window(100, Some(800)).unwrap();
        assert!(!ws.is_floating(100));

        // Tile -> Float: toggle_floating targets the tiled focused window
        let resp = state.handle_command(IpcCommand::ToggleFloating);
        assert_eq!(resp, IpcResponse::Ok);
        let ws = state.focused_workspace_mut().unwrap();
        assert!(ws.is_floating(100), "window should now be floating");

        // Real runtime path: the user clicks the floating window, the OS fires
        // EVENT_SYSTEM_FOREGROUND, and the Focused handler records the hwnd.
        state.handle_window_event(WindowEvent::Focused(100));
        assert_eq!(
            state.previous_focused_hwnd,
            Some(100),
            "Focused event should update previous_focused_hwnd for floating windows"
        );

        // Float -> Tile: ToggleFloating now sees the floating window via previous_focused_hwnd
        let resp = state.handle_command(IpcCommand::ToggleFloating);
        assert_eq!(resp, IpcResponse::Ok);
        let ws = state.focused_workspace_mut().unwrap();
        assert!(
            !ws.is_floating(100),
            "window should be back to tiled after roundtrip"
        );
        assert!(ws.contains_window(100));
    }

    #[test]
    fn test_move_to_monitor_is_all_or_nothing_on_insert_failure() {
        for (source, target, cmd) in [
            (1, 2, IpcCommand::MoveWindowToMonitorRight),
            (2, 1, IpcCommand::MoveWindowToMonitorLeft),
        ] {
            let mut state = AppState::new_with_config(test_config(), two_monitors());
            state.focused_monitor = source;
            state
                .workspaces
                .get_mut(&source)
                .unwrap()
                .insert_window(100, Some(800))
                .unwrap();
            // Force target insert failure (duplicate in target workspace).
            state
                .workspaces
                .get_mut(&target)
                .unwrap()
                .insert_window(100, Some(800))
                .unwrap();

            match state.handle_command(cmd.clone()) {
                IpcResponse::Error { message } => {
                    assert!(message.contains("Failed to add window to target"))
                }
                other => panic!("{:?}: expected error, got {:?}", cmd, other),
            }

            let src = &state.workspaces[&source];
            let dst = &state.workspaces[&target];
            assert_eq!(state.focused_monitor, source, "{:?}", cmd);
            assert_eq!(src.window_count(), 1, "{:?}: source rolled back", cmd);
            assert_eq!(src.focused_window(), Some(100), "{:?}", cmd);
            assert_eq!(dst.window_count(), 1, "{:?}: target unchanged", cmd);
        }
    }
}
