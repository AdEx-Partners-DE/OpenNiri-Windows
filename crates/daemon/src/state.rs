//! Daemon state: workspaces per monitor, layout application and persistence.

use crate::config::{self, Config};
use crate::recovery::run_visibility_recovery_pass;
use anyhow::{anyhow, Result};
use openniri_core_layout::{Rect, Workspace};
use openniri_platform_win32::{MonitorId, MonitorInfo, PlatformConfig};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Max time allowed for a single Win32 placement apply call.
const APPLY_LAYOUT_TIMEOUT: Duration = Duration::from_millis(1500);
/// Consecutive failed layout applications after which tiling pauses itself.
///
/// A failing apply repeats on every animation frame (about 60 per second); left
/// running it fights the user for window visibility and focus (INC-49).
const APPLY_FAILURE_BREAKER_THRESHOLD: u32 = 5;
/// Suppress MovedOrResized events briefly after placements are applied.
const MOVED_OR_RESIZED_SUPPRESSION_WINDOW: Duration = Duration::from_millis(250);

/// Fallback viewport dimensions when no monitor is detected.
pub(crate) const FALLBACK_VIEWPORT_WIDTH: i32 = 1920;
pub(crate) const FALLBACK_VIEWPORT_HEIGHT: i32 = 1080;
pub(crate) const FALLBACK_WORK_AREA_HEIGHT: i32 = 1040;

fn layout_apply_timeout_message(timeout: Duration) -> String {
    format!(
        "Layout application timed out after {} ms; tiling auto-paused to keep the daemon responsive. Resolve blocked Win32 placement, then use tray 'Pause/Resume Tiling' to resume. If desktop control degrades, run `openniri-cli panic-revert`.",
        timeout.as_millis()
    )
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) enum TestApplyPlacementsBehavior {
    SleepAndSucceed(Duration),
    SleepAndFail(Duration),
}

/// Application state supporting multiple monitors.
pub(crate) struct AppState {
    /// Workspaces indexed by monitor ID.
    pub(crate) workspaces: HashMap<MonitorId, Workspace>,
    /// Monitor info indexed by monitor ID.
    pub(crate) monitors: HashMap<MonitorId, MonitorInfo>,
    /// Currently focused monitor.
    pub(crate) focused_monitor: MonitorId,
    /// Platform configuration.
    pub(crate) platform_config: PlatformConfig,
    /// User configuration.
    pub(crate) config: Config,
    /// Command-line override that must survive configuration reloads.
    pub(crate) force_no_cloak: bool,
    /// Incremented when a parsed configuration is committed, even if layout fails.
    pub(crate) config_revision: u64,
    /// Pre-compiled window rules for efficient matching.
    pub(crate) compiled_rules: Vec<config::CompiledWindowRule>,
    /// Previously focused window for border color tracking.
    pub(crate) previous_focused_hwnd: Option<u64>,
    /// Whether tiling is paused.
    pub(crate) paused: bool,
    /// Guard flag to suppress MovedOrResized events during apply_layout().
    pub(crate) applying_layout: bool,
    /// Per-window suppression deadline for MovedOrResized events after apply_layout().
    pub(crate) moved_or_resized_suppression: HashMap<u64, std::time::Instant>,
    /// Cooperative cancellation flag for placement workers during shutdown/revert.
    pub(crate) apply_worker_cancelled: Arc<AtomicBool>,
    /// Monotonic token to invalidate stale workers when shutdown starts.
    pub(crate) apply_epoch: Arc<AtomicU64>,
    /// Timed-out placement workers retained for join during shutdown/revert.
    pub(crate) pending_apply_workers: Vec<std::thread::JoinHandle<()>>,
    /// Max time allowed for Win32 placement calls before auto-pausing tiling.
    pub(crate) layout_apply_timeout: Duration,
    /// Failed layout applications since the last success.
    pub(crate) consecutive_apply_failures: u32,
    /// Window focused by focus-follows-mouse, and when. The OS focus event that
    /// follows must not scroll the strip: scrolling would put another window under
    /// the cursor, which would then take focus in turn.
    pub(crate) mouse_focus_pending: Option<(u64, std::time::Instant)>,
    /// Daemon start time for uptime reporting.
    pub(crate) start_time: std::time::Instant,
    /// Injected window info for testing. When set, `lookup_window_info()` returns
    /// entries from this map instead of calling `enumerate_windows()`.
    #[cfg(test)]
    pub(crate) injected_window_info: HashMap<u64, openniri_platform_win32::WindowInfo>,
    /// Optional test-only behavior override for placement application.
    #[cfg(test)]
    pub(crate) injected_apply_placements_behavior: Option<TestApplyPlacementsBehavior>,
    /// Number of late-worker recovery passes executed after cancellation.
    #[cfg(test)]
    pub(crate) late_worker_recovery_count: Arc<AtomicUsize>,
}

/// Snapshot of workspace state for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkspaceSnapshot {
    /// Monitor device name (stable across restarts, unlike MonitorId/HMONITOR).
    monitor_device_name: String,
    /// Saved workspace state.
    workspace: Workspace,
}

/// Full daemon state snapshot for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StateSnapshot {
    /// Timestamp when state was saved.
    saved_at: String,
    /// Per-monitor workspace snapshots.
    workspaces: Vec<WorkspaceSnapshot>,
    /// Which monitor was focused (by device name).
    focused_monitor_name: String,
}

impl AppState {
    /// Create new state with config and monitors.
    pub(crate) fn new_with_config(config: Config, monitors: Vec<MonitorInfo>) -> Self {
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
            force_no_cloak: false,
            config_revision: 0,
            compiled_rules,
            previous_focused_hwnd: None,
            paused: false,
            applying_layout: false,
            moved_or_resized_suppression: HashMap::new(),
            apply_worker_cancelled: Arc::new(AtomicBool::new(false)),
            apply_epoch: Arc::new(AtomicU64::new(0)),
            pending_apply_workers: Vec::new(),
            layout_apply_timeout: APPLY_LAYOUT_TIMEOUT,
            consecutive_apply_failures: 0,
            mouse_focus_pending: None,
            start_time: std::time::Instant::now(),
            #[cfg(test)]
            injected_window_info: HashMap::new(),
            #[cfg(test)]
            injected_apply_placements_behavior: None,
            #[cfg(test)]
            late_worker_recovery_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get the currently focused workspace.
    pub(crate) fn focused_workspace(&self) -> Option<&Workspace> {
        self.workspaces.get(&self.focused_monitor)
    }

    /// Get the currently focused workspace mutably.
    pub(crate) fn focused_workspace_mut(&mut self) -> Option<&mut Workspace> {
        self.workspaces.get_mut(&self.focused_monitor)
    }

    /// Get the focused monitor's viewport.
    pub(crate) fn focused_viewport(&self) -> Rect {
        self.monitors
            .get(&self.focused_monitor)
            .map(|m| m.work_area)
            .unwrap_or_else(|| Rect::new(0, 0, FALLBACK_VIEWPORT_WIDTH, FALLBACK_VIEWPORT_HEIGHT))
    }

    /// Apply configuration to all workspaces.
    pub(crate) fn apply_config(&mut self, mut config: Config) {
        if self.force_no_cloak {
            config.appearance.use_cloaking = false;
        }
        for warning in config.validate() {
            warn!("Config: {} - {}", warning.field, warning.message);
        }
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
        self.config_revision = self.config_revision.wrapping_add(1);
        info!(
            "Configuration applied to all {} workspaces",
            self.workspaces.len()
        );
    }

    /// Save current workspace state to disk.
    pub(crate) fn save_state(&self) -> Result<()> {
        let snapshots: Vec<WorkspaceSnapshot> = self
            .workspaces
            .iter()
            .filter_map(|(monitor_id, workspace)| {
                self.monitors
                    .get(monitor_id)
                    .map(|monitor| WorkspaceSnapshot {
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
    pub(crate) fn load_state() -> Option<StateSnapshot> {
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
    /// This should be called AFTER windows are enumerated so that scroll offsets
    /// are not clamped against empty workspaces. Sets the scroll offset directly
    /// (bypassing clamping) to preserve the saved value.
    ///
    /// Returns the set of monitor IDs whose scroll offsets were successfully
    /// restored. The caller should skip `ensure_focused_visible()` for these
    /// monitors to avoid overwriting the restored offset.
    pub(crate) fn restore_state(&mut self, snapshot: &StateSnapshot) -> HashSet<MonitorId> {
        let mut restored_monitors = HashSet::new();

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
                        workspace.set_scroll_offset(saved_offset);
                    }
                    restored_monitors.insert(id);
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

        restored_monitors
    }

    /// Collect all managed window IDs across all workspaces.
    ///
    /// Returns tiled and floating window IDs from every monitor's workspace.
    pub(crate) fn all_managed_window_ids(&self) -> Vec<u64> {
        let mut ids = Vec::new();
        for workspace in self.workspaces.values() {
            ids.extend(workspace.all_window_ids());
        }
        ids
    }

    /// Record a short suppression window for moved/resized feedback generated by apply_layout().
    fn arm_moved_or_resized_suppression<I>(&mut self, window_ids: I)
    where
        I: IntoIterator<Item = u64>,
    {
        let now = std::time::Instant::now();
        self.moved_or_resized_suppression
            .retain(|_, deadline| *deadline > now);
        let deadline = now + MOVED_OR_RESIZED_SUPPRESSION_WINDOW;
        for hwnd in window_ids {
            self.moved_or_resized_suppression.insert(hwnd, deadline);
        }
    }

    /// Returns true when a moved/resized event should be ignored due to recent apply_layout().
    pub(crate) fn should_suppress_moved_or_resized(&mut self, hwnd: u64) -> bool {
        let now = std::time::Instant::now();
        self.moved_or_resized_suppression
            .retain(|_, deadline| *deadline > now);
        self.moved_or_resized_suppression
            .get(&hwnd)
            .is_some_and(|deadline| *deadline > now)
    }

    /// Join any finished timed-out apply workers so the pending list does not grow indefinitely.
    /// Returns the number of workers reaped in this pass.
    fn reap_finished_pending_apply_workers(&mut self) -> usize {
        if self.pending_apply_workers.is_empty() {
            return 0;
        }
        let mut still_running = Vec::with_capacity(self.pending_apply_workers.len());
        let mut reaped = 0usize;
        for handle in self.pending_apply_workers.drain(..) {
            if handle.is_finished() {
                let _ = handle.join();
                reaped += 1;
            } else {
                still_running.push(handle);
            }
        }
        self.pending_apply_workers = still_running;
        reaped
    }

    /// Mark shutdown/revert in progress and take ownership of any timed-out apply workers.
    pub(crate) fn begin_shutdown_or_revert(&mut self) -> Vec<std::thread::JoinHandle<()>> {
        self.apply_worker_cancelled.store(true, Ordering::SeqCst);
        self.apply_epoch.fetch_add(1, Ordering::SeqCst);
        std::mem::take(&mut self.pending_apply_workers)
    }

    /// Check if any workspace has an active animation.
    pub(crate) fn is_animating(&self) -> bool {
        self.workspaces.values().any(|w| w.is_animating())
    }

    /// Tick all active animations by the given delta time.
    /// Returns true if any animation is still running.
    pub(crate) fn tick_animations(&mut self, delta_ms: u64) -> bool {
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
    ///
    /// After [`APPLY_FAILURE_BREAKER_THRESHOLD`] consecutive failures, tiling is
    /// paused, animations stop, and window visibility is restored.
    pub(crate) fn apply_layout(&mut self) -> Result<()> {
        let result = self.apply_layout_once();
        match &result {
            Ok(()) => self.consecutive_apply_failures = 0,
            Err(e) => {
                self.consecutive_apply_failures = self.consecutive_apply_failures.saturating_add(1);
                if !self.paused
                    && self.consecutive_apply_failures >= APPLY_FAILURE_BREAKER_THRESHOLD
                {
                    self.trip_apply_failure_breaker(e);
                }
            }
        }
        result
    }

    /// Pause tiling after repeated apply failures and make every managed window visible.
    fn trip_apply_failure_breaker(&mut self, last_error: &anyhow::Error) {
        warn!(
            "Layout application failed {} times in a row (last error: {}); pausing tiling and restoring window visibility. Resume with toggle-pause once the cause is fixed.",
            self.consecutive_apply_failures, last_error
        );
        self.paused = true;
        self.consecutive_apply_failures = 0;
        for workspace in self.workspaces.values_mut() {
            workspace.cancel_animation();
        }
        let managed_window_ids = self.all_managed_window_ids();
        run_visibility_recovery_pass(&managed_window_ids, "apply-failure-breaker");
    }

    fn apply_layout_once(&mut self) -> Result<()> {
        let reaped_workers = self.reap_finished_pending_apply_workers();
        if reaped_workers > 0 {
            let managed_window_ids = self.all_managed_window_ids();
            run_visibility_recovery_pass(&managed_window_ids, "late-apply-worker");
        }

        if self.paused {
            return Ok(());
        }
        if self.apply_worker_cancelled.load(Ordering::SeqCst) {
            return Err(anyhow!(
                "Layout application skipped: shutdown/revert cleanup is in progress"
            ));
        }
        if !self.pending_apply_workers.is_empty() {
            return Err(anyhow!(
                "Layout application skipped: previous timed-out apply worker is still finishing"
            ));
        }
        self.applying_layout = true;
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
        self.arm_moved_or_resized_suppression(all_placements.iter().map(|p| p.window_id));

        let timeout = self.layout_apply_timeout;
        let platform_config = self.platform_config.clone();
        let apply_worker_cancelled = self.apply_worker_cancelled.clone();
        let apply_epoch_ref = self.apply_epoch.clone();
        let apply_epoch = apply_epoch_ref.fetch_add(1, Ordering::SeqCst) + 1;
        let apply_window_ids: Vec<u64> = all_placements.iter().map(|p| p.window_id).collect();
        #[cfg(test)]
        let injected_behavior = self.injected_apply_placements_behavior;
        #[cfg(test)]
        let late_worker_recovery_count = self.late_worker_recovery_count.clone();

        let (tx, rx) = std::sync::mpsc::channel::<Result<()>>();
        let spawn_result = std::thread::Builder::new()
            .name("openniri-apply-layout".to_string())
            .spawn(move || {
                let should_cancel = || {
                    apply_worker_cancelled.load(Ordering::SeqCst)
                        || apply_epoch_ref.load(Ordering::SeqCst) != apply_epoch
                };
                if should_cancel() {
                    let _ = tx.send(Ok(()));
                    return;
                }

                #[cfg(test)]
                if let Some(behavior) = injected_behavior {
                    let result = match behavior {
                        TestApplyPlacementsBehavior::SleepAndSucceed(delay) => {
                            std::thread::sleep(delay);
                            Ok(())
                        }
                        TestApplyPlacementsBehavior::SleepAndFail(delay) => {
                            std::thread::sleep(delay);
                            Err(anyhow!("injected apply_placements failure"))
                        }
                    };
                    if should_cancel() {
                        run_visibility_recovery_pass(
                            &apply_window_ids,
                            "apply-cancelled-late-worker",
                        );
                        #[cfg(test)]
                        late_worker_recovery_count.fetch_add(1, Ordering::SeqCst);
                        let _ = tx.send(Ok(()));
                        return;
                    }
                    let _ = tx.send(result);
                    return;
                }

                if should_cancel() {
                    let _ = tx.send(Ok(()));
                    return;
                }
                let result =
                    openniri_platform_win32::apply_placements(&all_placements, &platform_config)
                        .map_err(|e| anyhow!(e.to_string()));
                if should_cancel() {
                    run_visibility_recovery_pass(&apply_window_ids, "apply-cancelled-late-worker");
                    #[cfg(test)]
                    late_worker_recovery_count.fetch_add(1, Ordering::SeqCst);
                    let _ = tx.send(Ok(()));
                    return;
                }
                let _ = tx.send(result);
            });

        let worker_handle = match spawn_result {
            Ok(handle) => handle,
            Err(e) => {
                self.applying_layout = false;
                return Err(anyhow!("Failed to spawn layout worker thread: {}", e));
            }
        };

        let result = match rx.recv_timeout(timeout) {
            Ok(result) => {
                let _ = worker_handle.join();
                if result.is_err() {
                    self.moved_or_resized_suppression.clear();
                }
                result
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                self.paused = true;
                // Invalidate this apply epoch so late-starting workers bail before placement calls.
                self.apply_epoch.fetch_add(1, Ordering::SeqCst);
                self.pending_apply_workers.push(worker_handle);
                self.moved_or_resized_suppression.clear();
                let msg = layout_apply_timeout_message(timeout);
                warn!("{}", msg);
                let managed_window_ids = self.all_managed_window_ids();
                run_visibility_recovery_pass(&managed_window_ids, "apply-timeout");
                Err(anyhow!(msg))
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                let _ = worker_handle.join();
                self.moved_or_resized_suppression.clear();
                Err(anyhow!(
                    "Layout worker thread exited without returning a result"
                ))
            }
        };
        self.applying_layout = false;
        result
    }

    /// Set the OS foreground window to match the workspace's focused window.
    /// Also updates active window border colors if configured.
    pub(crate) fn sync_foreground_window(&mut self) {
        let focused_hwnd = self
            .focused_workspace()
            .and_then(|ws| ws.focused_visible_window());

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
                if let Ok(color) =
                    u32::from_str_radix(&self.config.appearance.active_border_color, 16)
                {
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

    /// Find which workspace contains a window.
    pub(crate) fn find_window_workspace(&self, window_id: u64) -> Option<MonitorId> {
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
    pub(crate) fn get_focused_column_rect(&self) -> Option<Rect> {
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

    /// Toggle paused state for tiling operations.
    ///
    /// When resuming, this immediately reapplies layout so windows snap back
    /// without waiting for another command/event. If resume reapply fails,
    /// paused state is restored to avoid claiming a healthy resumed mode.
    pub(crate) fn toggle_pause(&mut self, source: &str) -> Result<()> {
        let was_paused = self.paused;
        self.paused = !was_paused;
        info!(
            "Tiling {} via {}",
            if self.paused { "paused" } else { "resumed" },
            source
        );
        if !self.paused {
            if let Err(err) = self.apply_layout() {
                self.paused = was_paused;
                warn!(
                    "Resume apply failed via {}; restoring paused state: {}",
                    source, err
                );
                return Err(err);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recovery::join_with_timeout;
    use crate::test_support::*;

    #[test]
    fn reloaded_config_is_validated_and_keeps_the_cloaking_override() {
        use openniri_platform_win32::HideStrategy;

        for force_no_cloak in [false, true] {
            let mut state = paused_state(two_monitors());
            state.force_no_cloak = force_no_cloak;
            for requested_cloaking in [false, true, true] {
                let mut config = test_config();
                config.appearance.use_cloaking = requested_cloaking;
                config.layout.gap = -10;
                config.layout.outer_gap = -20;
                config.layout.min_column_width = 800;
                config.layout.max_column_width = 200;
                config.layout.default_column_width = 5000;
                state.apply_config(config);

                let uses_cloaking = requested_cloaking && !force_no_cloak;
                assert_eq!(state.config.appearance.use_cloaking, uses_cloaking);
                assert_eq!(
                    state.platform_config.hide_strategy == HideStrategy::Cloak,
                    uses_cloaking
                );
                assert_eq!(state.config.layout.min_column_width, 200);
                assert_eq!(state.config.layout.max_column_width, 800);
                for workspace in state.workspaces.values_mut() {
                    workspace
                        .insert_window(state.config_revision, None)
                        .unwrap();
                    let last = workspace.columns().last().unwrap();
                    assert_eq!(last.width(), 800);
                    assert_eq!(workspace.gap(), 0);
                    assert_eq!(workspace.outer_gap(), 0);
                }
            }
        }
    }

    fn snapshot(workspaces: &[(&str, f64)], focused: &str) -> StateSnapshot {
        StateSnapshot {
            saved_at: "test".to_string(),
            workspaces: workspaces
                .iter()
                .map(|(name, offset)| {
                    let mut workspace = Workspace::default();
                    workspace.set_scroll_offset(*offset);
                    WorkspaceSnapshot {
                        monitor_device_name: name.to_string(),
                        workspace,
                    }
                })
                .collect(),
            focused_monitor_name: focused.to_string(),
        }
    }

    #[test]
    fn test_state_snapshot_roundtrip_restores_scroll_and_focus() {
        let mut state = paused_state(two_monitors());
        let ws = state.workspaces.get_mut(&1).unwrap();
        for id in [100, 200, 300] {
            ws.insert_window(id, Some(800)).unwrap();
        }

        // Monitor 2 stays empty: its saved offset must not be clamped away.
        let saved = snapshot(
            &[("DISPLAY1", 500.0), ("DISPLAY2", 300.0), ("UNKNOWN", 100.0)],
            "DISPLAY2",
        );
        let json = serde_json::to_string_pretty(&saved).expect("serialize");
        let parsed: StateSnapshot = serde_json::from_str(&json).expect("deserialize");

        let restored = state.restore_state(&parsed);
        assert_eq!(restored, HashSet::from([1, 2]));
        assert_eq!(state.workspaces[&1].scroll_offset(), 500.0);
        assert_eq!(state.workspaces[&2].scroll_offset(), 300.0);
        assert_eq!(state.focused_monitor, 2);

        let restored = state.restore_state(&snapshot(&[("UNKNOWN", 100.0)], "UNKNOWN"));
        assert!(restored.is_empty(), "unknown monitors are skipped");
    }

    #[test]
    fn test_moved_or_resized_suppression_window_tracking() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        state.arm_moved_or_resized_suppression([100, 200]);
        assert!(
            state.should_suppress_moved_or_resized(100),
            "recently applied windows should be suppressed"
        );
        assert!(
            !state.should_suppress_moved_or_resized(300),
            "unrelated windows should not be suppressed"
        );

        state
            .moved_or_resized_suppression
            .insert(200, std::time::Instant::now() - Duration::from_millis(1));
        assert!(
            !state.should_suppress_moved_or_resized(200),
            "expired suppression entries should be ignored"
        );
    }

    // ========================================================================
    // apply_layout failure, timeout and late-worker handling
    // ========================================================================

    #[test]
    fn test_repeated_apply_failures_trip_breaker_and_pause_tiling() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        state.injected_apply_placements_behavior = Some(TestApplyPlacementsBehavior::SleepAndFail(
            Duration::from_millis(1),
        ));

        for attempt in 1..APPLY_FAILURE_BREAKER_THRESHOLD {
            assert!(state.apply_layout().is_err());
            assert!(!state.paused, "attempt {} should not pause yet", attempt);
            assert_eq!(state.consecutive_apply_failures, attempt);
        }

        assert!(state.apply_layout().is_err());
        assert!(state.paused, "breaker should pause tiling");
        assert_eq!(state.consecutive_apply_failures, 0);

        // Paused apply is a no-op that succeeds and does not count as a failure.
        assert!(state.apply_layout().is_ok());
        assert_eq!(state.consecutive_apply_failures, 0);
    }

    #[test]
    fn test_successful_apply_resets_failure_count() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        state.injected_apply_placements_behavior = Some(TestApplyPlacementsBehavior::SleepAndFail(
            Duration::from_millis(1),
        ));
        assert!(state.apply_layout().is_err());
        assert!(state.apply_layout().is_err());
        assert_eq!(state.consecutive_apply_failures, 2);

        state.injected_apply_placements_behavior = Some(
            TestApplyPlacementsBehavior::SleepAndSucceed(Duration::from_millis(1)),
        );
        assert!(state.apply_layout().is_ok());
        assert_eq!(state.consecutive_apply_failures, 0);
        assert!(!state.paused);
    }

    #[test]
    fn test_apply_layout_injected_failure_does_not_auto_pause() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        state.layout_apply_timeout = Duration::from_millis(50);
        state
            .moved_or_resized_suppression
            .insert(99, std::time::Instant::now() + Duration::from_secs(1));
        state.injected_apply_placements_behavior = Some(TestApplyPlacementsBehavior::SleepAndFail(
            Duration::from_millis(5),
        ));

        let err = state
            .apply_layout()
            .expect_err("injected placement failure should propagate");
        assert!(err
            .to_string()
            .contains("injected apply_placements failure"));
        assert!(
            !state.paused,
            "non-timeout placement failures should not auto-pause tiling"
        );
        assert!(
            !state.applying_layout,
            "applying_layout must be cleared after injected failure path"
        );
        assert!(
            state.moved_or_resized_suppression.is_empty(),
            "suppression entries must be cleared after failed apply"
        );
    }

    #[test]
    fn test_apply_layout_timeout_auto_pauses_tiling() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        state.layout_apply_timeout = Duration::from_millis(10);
        state
            .moved_or_resized_suppression
            .insert(42, std::time::Instant::now() + Duration::from_secs(1));
        state.injected_apply_placements_behavior = Some(
            TestApplyPlacementsBehavior::SleepAndSucceed(Duration::from_millis(40)),
        );

        let err = state
            .apply_layout()
            .expect_err("apply_layout should time out in injected test mode");

        let message = err.to_string();
        assert!(
            message.contains("timed out"),
            "timeout error should be actionable: {}",
            message
        );
        assert!(state.paused, "tiling should auto-pause after apply timeout");
        assert!(
            !state.applying_layout,
            "applying_layout must be cleared after timeout path"
        );
        assert!(
            state.moved_or_resized_suppression.is_empty(),
            "suppression entries must be cleared after timeout"
        );
    }

    #[test]
    fn test_apply_layout_timeout_worker_is_joined_during_shutdown_begin() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        state.layout_apply_timeout = Duration::from_millis(10);
        state.injected_apply_placements_behavior = Some(
            TestApplyPlacementsBehavior::SleepAndSucceed(Duration::from_millis(60)),
        );

        let _ = state
            .apply_layout()
            .expect_err("apply_layout should time out in injected test mode");
        assert_eq!(
            state.pending_apply_workers.len(),
            1,
            "timed-out apply worker should be tracked for shutdown join"
        );

        let workers = state.begin_shutdown_or_revert();
        assert!(
            state.apply_worker_cancelled.load(Ordering::SeqCst),
            "shutdown/revert should set cancellation flag"
        );
        assert_eq!(workers.len(), 1, "one timed-out worker should be returned");
        for handle in workers {
            let mut handle = Some(handle);
            assert!(
                join_with_timeout(&mut handle, Duration::from_millis(300)),
                "timed-out worker should exit after shutdown cancellation"
            );
        }
    }

    #[test]
    fn test_apply_layout_rejects_overlap_while_timed_out_worker_is_running() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        state.layout_apply_timeout = Duration::from_millis(10);
        state.injected_apply_placements_behavior = Some(
            TestApplyPlacementsBehavior::SleepAndSucceed(Duration::from_millis(500)),
        );

        let _ = state
            .apply_layout()
            .expect_err("first apply should time out in injected test mode");
        assert_eq!(state.pending_apply_workers.len(), 1);

        // Simulate manual resume happening before the timed-out worker exits.
        state.paused = false;
        let err = state
            .apply_layout()
            .expect_err("second apply must not overlap while prior worker is still running");
        assert!(
            err.to_string().contains("previous timed-out apply worker"),
            "expected overlap-prevention error, got: {}",
            err
        );

        std::thread::sleep(Duration::from_millis(700));
        let reaped = state.reap_finished_pending_apply_workers();
        assert_eq!(reaped, 1, "timed-out worker should eventually be reaped");
    }

    #[test]
    fn test_apply_layout_timeout_late_worker_triggers_recovery_pass() {
        let mut state = AppState::new_with_config(test_config(), test_monitors());
        state.layout_apply_timeout = Duration::from_millis(10);
        state.injected_apply_placements_behavior = Some(
            TestApplyPlacementsBehavior::SleepAndSucceed(Duration::from_millis(80)),
        );

        let _ = state
            .apply_layout()
            .expect_err("apply_layout should time out in injected test mode");
        assert_eq!(
            state.pending_apply_workers.len(),
            1,
            "timed-out apply worker should be tracked"
        );

        std::thread::sleep(Duration::from_millis(140));
        let reaped = state.reap_finished_pending_apply_workers();
        assert_eq!(reaped, 1, "timed-out worker should be reaped");
        assert_eq!(
            state.late_worker_recovery_count.load(Ordering::SeqCst),
            1,
            "cancelled late worker should trigger one final recovery pass"
        );
    }
}
