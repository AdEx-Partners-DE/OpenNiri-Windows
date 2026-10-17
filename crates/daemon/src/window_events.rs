//! Window lifecycle events, window rules, enumeration and monitor reconciliation.

use crate::config;
use crate::recovery::{is_process_more_privileged_than_daemon, log_skipped_privileged_window};
use crate::state::{AppState, FALLBACK_VIEWPORT_HEIGHT, FALLBACK_VIEWPORT_WIDTH};
use anyhow::Result;
use openniri_core_layout::{Rect, Workspace};
use openniri_platform_win32::{
    enumerate_monitors, enumerate_windows, find_monitor_for_rect, get_process_executable,
    MonitorId, MonitorInfo, WindowEvent,
};
use std::collections::HashSet;
use std::time::Duration;
use tracing::{debug, info, warn};

/// How long after focus-follows-mouse the matching OS focus event is expected.
const MOUSE_FOCUS_EVENT_WINDOW: Duration = Duration::from_secs(1);

impl AppState {
    /// Look up window info for a given window handle.
    ///
    /// In production, calls `enumerate_windows()` and finds the matching entry.
    /// In tests, returns from the injected window info map if available.
    fn lookup_window_info(&self, hwnd: u64) -> Option<openniri_platform_win32::WindowInfo> {
        #[cfg(test)]
        {
            if let Some(info) = self.injected_window_info.get(&hwnd) {
                return Some(info.clone());
            }
        }
        enumerate_windows()
            .ok()
            .and_then(|windows| windows.into_iter().find(|w| w.hwnd == hwnd))
    }

    /// Check whether a window ID is known to this state (managed or injected).
    ///
    /// Used by event validation to skip `is_valid_window` for windows we have
    /// info about, even if they aren't yet managed (e.g., during Created events).
    fn is_known_window(&self, wid: u64) -> bool {
        if self.find_window_workspace(wid).is_some() {
            return true;
        }
        #[cfg(test)]
        {
            if self.injected_window_info.contains_key(&wid) {
                return true;
            }
        }
        false
    }

    /// Reconcile workspaces after monitor configuration change.
    ///
    /// This handles:
    /// - Removing workspaces for disconnected monitors (migrating windows to primary)
    /// - Adding workspaces for newly connected monitors
    fn reconcile_monitors(&mut self, new_monitors: Vec<MonitorInfo>) {
        let new_ids: HashSet<MonitorId> = new_monitors.iter().map(|m| m.id).collect();
        let old_ids: HashSet<MonitorId> = self.monitors.keys().copied().collect();

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
                let mut workspace =
                    Workspace::with_gaps(self.config.layout.gap, self.config.layout.outer_gap);
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

    /// Enumerate windows and add them to the appropriate workspace based on position.
    pub(crate) fn enumerate_and_add_windows(&mut self) -> Result<usize> {
        let windows = enumerate_windows()?;
        let monitors: Vec<_> = self.monitors.values().cloned().collect();
        let mut added = 0;

        for win_info in windows {
            // Refresh re-enumerates every window; keep the ones already managed as they are.
            if self.find_window_workspace(win_info.hwnd).is_some() {
                continue;
            }

            // Get executable name for rule matching
            let executable = get_process_executable(win_info.process_id).unwrap_or_default();

            // Check window rules
            let action =
                self.evaluate_window_rules(&win_info.class_name, &win_info.title, &executable);

            // Skip ignored windows
            if action == config::WindowAction::Ignore {
                debug!(
                    "Ignoring window by rule: {} ({})",
                    win_info.title, win_info.class_name
                );
                continue;
            }

            if is_process_more_privileged_than_daemon(win_info.process_id) {
                log_skipped_privileged_window(&win_info.title);
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
                            let viewport = self
                                .monitors
                                .get(&monitor_id)
                                .map(|m| m.work_area)
                                .unwrap_or_else(|| {
                                    Rect::new(
                                        0,
                                        0,
                                        FALLBACK_VIEWPORT_WIDTH,
                                        FALLBACK_VIEWPORT_HEIGHT,
                                    )
                                });
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
                                    win_info.title,
                                    win_info.class_name,
                                    monitor_id,
                                    rule_rect.width,
                                    rule_rect.height
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
                                    win_info.title,
                                    win_info.class_name,
                                    monitor_id,
                                    win_info.rect.width,
                                    win_info.rect.height
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

    /// Handle a window lifecycle event.
    pub(crate) fn handle_window_event(&mut self, event: WindowEvent) {
        // Get window_id from event for validation (DisplayChange and MouseEnterWindow have no validation needed)
        let window_id = match &event {
            WindowEvent::Created(id)
            | WindowEvent::Destroyed(id)
            | WindowEvent::Focused(id)
            | WindowEvent::Minimized(id)
            | WindowEvent::Restored(id)
            | WindowEvent::MovedOrResized(id) => Some(*id),
            WindowEvent::DisplayChange | WindowEvent::MouseEnterWindow(_) => None,
        };

        // Validate window existence for events that require it.
        // Skip validation for:
        //   - Destroyed events (window is already gone)
        //   - Windows we already know about (managed or injected in tests)
        //   - DisplayChange / MouseEnterWindow (no window to validate)
        if let Some(wid) = window_id {
            if !matches!(event, WindowEvent::Destroyed(_))
                && !self.is_known_window(wid)
                && !openniri_platform_win32::is_valid_window(wid)
            {
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
                if let Some(win_info) = self.lookup_window_info(hwnd) {
                    // Get executable name for rule matching
                    let executable =
                        get_process_executable(win_info.process_id).unwrap_or_default();

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

                    if is_process_more_privileged_than_daemon(win_info.process_id) {
                        log_skipped_privileged_window(&win_info.title);
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

                    let viewport_width = self
                        .monitors
                        .get(&monitor_id)
                        .map(|m| m.work_area.width)
                        .unwrap_or(FALLBACK_VIEWPORT_WIDTH);

                    if let Some(workspace) = self.workspaces.get_mut(&monitor_id) {
                        let added = match action {
                            config::WindowAction::Float => {
                                // Use rule dimensions or default to centered 800x600 window
                                let rect = floating_rect.unwrap_or_else(|| {
                                    let viewport = self
                                        .monitors
                                        .get(&monitor_id)
                                        .map(|m| m.work_area)
                                        .unwrap_or_else(|| {
                                            Rect::new(
                                                0,
                                                0,
                                                FALLBACK_VIEWPORT_WIDTH,
                                                FALLBACK_VIEWPORT_HEIGHT,
                                            )
                                        });
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
                                if self.config.behavior.focus_new_windows {
                                    workspace.insert_window(hwnd, Some(width)).is_ok()
                                } else {
                                    workspace.insert_window_no_focus(hwnd, Some(width)).is_ok()
                                }
                            }
                            config::WindowAction::Ignore => unreachable!(),
                        };

                        if added {
                            info!(
                                "Window created: {} ({}) - added to monitor {} as {:?}",
                                win_info.title, win_info.class_name, monitor_id, action
                            );
                            if self.config.behavior.focus_new_windows {
                                workspace.ensure_focused_visible_animated(viewport_width);
                            }
                            if let Err(e) = self.apply_layout() {
                                warn!("Failed to apply layout after window create: {}", e);
                            }
                            if self.config.behavior.focus_new_windows {
                                self.sync_foreground_window();
                            }
                        } else {
                            debug!("Failed to add window {} to workspace", hwnd);
                        }
                    }
                }
            }
            WindowEvent::Destroyed(hwnd) => {
                // Find which workspace contains this window
                if let Some(monitor_id) = self.find_window_workspace(hwnd) {
                    let viewport_width = self
                        .monitors
                        .get(&monitor_id)
                        .map(|m| m.work_area.width)
                        .unwrap_or(FALLBACK_VIEWPORT_WIDTH);

                    if let Some(workspace) = self.workspaces.get_mut(&monitor_id) {
                        // Try to remove as floating window first
                        let was_floating = workspace.remove_floating(hwnd);

                        if was_floating {
                            info!(
                                "Floating window {} destroyed - removed from monitor {}",
                                hwnd, monitor_id
                            );
                        } else if let Err(e) = workspace.remove_window(hwnd) {
                            warn!("Failed to remove window {}: {}", hwnd, e);
                        } else {
                            info!(
                                "Window {} destroyed - removed from monitor {}",
                                hwnd, monitor_id
                            );
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

                    // Always track the OS-foreground window — including floating
                    // windows — so that ToggleFloating can reliably detect and
                    // unfloat the currently focused floating window.
                    self.previous_focused_hwnd = Some(hwnd);

                    let viewport_width = self
                        .monitors
                        .get(&monitor_id)
                        .map(|m| m.work_area.width)
                        .unwrap_or(FALLBACK_VIEWPORT_WIDTH);

                    if let Some(workspace) = self.workspaces.get_mut(&monitor_id) {
                        if let Err(e) = workspace.focus_window(hwnd) {
                            // Floating windows are not in the tiled column list,
                            // so focus_window fails for them — that's expected.
                            debug!("Failed to focus window {}: {}", hwnd, e);
                        } else if matches!(
                            self.mouse_focus_pending.take(),
                            Some((pending, at)) if pending == hwnd && at.elapsed() < MOUSE_FOCUS_EVENT_WINDOW
                        ) {
                            debug!(
                                "Focus changed to window {} by focus-follows-mouse; not scrolling",
                                hwnd
                            );
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
                if let Some(monitor_id) = self.find_window_workspace(hwnd) {
                    let viewport_width = self
                        .monitors
                        .get(&monitor_id)
                        .map(|m| m.work_area.width)
                        .unwrap_or(FALLBACK_VIEWPORT_WIDTH);
                    if let Some(workspace) = self.workspaces.get_mut(&monitor_id) {
                        let cleared_fullscreen = workspace.clear_fullscreen_if_window(hwnd);
                        if workspace.mark_minimized(hwnd) || cleared_fullscreen {
                            info!("Window {} marked minimized", hwnd);

                            // If the minimized window was the focused window, move focus
                            if workspace.focused_window() == Some(hwnd) {
                                // Try to focus another window in the same column
                                workspace.focus_down();
                                if workspace.focused_window() == Some(hwnd) {
                                    workspace.focus_up();
                                }
                                // If still focused on minimized (only window in column), try next column
                                if workspace.focused_window() == Some(hwnd) {
                                    workspace.focus_right();
                                    if workspace.focused_window() == Some(hwnd) {
                                        workspace.focus_left();
                                    }
                                }
                            }
                            workspace.ensure_focused_visible_animated(viewport_width);

                            if let Err(e) = self.apply_layout() {
                                warn!("Failed to apply layout after minimize: {}", e);
                            }
                            // Keep monitor focus aligned before foreground sync so we don't
                            // accidentally steer foreground to a stale monitor.
                            self.focused_monitor = monitor_id;
                            self.sync_foreground_window();
                        }
                    }
                } else {
                    debug!("Window {} minimized (unmanaged)", hwnd);
                }
            }
            WindowEvent::Restored(hwnd) => {
                if let Some(monitor_id) = self.find_window_workspace(hwnd) {
                    let viewport_width = self
                        .monitors
                        .get(&monitor_id)
                        .map(|m| m.work_area.width)
                        .unwrap_or(FALLBACK_VIEWPORT_WIDTH);
                    let mut should_sync_foreground = false;
                    if let Some(workspace) = self.workspaces.get_mut(&monitor_id) {
                        if workspace.mark_restored(hwnd) {
                            info!("Window {} restored from minimized", hwnd);
                            if workspace.is_floating(hwnd) {
                                // Keep floating restores from stealing focus back to tiled windows.
                                debug!(
                                    "Restored floating window {} without changing tiled focus",
                                    hwnd
                                );
                            } else if let Err(e) = workspace.focus_window(hwnd) {
                                warn!("Failed to focus restored window {}: {}", hwnd, e);
                            } else {
                                workspace.ensure_focused_visible_animated(viewport_width);
                                should_sync_foreground = true;
                            }
                        }
                    }
                    if let Err(e) = self.apply_layout() {
                        warn!("Failed to apply layout after window restore: {}", e);
                    }
                    if should_sync_foreground {
                        self.focused_monitor = monitor_id;
                        self.sync_foreground_window();
                    }
                } else {
                    debug!("Window {} restored (unmanaged)", hwnd);
                }
            }
            WindowEvent::MovedOrResized(hwnd) => {
                // Skip events triggered by our own apply_layout() to avoid feedback loop.
                if self.applying_layout || self.should_suppress_moved_or_resized(hwnd) {
                    debug!(
                        "Suppressing MovedOrResized during/after apply_layout for window {}",
                        hwnd
                    );
                    return;
                }
                // If the window is managed (tiled), snap it back to its layout position.
                // Floating and unmanaged windows are left alone.
                if let Some(monitor_id) = self.find_window_workspace(hwnd) {
                    if let Some(workspace) = self.workspaces.get(&monitor_id) {
                        if !workspace.is_floating(hwnd) {
                            debug!("Managed window {} moved/resized — snapping back", hwnd);
                            if let Err(e) = self.apply_layout() {
                                warn!("Failed to snap back layout after move/resize: {}", e);
                            }
                        } else {
                            debug!("Floating window {} moved/resized by user — ignored", hwnd);
                        }
                    }
                } else {
                    debug!("Unmanaged window {} moved/resized — ignored", hwnd);
                }
            }
            WindowEvent::DisplayChange => {
                // Display configuration changed (monitors added/removed/rearranged)
                info!("Display configuration changed - reconciling monitors");

                // Re-enumerate monitors
                match enumerate_monitors() {
                    Ok(new_monitors) if !new_monitors.is_empty() => {
                        info!(
                            "Detected {} monitor(s) after display change",
                            new_monitors.len()
                        );
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
    /// Revalidate the live cursor target after the debounce delay. Leaving for an
    /// unmanaged window or the desktop does not necessarily produce a hook event.
    /// Returns true if focus was applied.
    pub(crate) fn apply_focus_follows_mouse(
        &mut self,
        hwnd: u64,
        cursor_window: Option<u64>,
    ) -> bool {
        if cursor_window != Some(hwnd) {
            return false;
        }
        if let Some(monitor_id) = self.find_window_workspace(hwnd) {
            // Update focused monitor to match the window's monitor
            self.focused_monitor = monitor_id;

            if let Some(workspace) = self.workspaces.get_mut(&monitor_id) {
                if workspace.is_floating(hwnd) {
                    // Floating windows are managed but not represented in tiled columns.
                    self.previous_focused_hwnd = Some(hwnd);
                    let _ = openniri_platform_win32::set_foreground_window(hwnd);
                    debug!(
                        "Focus-follows-mouse: focused floating window {} on monitor {}",
                        hwnd, monitor_id
                    );
                    return true;
                }
                if let Err(e) = workspace.focus_window(hwnd) {
                    debug!(
                        "Failed to focus window {} for focus-follows-mouse: {}",
                        hwnd, e
                    );
                    return false;
                }
                debug!(
                    "Focus-follows-mouse: focused window {} on monitor {}",
                    hwnd, monitor_id
                );
                // The window under the cursor is already visible. Like niri, do not
                // scroll for mouse focus (see `mouse_focus_pending`).
                self.mouse_focus_pending = Some((hwnd, std::time::Instant::now()));
                self.sync_foreground_window();
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::test_support::*;

    // ========================================================================
    // Window rules and state persistence
    // ========================================================================

    #[test]
    fn test_window_rule_evaluation() {
        let rule = |class: Option<&str>,
                    title: Option<&str>,
                    exe: Option<&str>,
                    action: config::WindowAction,
                    size: Option<(i32, i32)>| config::WindowRule {
            match_class: class.map(str::to_string),
            match_title: title.map(str::to_string),
            match_executable: exe.map(str::to_string),
            action,
            width: size.map(|s| s.0),
            height: size.map(|s| s.1),
        };
        let config = Config {
            window_rules: vec![
                rule(
                    Some("SizedClass"),
                    None,
                    None,
                    config::WindowAction::Float,
                    Some((1024, 768)),
                ),
                rule(
                    None,
                    Some(".*DevTools.*"),
                    None,
                    config::WindowAction::Float,
                    None,
                ),
                rule(
                    None,
                    None,
                    Some("spotify.exe"),
                    config::WindowAction::Ignore,
                    None,
                ),
                // Shadowed by the DevTools rule above: the first matching rule wins.
                rule(
                    None,
                    Some("DevTools.*"),
                    None,
                    config::WindowAction::Ignore,
                    None,
                ),
            ],
            ..Default::default()
        };
        let state = AppState::new_with_config(config, test_monitors());

        let cases = [
            ("SizedClass", "Any", "any.exe", config::WindowAction::Float),
            (
                "AnyClass",
                "DevTools - localhost",
                "chrome.exe",
                config::WindowAction::Float,
            ),
            (
                "SpotifyClass",
                "Spotify",
                "spotify.exe",
                config::WindowAction::Ignore,
            ),
            (
                "SomeClass",
                "Some Title",
                "some.exe",
                config::WindowAction::Tile,
            ),
        ];
        for (class, title, exe, expected) in cases {
            assert_eq!(
                state.evaluate_window_rules(class, title, exe),
                expected,
                "rule evaluation for ({class}, {title}, {exe})"
            );
        }

        let original = Rect::new(100, 100, 640, 480);
        assert_eq!(
            state.get_floating_rect_from_rules("SizedClass", "Any", "any.exe", &original),
            Rect::new(100, 100, 1024, 768),
            "rule dimensions override the window size, position is kept"
        );
        assert_eq!(
            state.get_floating_rect_from_rules("AnyClass", "DevTools", "chrome.exe", &original),
            original,
            "a rule without dimensions keeps the original rect"
        );
    }

    // ========================================================================
    // Monitor hot-plug
    // ========================================================================

    #[test]
    fn test_reconcile_adds_workspaces_for_new_monitors() {
        let mut state = AppState::new_with_config(test_config(), vec![]);
        state.reconcile_monitors(test_monitors());
        assert_eq!(state.workspaces.len(), 1);
        state.reconcile_monitors(two_monitors());
        assert_eq!(state.workspaces.len(), 2);
        assert!(state.workspaces.contains_key(&2));
        // Re-applying the same set is a no-op.
        state.reconcile_monitors(two_monitors());
        assert_eq!(state.workspaces.len(), 2);
        assert_eq!(state.focused_monitor, 1);
    }

    #[test]
    fn test_reconcile_removed_monitor_migrates_windows_and_focus() {
        let mut state = AppState::new_with_config(test_config(), two_monitors());
        let ws = state.workspaces.get_mut(&2).unwrap();
        ws.insert_window(1001, None).unwrap();
        ws.insert_window(1002, None).unwrap();
        state.focused_monitor = 2;

        state.reconcile_monitors(test_monitors());

        assert!(!state.workspaces.contains_key(&2));
        assert_eq!(state.workspaces[&1].window_count(), 2);
        assert_eq!(state.focused_monitor, 1, "focus falls back to primary");
    }

    #[test]
    fn test_reconcile_full_monitor_churn() {
        let mut state = AppState::new_with_config(test_config(), two_monitors());
        let ws = state.workspaces.get_mut(&1).unwrap();
        ws.insert_window(100, None).unwrap();
        ws.insert_window(101, None).unwrap();
        state
            .workspaces
            .get_mut(&2)
            .unwrap()
            .insert_window(200, None)
            .unwrap();

        // Replace ALL monitors with entirely new ones (ids 3 and 4)
        state.reconcile_monitors(vec![
            monitor(3, 0, true, "DISPLAY3"),
            monitor(4, 1920, false, "DISPLAY4"),
        ]);

        // All 3 windows must have been migrated to the new primary (id 3)
        assert_eq!(state.workspaces.len(), 2);
        assert_eq!(state.workspaces[&3].window_count(), 3);
        assert!(state.workspaces.contains_key(&4));
        assert!(!state.workspaces.contains_key(&1));
        assert!(!state.workspaces.contains_key(&2));
        assert_eq!(state.focused_monitor, 3);
    }

    // ========================================================================
    // Window events
    // ========================================================================

    fn state_with_wide_strip() -> AppState {
        let mut state = paused_state(test_monitors());
        let ws = state.focused_workspace_mut().unwrap();
        for id in [100, 200, 300, 400, 500] {
            ws.insert_window(id, Some(800)).unwrap();
        }
        ws.focus_window(100).unwrap();
        ws.ensure_focused_visible(1920);
        ws.stop_animation();
        state
    }

    #[test]
    fn test_focus_event_scrolls_to_window() {
        let mut state = state_with_wide_strip();
        state.handle_window_event(WindowEvent::Focused(500));
        assert_eq!(state.previous_focused_hwnd, Some(500));
        let ws = state.focused_workspace_mut().unwrap();
        assert_eq!(ws.focused_window(), Some(500));
        assert!(
            ws.is_animating(),
            "keyboard/click focus should scroll to the window"
        );
    }

    #[test]
    fn test_focus_event_from_mouse_does_not_scroll() {
        let mut state = state_with_wide_strip();
        state.mouse_focus_pending = Some((500, std::time::Instant::now()));
        state.handle_window_event(WindowEvent::Focused(500));
        assert!(state.mouse_focus_pending.is_none());
        let ws = state.focused_workspace_mut().unwrap();
        assert_eq!(ws.focused_window(), Some(500));
        assert!(
            !ws.is_animating(),
            "focus-follows-mouse must not scroll (feedback loop with the cursor)"
        );
    }

    #[test]
    fn test_stale_mouse_focus_marker_does_not_suppress_scroll() {
        let mut state = state_with_wide_strip();
        state.mouse_focus_pending = Some((
            500,
            std::time::Instant::now() - MOUSE_FOCUS_EVENT_WINDOW - Duration::from_millis(10),
        ));
        state.handle_window_event(WindowEvent::Focused(500));
        assert!(state.focused_workspace_mut().unwrap().is_animating());
    }

    #[test]
    fn test_focus_follows_mouse_tracks_tiled_and_floating_windows() {
        let mut state = paused_state(test_monitors());
        let ws = state.focused_workspace_mut().unwrap();
        ws.insert_window(100, Some(800)).unwrap();
        ws.insert_window(200, Some(800)).unwrap();
        ws.add_floating(500, Rect::new(100, 100, 400, 300)).unwrap();

        assert!(state.apply_focus_follows_mouse(100, Some(100)));
        assert_eq!(state.previous_focused_hwnd, Some(100));
        assert_eq!(
            state.focused_workspace().unwrap().focused_window(),
            Some(100)
        );

        assert!(state.apply_focus_follows_mouse(500, Some(500)));
        assert_eq!(state.previous_focused_hwnd, Some(500));
        assert_eq!(
            state.focused_workspace().unwrap().focused_window(),
            Some(100),
            "floating focus-follows-mouse should not mutate tiled focus"
        );
    }

    #[test]
    fn stale_mouse_focus_preserves_focus_and_monitor() {
        for cursor_window in [None, Some(200), Some(999)] {
            let mut state = paused_state(two_monitors());
            state
                .workspaces
                .get_mut(&1)
                .unwrap()
                .insert_window(100, None)
                .unwrap();
            state
                .workspaces
                .get_mut(&2)
                .unwrap()
                .insert_window(200, None)
                .unwrap();
            state.focused_monitor = 2;
            state.previous_focused_hwnd = Some(200);

            assert!(!state.apply_focus_follows_mouse(100, cursor_window));
            assert_eq!(state.focused_monitor, 2);
            assert_eq!(state.previous_focused_hwnd, Some(200));
            assert_eq!(
                state.focused_workspace().unwrap().focused_window(),
                Some(200)
            );
            assert!(state.mouse_focus_pending.is_none());
        }
    }

    #[test]
    fn test_minimize_moves_focus_and_restore_refocuses() {
        let mut state = paused_state(two_monitors());
        let ws = state.workspaces.get_mut(&2).unwrap();
        ws.insert_window(100, Some(800)).unwrap();
        ws.insert_window(200, Some(800)).unwrap();
        assert_eq!(ws.focused_window(), Some(200));
        state.focused_monitor = 1;

        state.handle_window_event(WindowEvent::Minimized(200));
        let ws = &state.workspaces[&2];
        assert!(ws.is_minimized(200));
        assert!(ws.contains_window(200), "minimized windows stay managed");
        assert_eq!(
            ws.focused_window(),
            Some(100),
            "focus leaves the minimized window"
        );
        assert_eq!(state.focused_monitor, 2, "focus follows the source monitor");

        state.focused_monitor = 1;
        state.handle_window_event(WindowEvent::Restored(200));
        let ws = &state.workspaces[&2];
        assert!(!ws.is_minimized(200));
        assert_eq!(ws.focused_window(), Some(200), "restored window gets focus");
        assert_eq!(state.focused_monitor, 2);
    }

    #[test]
    fn test_minimizing_fullscreen_window_clears_fullscreen() {
        let mut state = paused_state(test_monitors());
        let ws = state.focused_workspace_mut().unwrap();
        ws.insert_window(100, Some(800)).unwrap();
        ws.insert_window(200, Some(800)).unwrap();
        ws.focus_window(100).unwrap();
        ws.toggle_fullscreen();
        assert_eq!(ws.fullscreen_window_id(), Some(100));

        state.handle_window_event(WindowEvent::Minimized(100));

        let viewport = state.focused_viewport();
        let ws = state.focused_workspace().unwrap();
        assert!(
            !ws.is_fullscreen(),
            "Fullscreen should be cleared when fullscreen window is minimized"
        );
        assert!(
            ws.compute_placements(viewport)
                .iter()
                .any(|p| p.window_id == 200),
            "Window 200 should be placed after the fullscreen window is minimized"
        );
    }

    #[test]
    fn test_restored_floating_window_does_not_steal_tiled_focus() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        let ws = state.focused_workspace_mut().unwrap();
        ws.insert_window(100, Some(800)).unwrap();
        ws.add_floating(500, Rect::new(100, 100, 400, 300)).unwrap();
        assert_eq!(ws.focused_window(), Some(100));
        state.previous_focused_hwnd = None;

        state.handle_window_event(WindowEvent::Restored(500));
        assert_eq!(
            state.focused_workspace().unwrap().focused_window(),
            Some(100),
            "restoring a floating window should not steal tiled focus"
        );
        assert_eq!(
            state.previous_focused_hwnd, None,
            "floating restore should not call sync_foreground_window"
        );
    }

    #[test]
    fn test_destroyed_event_removes_tiled_and_floating_windows() {
        let mut state = paused_state(test_monitors());
        let ws = state.focused_workspace_mut().unwrap();
        ws.insert_window(100, Some(800)).unwrap();
        ws.insert_window(200, Some(800)).unwrap();
        ws.add_floating(300, Rect::new(0, 0, 400, 300)).unwrap();

        state.handle_window_event(WindowEvent::Destroyed(100));
        state.handle_window_event(WindowEvent::Destroyed(300));
        state.handle_window_event(WindowEvent::Destroyed(99999));

        let ws = state.focused_workspace().unwrap();
        assert!(!ws.contains_window(100));
        assert!(!ws.contains_window(300), "floating windows are removed too");
        assert!(ws.contains_window(200));
        assert_eq!(ws.window_count(), 1);
    }

    fn make_test_window_info(hwnd: u64) -> openniri_platform_win32::WindowInfo {
        openniri_platform_win32::WindowInfo {
            hwnd,
            title: format!("Test Window {}", hwnd),
            class_name: "TestWindowClass".to_string(),
            process_id: 1000 + hwnd as u32,
            rect: Rect::new(100, 100, 800, 600),
            visible: true,
        }
    }

    #[test]
    fn test_created_event_tiles_window_once() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        state
            .injected_window_info
            .insert(100, make_test_window_info(100));

        state.handle_window_event(WindowEvent::Created(100));
        let ws = state.focused_workspace().unwrap();
        assert!(
            ws.contains_window(100),
            "window should be managed after Created event"
        );
        assert_eq!(ws.window_count(), 1);

        state.handle_window_event(WindowEvent::Created(100));
        assert_eq!(
            state.focused_workspace().unwrap().window_count(),
            1,
            "duplicate Created event should be ignored"
        );
    }

    #[test]
    fn test_created_event_focus_new_windows_false_preserves_focus() {
        let mut config = test_config();
        config.behavior.focus_new_windows = false;
        let mut state = AppState::new_with_config(config, test_monitors());

        // The first window gets focus because the workspace is empty.
        state
            .injected_window_info
            .insert(100, make_test_window_info(100));
        state.handle_window_event(WindowEvent::Created(100));
        assert_eq!(
            state.focused_workspace().unwrap().focused_window(),
            Some(100),
            "first window should get focus even with focus_new_windows=false"
        );

        state
            .injected_window_info
            .insert(200, make_test_window_info(200));
        state.handle_window_event(WindowEvent::Created(200));

        let ws = state.focused_workspace().unwrap();
        assert_eq!(ws.window_count(), 2);
        assert_eq!(
            ws.focused_window(),
            Some(100),
            "focus should stay on window 100 when focus_new_windows=false"
        );
    }
}
