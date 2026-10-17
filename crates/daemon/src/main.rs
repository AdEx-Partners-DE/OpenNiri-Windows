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

use anyhow::{anyhow, Result};
use clap::Parser;
use config::Config;
use openniri_core_layout::{Rect, Workspace};
use openniri_ipc::{
    pipe_name_candidates, preferred_pipe_name, IpcCommand, IpcResponse, MAX_IPC_MESSAGE_SIZE,
};
use openniri_platform_win32::{
    enumerate_monitors, enumerate_windows, find_monitor_for_rect, get_process_executable,
    monitor_to_left, monitor_to_right, overlay::OverlayWindow, parse_hotkey_string,
    register_hotkeys, restore_windows_moved_offscreen, set_display_change_sender,
    set_dpi_awareness, spawn_event_hook_thread, spawn_gesture_thread, spawn_mouse_hook_thread,
    uncloak_all_managed_windows, uncloak_all_visible_windows, GestureEvent, Hotkey, HotkeyEvent,
    HotkeyId, MonitorId, MonitorInfo, PlatformConfig, WindowEvent,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{PipeMode, ServerOptions};
use tokio::sync::{mpsc, oneshot, Mutex, Semaphore};
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::FmtSubscriber;

/// Command-line arguments for the daemon binary.
#[derive(Parser, Debug, Clone)]
#[command(name = "openniri", about = "OpenNiri tiling window manager daemon")]
pub struct Args {
    /// Disable global hotkey registration
    #[arg(long)]
    pub no_hotkeys: bool,
    /// Use MoveOffScreen instead of DWM cloaking
    #[arg(long)]
    pub no_cloak: bool,
    /// Safe mode: combines --no-hotkeys and --no-cloak
    #[arg(long)]
    pub safe_mode: bool,
}

impl Args {
    /// Returns true if hotkeys should be skipped (either --no-hotkeys or --safe-mode).
    pub fn skip_hotkeys(&self) -> bool {
        self.no_hotkeys || self.safe_mode
    }

    /// Returns true if cloaking should be disabled (either --no-cloak or --safe-mode).
    pub fn skip_cloak(&self) -> bool {
        self.no_cloak || self.safe_mode
    }
}

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
/// IPC responder timeout - daemon must answer within this period.
const IPC_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
/// Max time allowed for a single Win32 placement apply call.
const APPLY_LAYOUT_TIMEOUT: Duration = Duration::from_millis(1500);
/// Consecutive failed layout applications after which tiling pauses itself.
///
/// A failing apply repeats on every animation frame (about 60 per second); left
/// running it fights the user for window visibility and focus (INC-49).
const APPLY_FAILURE_BREAKER_THRESHOLD: u32 = 5;
/// How long after focus-follows-mouse the matching OS focus event is expected.
const MOUSE_FOCUS_EVENT_WINDOW: Duration = Duration::from_secs(1);
/// Poll interval for cooperative timed thread joins.
const JOIN_WITH_TIMEOUT_POLL_INTERVAL: Duration = Duration::from_millis(10);
/// Suppress MovedOrResized events briefly after placements are applied.
const MOVED_OR_RESIZED_SUPPRESSION_WINDOW: Duration = Duration::from_millis(250);
/// Retry count for shutdown visibility recovery when an apply worker fails to exit in time.
const SHUTDOWN_RECOVERY_RETRY_ATTEMPTS: usize = 3;
/// Delay between additional shutdown visibility recovery attempts.
const SHUTDOWN_RECOVERY_RETRY_DELAY: Duration = Duration::from_millis(250);
/// Final bounded wait per lingering apply worker before daemon exit.
const SHUTDOWN_FINAL_JOIN_TIMEOUT: Duration = Duration::from_secs(2);

/// Fallback viewport dimensions when no monitor is detected.
const FALLBACK_VIEWPORT_WIDTH: i32 = 1920;
const FALLBACK_VIEWPORT_HEIGHT: i32 = 1080;
const FALLBACK_WORK_AREA_HEIGHT: i32 = 1040;
const MIN_SET_WIDTH_FRACTION: f64 = 0.1;
const MAX_SET_WIDTH_FRACTION: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShutdownMode {
    Graceful,
    PanicRevert,
}

impl ShutdownMode {
    fn should_save_state(self) -> bool {
        matches!(self, Self::Graceful)
    }

    fn label(self) -> &'static str {
        match self {
            Self::Graceful => "graceful",
            Self::PanicRevert => "panic_revert",
        }
    }
}

fn shutdown_mode_for_command(cmd: &IpcCommand) -> Option<ShutdownMode> {
    match cmd {
        IpcCommand::Stop => Some(ShutdownMode::Graceful),
        IpcCommand::PanicRevert => Some(ShutdownMode::PanicRevert),
        _ => None,
    }
}

fn layout_apply_timeout_message(timeout: Duration) -> String {
    format!(
        "Layout application timed out after {} ms; tiling auto-paused to keep the daemon responsive. Resolve blocked Win32 placement, then use tray 'Pause/Resume Tiling' to resume. If desktop control degrades, run `openniri-cli panic-revert`.",
        timeout.as_millis()
    )
}

fn response_for_ipc_wait_failure(cmd: &IpcCommand, timed_out: bool) -> IpcResponse {
    if matches!(cmd, IpcCommand::Stop | IpcCommand::PanicRevert) {
        // Stop/panic_revert semantics are "shutdown initiated"; don't report as a hard failure
        // if the responder channel closes or cleanup outlives the client timeout.
        IpcResponse::Ok
    } else if timed_out {
        IpcResponse::error("Timed out waiting for daemon response")
    } else {
        IpcResponse::error("Failed to get response from daemon")
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
enum TestApplyPlacementsBehavior {
    SleepAndSucceed(Duration),
    SleepAndFail(Duration),
}

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
    /// Guard flag to suppress MovedOrResized events during apply_layout().
    applying_layout: bool,
    /// Per-window suppression deadline for MovedOrResized events after apply_layout().
    moved_or_resized_suppression: HashMap<u64, std::time::Instant>,
    /// Cooperative cancellation flag for placement workers during shutdown/revert.
    apply_worker_cancelled: Arc<AtomicBool>,
    /// Monotonic token to invalidate stale workers when shutdown starts.
    apply_epoch: Arc<AtomicU64>,
    /// Timed-out placement workers retained for join during shutdown/revert.
    pending_apply_workers: Vec<std::thread::JoinHandle<()>>,
    /// Max time allowed for Win32 placement calls before auto-pausing tiling.
    layout_apply_timeout: Duration,
    /// Failed layout applications since the last success.
    consecutive_apply_failures: u32,
    /// Window focused by focus-follows-mouse, and when. The OS focus event that
    /// follows must not scroll the strip: scrolling would put another window under
    /// the cursor, which would then take focus in turn.
    mouse_focus_pending: Option<(u64, std::time::Instant)>,
    /// Daemon start time for uptime reporting.
    start_time: std::time::Instant,
    /// Injected window info for testing. When set, `lookup_window_info()` returns
    /// entries from this map instead of calling `enumerate_windows()`.
    #[cfg(test)]
    injected_window_info: HashMap<u64, openniri_platform_win32::WindowInfo>,
    /// Optional test-only behavior override for placement application.
    #[cfg(test)]
    injected_apply_placements_behavior: Option<TestApplyPlacementsBehavior>,
    /// Number of late-worker recovery passes executed after cancellation.
    #[cfg(test)]
    late_worker_recovery_count: Arc<AtomicUsize>,
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
        info!(
            "Configuration applied to all {} workspaces",
            self.workspaces.len()
        );
    }

    /// Save current workspace state to disk.
    fn save_state(&self) -> Result<()> {
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
    /// This should be called AFTER windows are enumerated so that scroll offsets
    /// are not clamped against empty workspaces. Sets the scroll offset directly
    /// (bypassing clamping) to preserve the saved value.
    ///
    /// Returns the set of monitor IDs whose scroll offsets were successfully
    /// restored. The caller should skip `ensure_focused_visible()` for these
    /// monitors to avoid overwriting the restored offset.
    fn restore_state(&mut self, snapshot: &StateSnapshot) -> HashSet<MonitorId> {
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
    fn should_suppress_moved_or_resized(&mut self, hwnd: u64) -> bool {
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
    fn begin_shutdown_or_revert(&mut self) -> Vec<std::thread::JoinHandle<()>> {
        self.apply_worker_cancelled.store(true, Ordering::SeqCst);
        self.apply_epoch.fetch_add(1, Ordering::SeqCst);
        std::mem::take(&mut self.pending_apply_workers)
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
    ///
    /// After [`APPLY_FAILURE_BREAKER_THRESHOLD`] consecutive failures, tiling is
    /// paused, animations stop, and window visibility is restored.
    fn apply_layout(&mut self) -> Result<()> {
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
    fn sync_foreground_window(&mut self) {
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

    /// Enumerate windows and add them to the appropriate workspace based on position.
    fn enumerate_and_add_windows(&mut self) -> Result<usize> {
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

    /// Find which workspace contains a window.
    fn find_window_workspace(&self, window_id: u64) -> Option<MonitorId> {
        for (monitor_id, workspace) in &self.workspaces {
            if workspace.contains_window(window_id) {
                return Some(*monitor_id);
            }
        }
        None
    }

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

    /// Toggle paused state for tiling operations.
    ///
    /// When resuming, this immediately reapplies layout so windows snap back
    /// without waiting for another command/event. If resume reapply fails,
    /// paused state is restored to avoid claiming a healthy resumed mode.
    fn toggle_pause(&mut self, source: &str) -> Result<()> {
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

    /// Handle a window lifecycle event.
    fn handle_window_event(&mut self, event: WindowEvent) {
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
    /// Returns true if focus was applied, false if the window isn't managed.
    fn apply_focus_follows_mouse(&mut self, hwnd: u64) -> bool {
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

/// Hotkey registration result containing handle and mapping.
struct HotkeyState {
    /// Handle to unregister hotkeys on drop.
    handle: Option<openniri_platform_win32::HotkeyHandle>,
    /// Mapping of hotkey IDs to commands.
    mapping: HashMap<HotkeyId, IpcCommand>,
    /// Number of hotkeys that were requested for registration.
    requested_count: usize,
    /// Number of hotkeys the OS actually registered (may be less than
    /// `requested_count` if some conflicted with other applications).
    registered_count: usize,
}

/// Register hotkeys from config and return state.
///
/// This function is called both at startup and on config reload.
fn setup_hotkeys(config: &Config, event_tx: mpsc::Sender<DaemonEvent>) -> HotkeyState {
    let config_hotkeys = &config.hotkeys.bindings;

    // Build hotkey definitions and command mapping
    let mut hotkeys = Vec::new();
    let mut mapping = HashMap::new();
    let mut key_names: HashMap<HotkeyId, &str> = HashMap::new();
    let mut next_id: HotkeyId = 1;

    for (key_str, cmd_str) in config_hotkeys {
        if let Some((modifiers, vk)) = parse_hotkey_string(key_str) {
            if let Some(cmd) = config::parse_command(cmd_str) {
                hotkeys.push(Hotkey::new(next_id, modifiers, vk));
                key_names.insert(next_id, key_str.as_str());
                mapping.insert(next_id, cmd);
                debug!(
                    "Configured hotkey {}: {} -> {:?}",
                    next_id, key_str, cmd_str
                );
                next_id += 1;
            } else {
                warn!(
                    "Unknown command in hotkey config: {} -> {}",
                    key_str, cmd_str
                );
            }
        } else {
            warn!("Invalid hotkey string in config: {}", key_str);
        }
    }

    let requested_count = hotkeys.len();

    if hotkeys.is_empty() {
        info!("No hotkeys configured");
        return HotkeyState {
            handle: None,
            mapping,
            requested_count: 0,
            registered_count: 0,
        };
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
                }) {
                Ok(_) => {} // Thread is detached, we don't track it
                Err(e) => {
                    warn!("Failed to spawn hotkey-fwd thread: {}", e);
                }
            }

            report_unregistered_hotkeys(&handle, &mapping, &key_names);

            let registered_count = handle.registered_count();
            HotkeyState {
                handle: Some(handle),
                mapping,
                requested_count,
                registered_count,
            }
        }
        Err(e) => {
            warn!(
                "Failed to register hotkeys: {}. Global shortcuts disabled.",
                e
            );
            HotkeyState {
                handle: None,
                mapping,
                requested_count,
                registered_count: 0,
            }
        }
    }
}

/// Whether a window's process is more privileged than the daemon (see
/// [`openniri_platform_win32::is_process_more_privileged`]). Tests use fake
/// process ids, so the check is disabled there.
fn is_process_more_privileged_than_daemon(process_id: u32) -> bool {
    #[cfg(test)]
    {
        let _ = process_id;
        false
    }
    #[cfg(not(test))]
    {
        openniri_platform_win32::is_process_more_privileged(process_id)
    }
}

fn log_skipped_privileged_window(title: &str) {
    info!(
        "Not managing window '{}': OpenNiri cannot read its process's privilege level or it runs with higher privileges (for example as administrator), so Windows would block moving it",
        title
    );
}

/// Log each configured hotkey that could not be registered.
fn report_unregistered_hotkeys(
    handle: &openniri_platform_win32::HotkeyHandle,
    mapping: &HashMap<HotkeyId, IpcCommand>,
    key_names: &HashMap<HotkeyId, &str>,
) {
    let mut ids: Vec<HotkeyId> = mapping.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        if handle.is_registered(id) {
            continue;
        }
        let key = key_names.get(&id).copied().unwrap_or("?");
        if matches!(mapping.get(&id), Some(IpcCommand::PanicRevert)) {
            warn!(
                "Emergency hotkey {} (panic_revert) could not be registered; use `openniri-cli panic-revert` or the tray menu instead",
                key
            );
        } else {
            warn!(
                "Hotkey {} -> {:?} could not be registered (already used by Windows or another application)",
                key,
                mapping.get(&id)
            );
        }
    }
}

fn merged_cleanup_window_ids(
    managed_window_ids: &[u64],
    discovered_window_ids: &[u64],
) -> Vec<u64> {
    let mut merged = Vec::with_capacity(managed_window_ids.len() + discovered_window_ids.len());
    merged.extend_from_slice(managed_window_ids);
    merged.extend_from_slice(discovered_window_ids);
    merged.sort_unstable();
    merged.dedup();
    merged
}

fn run_visibility_recovery_pass(managed_window_ids: &[u64], context_label: &str) {
    let discovered_window_ids = match enumerate_windows() {
        Ok(windows) => windows.into_iter().map(|w| w.hwnd).collect::<Vec<_>>(),
        Err(e) => {
            warn!(
                "Failed to enumerate windows during {} recovery: {}",
                context_label, e
            );
            Vec::new()
        }
    };
    let recovery_window_ids = merged_cleanup_window_ids(managed_window_ids, &discovered_window_ids);

    match restore_windows_moved_offscreen(&recovery_window_ids) {
        Ok(restored) => {
            if restored > 0 {
                info!(
                    "Restored {} windows from MoveOffScreen sentinel positions",
                    restored
                );
            }
        }
        Err(e) => warn!("MoveOffScreen recovery failed: {}", e),
    }

    // Keep managed-window uncloak behavior.
    uncloak_all_managed_windows(managed_window_ids);
    // Safety net: also uncloak all visible windows on the desktop.
    uncloak_all_visible_windows();
}

/// Shared shutdown/recovery cleanup used by all daemon exit paths.
async fn run_shutdown_cleanup(state: &Arc<Mutex<AppState>>, mode: ShutdownMode) {
    info!("Running {} shutdown cleanup", mode.label());

    let (managed_window_ids, pending_apply_workers, apply_timeout) = {
        let mut state = state.lock().await;
        let pending_apply_workers = state.begin_shutdown_or_revert();
        if mode.should_save_state() {
            if let Err(e) = state.save_state() {
                warn!("Failed to save workspace state: {}", e);
            }
        }
        (
            state.all_managed_window_ids(),
            pending_apply_workers,
            state.layout_apply_timeout,
        )
    };

    let mut pending_workers = pending_apply_workers
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>();

    let mut timed_out_workers = 0usize;
    for worker in &mut pending_workers {
        if !join_with_timeout(worker, apply_timeout) {
            warn!(
                "Timed-out apply worker did not exit before {} cleanup; continuing with best effort",
                mode.label()
            );
            timed_out_workers += 1;
        }
    }
    pending_workers.retain(Option::is_some);

    run_visibility_recovery_pass(&managed_window_ids, mode.label());

    if !pending_workers.is_empty() {
        for attempt in 1..=SHUTDOWN_RECOVERY_RETRY_ATTEMPTS {
            tokio::time::sleep(SHUTDOWN_RECOVERY_RETRY_DELAY).await;
            for worker in &mut pending_workers {
                let _ = join_with_timeout(worker, SHUTDOWN_RECOVERY_RETRY_DELAY);
            }
            pending_workers.retain(Option::is_some);
            info!(
                "Running additional {} cleanup visibility recovery pass {}/{} after {} timed-out apply worker(s)",
                mode.label(),
                attempt,
                SHUTDOWN_RECOVERY_RETRY_ATTEMPTS,
                timed_out_workers.max(pending_workers.len())
            );
            run_visibility_recovery_pass(&managed_window_ids, mode.label());
            if pending_workers.is_empty() {
                break;
            }
        }
    }

    if !pending_workers.is_empty() {
        warn!(
            "{} timed-out apply worker(s) still running after {} cleanup retries; running final bounded joins before exit",
            pending_workers.len(),
            mode.label()
        );
        for worker in &mut pending_workers {
            let _ = join_with_timeout(worker, SHUTDOWN_FINAL_JOIN_TIMEOUT);
        }
        pending_workers.retain(Option::is_some);
        run_visibility_recovery_pass(&managed_window_ids, mode.label());
        if !pending_workers.is_empty() {
            warn!(
                "{} timed-out apply worker(s) still running after final {} bounded joins ({} ms each); exiting without detached recovery threads",
                pending_workers.len(),
                mode.label(),
                SHUTDOWN_FINAL_JOIN_TIMEOUT.as_millis()
            );
        }
    }
}

/// Run the IPC server, accepting connections and dispatching commands.
async fn run_ipc_server(event_tx: mpsc::Sender<DaemonEvent>) {
    let mut is_first_instance = true;
    let pipe_name = preferred_pipe_name();
    // Bound concurrent IPC handlers to avoid local task-exhaustion DoS.
    let connection_limit = Arc::new(Semaphore::new(32));
    // Only this user and SYSTEM may connect: the pipe accepts commands that move,
    // hide and close windows.
    let mut pipe_security =
        match openniri_platform_win32::pipe_security::PipeSecurityAttributes::current_user_only() {
            Ok(security) => Some(security),
            Err(e) => {
                warn!(
                    "Could not restrict the IPC pipe to the current user ({}); using the default ACL",
                    e
                );
                None
            }
        };

    loop {
        let permit = match connection_limit.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => {
                warn!("IPC connection limiter closed while accepting client");
                return;
            }
        };

        // Create a new pipe server instance
        let mut options = ServerOptions::new();
        options
            .first_pipe_instance(is_first_instance)
            .pipe_mode(PipeMode::Byte);
        let created = match pipe_security.as_mut() {
            // SAFETY: the pointer refers to SECURITY_ATTRIBUTES owned by
            // `pipe_security`, which outlives this call.
            Some(security) => unsafe {
                options.create_with_security_attributes_raw(&pipe_name, security.as_mut_ptr())
            },
            None => options.create(&pipe_name),
        };
        let server = match created {
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
                drop(permit);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }
        };

        debug!("Waiting for client connection on {}", pipe_name);

        // Wait for a client to connect
        if let Err(e) = server.connect().await {
            error!("Failed to accept client connection: {}", e);
            drop(permit);
            continue;
        }

        debug!("Client connected");

        // Handle this client
        let event_tx = event_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(server, event_tx).await {
                warn!("Client handler error: {}", e);
            }
            drop(permit);
        });
    }
}

/// Handle a single client connection.
async fn handle_client(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    event_tx: mpsc::Sender<DaemonEvent>,
) -> Result<()> {
    async fn write_ipc_response_line<W>(writer: &mut W, response: &IpcResponse) -> Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let mut response_json = match serde_json::to_string(response) {
            Ok(json) => json + "\n",
            Err(e) => {
                warn!("Failed to serialize IPC response: {}", e);
                "{\"status\":\"error\",\"message\":\"Internal serialization error\"}\n".to_string()
            }
        };

        if response_json.len() > MAX_IPC_MESSAGE_SIZE {
            warn!(
                "IPC response exceeded {} bytes; returning bounded error response instead",
                MAX_IPC_MESSAGE_SIZE
            );
            response_json = serde_json::to_string(&IpcResponse::error(
                "IPC response exceeded maximum size; narrow query scope and retry",
            ))
            .unwrap_or_else(|_| {
                "{\"status\":\"error\",\"message\":\"Internal serialization error\"}".to_string()
            });
            response_json.push('\n');
        }

        writer.write_all(response_json.as_bytes()).await?;
        Ok(())
    }

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

    if !line.ends_with('\n') {
        let msg = if bytes_read >= MAX_IPC_MESSAGE_SIZE {
            "Command too large or missing newline terminator"
        } else {
            "IPC command must be newline-terminated"
        };
        write_ipc_response_line(&mut writer, &IpcResponse::error(msg)).await?;
        return Ok(());
    }

    let line = line.trim_end_matches(['\r', '\n']);
    debug!("Received command: {}", line);

    // Parse the command
    let cmd: IpcCommand = match serde_json::from_str(line) {
        Ok(cmd) => cmd,
        Err(e) => {
            let response = IpcResponse::error(format!("Invalid command: {}", e));
            write_ipc_response_line(&mut writer, &response).await?;
            return Ok(());
        }
    };

    // Create a oneshot channel for the response
    let (resp_tx, resp_rx) = oneshot::channel();
    let response_cmd = cmd.clone();

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
        write_ipc_response_line(&mut writer, &response).await?;
        return Ok(());
    }

    // Wait for the response (bounded so clients don't hang forever).
    let response = match tokio::time::timeout(IPC_RESPONSE_TIMEOUT, resp_rx).await {
        Ok(Ok(resp)) => resp,
        Ok(Err(_)) => response_for_ipc_wait_failure(&response_cmd, false),
        Err(_) => response_for_ipc_wait_failure(&response_cmd, true),
    };

    // Send response back to client.
    write_ipc_response_line(&mut writer, &response).await?;

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

/// Join a thread with a timeout. Returns true if the thread joined within the deadline,
/// false if it timed out. The join handle remains available on timeout so callers can retry
/// later without losing ownership.
fn join_with_timeout(handle: &mut Option<std::thread::JoinHandle<()>>, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;

    loop {
        let Some(join_handle) = handle.as_ref() else {
            return true;
        };
        if join_handle.is_finished() {
            let join_handle = handle
                .take()
                .expect("join handle must exist when finishing timed join");
            let _ = join_handle.join();
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(JOIN_WITH_TIMEOUT_POLL_INTERVAL);
    }
}

/// Startup banner info for display after initialization.
pub struct StartupInfo {
    pub version: String,
    pub monitor_names: Vec<String>,
    pub window_count: usize,
    pub hotkeys_registered: usize,
    pub hotkeys_requested: usize,
    pub config_path: Option<String>,
    pub config_warnings: Vec<String>,
    pub log_path: String,
    pub safe_mode: bool,
    pub no_hotkeys: bool,
    pub no_cloak: bool,
}

/// Print a startup banner to stderr so users see immediate feedback.
/// Format the startup banner into a string (testable without capturing stderr).
pub fn format_startup_banner(info: &StartupInfo) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    writeln!(out).unwrap();
    writeln!(out, "OpenNiri Windows v{}", info.version).unwrap();
    if info.monitor_names.is_empty() {
        writeln!(out, "  Monitors: 0 (fallback mode)").unwrap();
    } else {
        writeln!(
            out,
            "  Monitors: {} ({})",
            info.monitor_names.len(),
            info.monitor_names.join(", ")
        )
        .unwrap();
    }
    writeln!(out, "  Windows:  {} managed", info.window_count).unwrap();
    if info.hotkeys_registered < info.hotkeys_requested {
        writeln!(
            out,
            "  Hotkeys:  {}/{} registered ({} failed)",
            info.hotkeys_registered,
            info.hotkeys_requested,
            info.hotkeys_requested - info.hotkeys_registered
        )
        .unwrap();
    } else {
        writeln!(out, "  Hotkeys:  {} registered", info.hotkeys_registered).unwrap();
    }
    if let Some(ref path) = info.config_path {
        writeln!(out, "  Config:   {}", path).unwrap();
    } else {
        writeln!(out, "  Config:   (default — no config file found)").unwrap();
    }
    for w in &info.config_warnings {
        writeln!(out, "  Warning:  {}", w).unwrap();
    }
    writeln!(out, "  Logs:     {}", info.log_path).unwrap();
    if info.safe_mode {
        writeln!(
            out,
            "  Mode:     SAFE MODE (hotkeys disabled, cloaking disabled)"
        )
        .unwrap();
    } else if info.no_hotkeys {
        writeln!(out, "  Mode:     hotkeys disabled").unwrap();
    } else if info.no_cloak {
        writeln!(out, "  Mode:     cloaking disabled").unwrap();
    } else {
        writeln!(out, "  Status:   Active").unwrap();
    }
    writeln!(out).unwrap();
    out
}

/// Print the startup banner to stderr.
pub fn print_startup_banner(info: &StartupInfo) {
    eprint!("{}", format_startup_banner(info));
}

/// Format a crash report from a panic.
fn format_crash_report(info: &std::panic::PanicHookInfo<'_>) -> String {
    use std::fmt::Write;
    let mut report = String::new();
    writeln!(report, "OpenNiri Crash Report").unwrap();
    writeln!(report, "=====================").unwrap();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    writeln!(report, "Timestamp (unix): {}", timestamp).unwrap();
    writeln!(report, "Version: {}", env!("CARGO_PKG_VERSION")).unwrap();
    writeln!(report).unwrap();

    // Panic message
    writeln!(report, "## Panic Info").unwrap();
    if let Some(msg) = info.payload().downcast_ref::<&str>() {
        writeln!(report, "Message: {}", msg).unwrap();
    } else if let Some(msg) = info.payload().downcast_ref::<String>() {
        writeln!(report, "Message: {}", msg).unwrap();
    } else {
        writeln!(report, "Message: (unknown payload type)").unwrap();
    }
    if let Some(location) = info.location() {
        writeln!(
            report,
            "Location: {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        )
        .unwrap();
    }
    writeln!(report).unwrap();

    // Backtrace
    writeln!(report, "## Backtrace").unwrap();
    writeln!(report, "{}", std::backtrace::Backtrace::force_capture()).unwrap();

    report
}

const ERROR_PIPE_BUSY: i32 = 231;
const ERROR_FILE_NOT_FOUND: i32 = 2;

fn pipe_probe_error_indicates_running(error: &std::io::Error) -> bool {
    match error.raw_os_error() {
        Some(ERROR_PIPE_BUSY) => true,
        Some(ERROR_FILE_NOT_FOUND) => false,
        _ if error.kind() == std::io::ErrorKind::NotFound => false,
        _ => true,
    }
}

fn pipe_probe_result_indicates_running<T>(probe_result: std::io::Result<T>) -> bool {
    match probe_result {
        Ok(_) => true,
        Err(error) => pipe_probe_error_indicates_running(&error),
    }
}

/// Check if another daemon instance is already running by probing the named pipe.
///
/// Returns `true` if the pipe exists (connected or busy). ERROR_PIPE_BUSY (231)
/// means another client is already connected — the daemon is still running.
async fn check_already_running() -> bool {
    for pipe_name in pipe_name_candidates() {
        let probe_result = tokio::net::windows::named_pipe::ClientOptions::new().open(&pipe_name);
        if let Err(error) = &probe_result {
            if pipe_probe_error_indicates_running(error)
                && error.raw_os_error() != Some(ERROR_PIPE_BUSY)
            {
                warn!(
                    "Named pipe probe for {} failed with non-NotFound error ({}); assuming daemon is already running to avoid duplicate instances",
                    pipe_name,
                    error
                );
            }
        }
        if pipe_probe_result_indicates_running(probe_result.map(|_| ())) {
            return true;
        }
    }
    false
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = Args::parse();

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

    // Apply safe-mode overrides to config
    if args.skip_cloak() {
        config.appearance.use_cloaking = false;
    }

    // Initialize logging with configured log level
    let log_level = match config.behavior.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO, // default fallback for invalid values
    };
    let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Validate and clamp config values
    let config_warnings = config.validate();
    for w in &config_warnings {
        warn!("Config: {} - {}", w.field, w.message);
    }

    // Install panic hook to uncloak all windows and write a crash report
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("[openniri] PANIC detected — emergency uncloaking all windows");
        uncloak_all_visible_windows();
        match enumerate_windows() {
            Ok(windows) => {
                let window_ids: Vec<u64> = windows.into_iter().map(|w| w.hwnd).collect();
                match restore_windows_moved_offscreen(&window_ids) {
                    Ok(restored) => {
                        if restored > 0 {
                            eprintln!(
                                "[openniri] Restored {} MoveOffScreen window(s) in panic recovery",
                                restored
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("[openniri] MoveOffScreen panic recovery failed: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "[openniri] Failed to enumerate windows for MoveOffScreen recovery: {}",
                    e
                );
            }
        }

        // Write crash report to temp dir
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let crash_path = std::env::temp_dir().join(format!("openniri-crash-{}.txt", ts));
        let report = format_crash_report(info);
        if let Err(e) = std::fs::write(&crash_path, &report) {
            eprintln!("[openniri] Failed to write crash report: {}", e);
        } else {
            eprintln!(
                "[openniri] Crash report written to: {}",
                crash_path.display()
            );
        }

        default_hook(info);
    }));

    info!("OpenNiri daemon starting...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Check if another instance is already running
    let ipc_pipe_names = pipe_name_candidates();
    if check_already_running().await {
        eprintln!("Error: Another openniri-daemon instance is already running.");
        eprintln!("Use 'openniri-cli status' to check the running instance.");
        error!(
            "Another openniri-daemon instance is already running (active pipe candidates: {})",
            ipc_pipe_names.join(", ")
        );
        std::process::exit(1);
    }

    info!(
        "Configuration loaded: gap={}, outer_gap={}, default_column_width={}, log_level={}",
        config.layout.gap,
        config.layout.outer_gap,
        config.layout.default_column_width,
        config.behavior.log_level
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
    let state = Arc::new(Mutex::new(AppState::new_with_config(
        config.clone(),
        monitors,
    )));

    // Windows a crashed or killed earlier run left cloaked would be skipped by
    // enumeration (it ignores cloaked windows), so bring them back first.
    let recovered = openniri_platform_win32::restore_journaled_cloaked_windows();
    if recovered > 0 {
        info!(
            "Recovered {} window(s) left cloaked by an earlier run",
            recovered
        );
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

        // Restore saved workspace state (after windows are enumerated so scroll
        // offsets aren't clamped against empty workspaces).
        let restored_monitors = if let Some(snapshot) = AppState::load_state() {
            let restored = state.restore_state(&snapshot);
            info!("Restored workspace state from previous session");
            restored
        } else {
            HashSet::new()
        };

        // Collect viewport widths first to avoid borrow issues
        let monitor_widths: HashMap<MonitorId, i32> = state
            .monitors
            .iter()
            .map(|(id, m)| (*id, m.work_area.width))
            .collect();

        // Center each workspace on its first column if it has windows.
        // Skip monitors whose scroll offset was restored from saved state.
        for (monitor_id, workspace) in state.workspaces.iter_mut() {
            if workspace.column_count() > 0 && !restored_monitors.contains(monitor_id) {
                let width = monitor_widths
                    .get(monitor_id)
                    .copied()
                    .unwrap_or(FALLBACK_VIEWPORT_WIDTH);
                workspace.ensure_focused_visible(width);
            }
        }
    }

    // Create event channel
    let (event_tx, mut event_rx) = mpsc::channel::<DaemonEvent>(100);

    // Collect forwarding thread handles for graceful shutdown
    let mut thread_handles: Vec<std::thread::JoinHandle<()>> = Vec::new();

    // Install WinEvent hooks for window lifecycle tracking (if enabled in config)
    let hook_thread = if config.behavior.track_focus_changes {
        match spawn_event_hook_thread() {
            Ok((handle, event_receiver)) => {
                info!("WinEvent hooks installed on dedicated message-loop thread");

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
                warn!(
                    "Failed to install WinEvent hooks: {}. Window tracking disabled.",
                    e
                );
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
    let mut hotkey_state = if args.skip_hotkeys() {
        info!("Hotkeys disabled by command-line flag");
        HotkeyState {
            handle: None,
            mapping: HashMap::new(),
            requested_count: 0,
            registered_count: 0,
        }
    } else {
        setup_hotkeys(&config, event_tx.clone())
    };

    // Install mouse hook for focus-follows-mouse (if enabled)
    let mouse_hook_thread = if config.behavior.focus_follows_mouse {
        let (mouse_tx, mouse_rx) = std::sync::mpsc::channel::<WindowEvent>();
        match spawn_mouse_hook_thread(mouse_tx) {
            Ok(handle) => {
                info!(
                    "Focus-follows-mouse enabled (delay: {}ms)",
                    config.behavior.focus_follows_mouse_delay_ms
                );

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
                warn!(
                    "Failed to install mouse hook: {}. Focus-follows-mouse disabled.",
                    e
                );
                None
            }
        }
    } else {
        info!("Focus-follows-mouse disabled by config (focus_follows_mouse = false)");
        None
    };

    // Register gesture detection (if enabled)
    let gesture_thread = if config.gestures.enabled {
        match spawn_gesture_thread() {
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
                warn!(
                    "Failed to register gestures: {}. Gesture support disabled.",
                    e
                );
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
                warn!(
                    "Failed to create snap hint overlay: {}. Snap hints disabled.",
                    e
                );
                None
            }
        }
    } else {
        info!("Snap hints disabled by config (snap_hints.enabled = false)");
        None
    };

    // Initialize system tray icon
    // Create an intermediate sync channel that bridges tray events to the async event loop
    let tray_manager = {
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

    info!("IPC server listening on {}", preferred_pipe_name());

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

    // Print startup banner for immediate user feedback
    {
        let state = state.lock().await;
        let monitor_names: Vec<String> = state
            .monitors
            .values()
            .map(|m| m.device_name.clone())
            .collect();
        let window_count: usize = state.workspaces.values().map(|w| w.window_count()).sum();
        let config_path = config::config_paths()
            .into_iter()
            .find(|p| p.exists())
            .map(|p| p.display().to_string());
        let log_path = std::env::temp_dir()
            .join("openniri-daemon.log")
            .display()
            .to_string();
        print_startup_banner(&StartupInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            monitor_names,
            window_count,
            hotkeys_registered: hotkey_state.registered_count,
            hotkeys_requested: hotkey_state.requested_count,
            config_path,
            config_warnings: config_warnings
                .iter()
                .map(|w| format!("{}: {}", w.field, w.message))
                .collect(),
            log_path,
            safe_mode: args.safe_mode,
            no_hotkeys: args.skip_hotkeys(),
            no_cloak: args.skip_cloak(),
        });
    }

    // Tile the windows found at startup instead of waiting for the first command.
    {
        let mut state = state.lock().await;
        match state.apply_layout() {
            Ok(()) => info!("Initial layout applied"),
            Err(e) => warn!("Initial layout failed: {}", e),
        }
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
            let mut interval =
                tokio::time::interval(std::time::Duration::from_millis(ANIMATION_TICK_MS));
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
                if let Some(mode) = shutdown_mode_for_command(&cmd) {
                    if mode == ShutdownMode::PanicRevert {
                        warn!("IPC: panic_revert requested");
                    } else {
                        info!("IPC: stop requested");
                    }
                    if responder.send(IpcResponse::Ok).is_err() {
                        debug!(
                            "Client disconnected before receiving {} response",
                            mode.label()
                        );
                    }
                    run_shutdown_cleanup(&state, mode).await;
                    break;
                }

                let is_reload = matches!(&cmd, IpcCommand::Reload);
                let is_resize = matches!(&cmd, IpcCommand::Resize { .. });
                let is_toggle_pause = matches!(&cmd, IpcCommand::TogglePause);

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

                if is_toggle_pause {
                    let state = state.lock().await;
                    if let Some(ref mgr) = tray_manager {
                        mgr.update_pause_text(state.paused);
                        let wc = state.all_managed_window_ids().len();
                        let mc = state.monitors.len();
                        mgr.update_tooltip(
                            wc,
                            mc,
                            state.paused,
                            Some((hotkey_state.registered_count, hotkey_state.requested_count)),
                        );
                    }
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
                            tokio::time::sleep(std::time::Duration::from_millis(duration as u64))
                                .await;
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
                            tokio::time::sleep(std::time::Duration::from_millis(delay as u64))
                                .await;
                            let _ = focus_tx
                                .send(DaemonEvent::FocusFollowsMouse { window_id: hwnd })
                                .await;
                        }));
                    }
                } else {
                    let mut state = state.lock().await;
                    state.handle_window_event(win_event);
                    // Update tray tooltip with current state
                    if let Some(ref mgr) = tray_manager {
                        let wc = state.all_managed_window_ids().len();
                        let mc = state.monitors.len();
                        mgr.update_tooltip(
                            wc,
                            mc,
                            state.paused,
                            Some((hotkey_state.registered_count, hotkey_state.requested_count)),
                        );
                    }
                }
            }
            DaemonEvent::Hotkey(hotkey_event) => {
                let mut requested_shutdown: Option<ShutdownMode> = None;
                let (should_animate, is_resize, column_rect, hint_duration) =
                    if let Some(cmd) = hotkey_state.mapping.get(&hotkey_event.id).cloned() {
                        debug!("Hotkey {} triggered, executing {:?}", hotkey_event.id, cmd);
                        if let Some(mode) = shutdown_mode_for_command(&cmd) {
                            requested_shutdown = Some(mode);
                            (false, false, None, 200)
                        } else {
                            let is_resize = matches!(cmd, IpcCommand::Resize { .. });
                            let mut state = state.lock().await;
                            let response = state.handle_command(cmd);
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
                        }
                    } else {
                        warn!("Unknown hotkey ID: {}", hotkey_event.id);
                        (false, false, None, 200)
                    };

                if let Some(mode) = requested_shutdown {
                    warn!(
                        "Hotkey {} requested {}; running shutdown cleanup",
                        hotkey_event.id,
                        mode.label()
                    );
                    run_shutdown_cleanup(&state, mode).await;
                    break;
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
                            tokio::time::sleep(std::time::Duration::from_millis(duration as u64))
                                .await;
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
                    if let Some(mode) = shutdown_mode_for_command(&cmd) {
                        warn!(
                            "Gesture {:?} requested {}; running shutdown cleanup",
                            gesture_event,
                            mode.label()
                        );
                        run_shutdown_cleanup(&state, mode).await;
                        break;
                    }
                    let should_animate = {
                        let mut state = state.lock().await;
                        let response = state.handle_command(cmd);
                        if let IpcResponse::Error { message } = response {
                            warn!("Gesture command failed: {}", message);
                        }
                        state.is_animating()
                    };

                    // Start animation timer if needed
                    if should_animate
                        && !animation_running.load(std::sync::atomic::Ordering::SeqCst)
                    {
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
                        if let Err(e) = state.toggle_pause("tray toggle") {
                            warn!("Tray toggle pause failed: {}", e);
                        }
                        if let Some(ref mgr) = tray_manager {
                            mgr.update_pause_text(state.paused);
                            let wc = state.all_managed_window_ids().len();
                            let mc = state.monitors.len();
                            mgr.update_tooltip(
                                wc,
                                mc,
                                state.paused,
                                Some((hotkey_state.registered_count, hotkey_state.requested_count)),
                            );
                        }
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
                    tray::TrayEvent::EmergencyUncloakAll => {
                        warn!("Tray: Emergency uncloak requested");
                        uncloak_all_visible_windows();
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
                    if !state.config.behavior.focus_follows_mouse {
                        false
                    } else {
                        let applied = state.apply_focus_follows_mouse(window_id);
                        if applied {
                            state.is_animating()
                        } else {
                            false
                        }
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
                run_shutdown_cleanup(&state, ShutdownMode::Graceful).await;
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

    // Unhook first: removing the hooks drops the global senders, and dropping the
    // tray stops its menu thread, so those forwarding threads see a closed channel.
    // Dropping the receiver unblocks any forwarder waiting on a full channel.
    drop(hook_thread);
    drop(mouse_hook_thread);
    drop(gesture_thread);
    drop(tray_manager);
    drop(hotkey_state);
    openniri_platform_win32::clear_display_change_sender();
    drop(event_rx);

    // Join forwarding threads with timeout for graceful shutdown
    info!("Waiting for forwarding threads to exit...");
    let shutdown_deadline = std::time::Instant::now() + Duration::from_secs(5);
    for handle in thread_handles {
        let remaining = shutdown_deadline.saturating_duration_since(std::time::Instant::now());
        let per_thread = remaining.min(Duration::from_secs(3));
        let mut handle = Some(handle);
        if !join_with_timeout(&mut handle, per_thread) {
            warn!("A forwarding thread did not exit within timeout, continuing shutdown");
        }
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

    fn monitor(id: MonitorId, x: i32, is_primary: bool, name: &str) -> MonitorInfo {
        MonitorInfo {
            id,
            rect: Rect::new(x, 0, 1920, 1080),
            work_area: Rect::new(x, 0, 1920, 1040),
            is_primary,
            device_name: name.to_string(),
        }
    }

    fn test_monitors() -> Vec<MonitorInfo> {
        vec![monitor(1, 0, true, "DISPLAY1")]
    }

    fn two_monitors() -> Vec<MonitorInfo> {
        vec![
            monitor(1, 0, true, "DISPLAY1"),
            monitor(2, 1920, false, "DISPLAY2"),
        ]
    }

    /// State whose apply_layout() is a no-op, so tests never position real windows.
    fn paused_state(monitors: Vec<MonitorInfo>) -> AppState {
        let mut state = AppState::new_with_config(test_config(), monitors);
        state.paused = true;
        state
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
    fn test_ipc_wait_failure_responses_and_shutdown_mapping() {
        // Stop/PanicRevert mean "shutdown initiated": a closed responder is not a failure.
        for timed_out in [true, false] {
            assert_eq!(
                response_for_ipc_wait_failure(&IpcCommand::Stop, timed_out),
                IpcResponse::Ok
            );
            assert_eq!(
                response_for_ipc_wait_failure(&IpcCommand::PanicRevert, timed_out),
                IpcResponse::Ok
            );
        }
        assert!(matches!(
            response_for_ipc_wait_failure(&IpcCommand::FocusLeft, true),
            IpcResponse::Error { message } if message.contains("Timed out")
        ));
        assert!(matches!(
            response_for_ipc_wait_failure(&IpcCommand::FocusLeft, false),
            IpcResponse::Error { message } if message.contains("Failed to get response")
        ));

        assert_eq!(
            shutdown_mode_for_command(&IpcCommand::Stop),
            Some(ShutdownMode::Graceful)
        );
        assert_eq!(
            shutdown_mode_for_command(&IpcCommand::PanicRevert),
            Some(ShutdownMode::PanicRevert)
        );
        assert_eq!(shutdown_mode_for_command(&IpcCommand::FocusLeft), None);
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

        assert!(state.apply_focus_follows_mouse(100));
        assert_eq!(state.previous_focused_hwnd, Some(100));
        assert_eq!(
            state.focused_workspace().unwrap().focused_window(),
            Some(100)
        );

        assert!(state.apply_focus_follows_mouse(500));
        assert_eq!(state.previous_focused_hwnd, Some(500));
        assert_eq!(
            state.focused_workspace().unwrap().focused_window(),
            Some(100),
            "floating focus-follows-mouse should not mutate tiled focus"
        );
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

    // ========================================================================
    // Startup, shutdown and singleton
    // ========================================================================

    #[test]
    fn test_args_flags_and_safe_mode() {
        for (argv, skip_hotkeys, skip_cloak) in [
            (&["openniri"][..], false, false),
            (&["openniri", "--no-hotkeys"][..], true, false),
            (&["openniri", "--no-cloak"][..], false, true),
            (&["openniri", "--safe-mode"][..], true, true),
        ] {
            let args = Args::try_parse_from(argv).unwrap();
            assert_eq!(args.skip_hotkeys(), skip_hotkeys, "{:?}", argv);
            assert_eq!(args.skip_cloak(), skip_cloak, "{:?}", argv);
        }
    }

    #[test]
    fn test_startup_banner_reports_failed_hotkeys_and_config_warnings() {
        let mut info = StartupInfo {
            version: "0.1.0".to_string(),
            monitor_names: vec!["DISPLAY1".to_string(), "DISPLAY2".to_string()],
            window_count: 14,
            hotkeys_registered: 7,
            hotkeys_requested: 10,
            config_path: None,
            config_warnings: vec!["layout.gap: Negative gap (-5) clamped to 0".to_string()],
            log_path: "openniri-daemon.log".to_string(),
            safe_mode: false,
            no_hotkeys: false,
            no_cloak: false,
        };
        let banner = format_startup_banner(&info);
        assert!(banner.contains("7/10 registered (3 failed)"));
        assert!(banner.contains("Warning:  layout.gap"));

        info.hotkeys_registered = 10;
        info.config_warnings.clear();
        let banner = format_startup_banner(&info);
        assert!(!banner.contains("failed"));
        assert!(!banner.contains("Warning:"));
    }

    #[test]
    fn test_spawn_forwarding_thread_forwards_then_exits_on_close() {
        let (tx, rx) = std::sync::mpsc::channel::<u32>();
        let (async_tx, mut async_rx) = mpsc::channel::<DaemonEvent>(10);

        let handle =
            spawn_forwarding_thread("test", rx, async_tx, |_n| DaemonEvent::AnimationTick).unwrap();

        tx.send(42).unwrap();
        drop(tx);

        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(rt.block_on(async { async_rx.recv().await }).is_some());
        handle
            .join()
            .expect("thread should exit once the sender closes");
    }

    #[test]
    fn test_join_with_timeout_hanging_thread() {
        let mut handle = Some(std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(60));
        }));
        let result = join_with_timeout(&mut handle, Duration::from_millis(100));
        assert!(
            !result,
            "Should return false when thread doesn't join in time"
        );
        assert!(
            handle.is_some(),
            "timed-out join should retain ownership for later retry"
        );
    }

    #[test]
    fn test_shutdown_cleanup_covers_all_managed_and_discovered_windows() {
        let mut state = paused_state(two_monitors());
        let ws = state.workspaces.get_mut(&1).unwrap();
        ws.insert_window(30, Some(800)).unwrap();
        ws.add_floating(10, Rect::new(0, 0, 400, 300)).unwrap();
        state
            .workspaces
            .get_mut(&2)
            .unwrap()
            .insert_window(20, Some(800))
            .unwrap();

        let mut managed = state.all_managed_window_ids();
        managed.sort_unstable();
        assert_eq!(
            managed,
            vec![10, 20, 30],
            "tiled + floating on every monitor"
        );

        let discovered = vec![20, 40, 10, 50];
        assert_eq!(
            merged_cleanup_window_ids(&managed, &discovered),
            vec![10, 20, 30, 40, 50]
        );
    }

    #[test]
    fn test_check_already_running_with_isolated_pipe() {
        // Error classification: busy/denied mean another daemon owns the pipe.
        let busy = std::io::Error::from_raw_os_error(ERROR_PIPE_BUSY);
        assert!(pipe_probe_error_indicates_running(&busy));
        let access_denied = std::io::Error::from_raw_os_error(5); // ERROR_ACCESS_DENIED
        assert!(pipe_probe_error_indicates_running(&access_denied));
        let not_found = std::io::Error::from_raw_os_error(ERROR_FILE_NOT_FOUND);
        assert!(!pipe_probe_error_indicates_running(&not_found));
        let not_found_kind = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        assert!(!pipe_probe_error_indicates_running(&not_found_kind));

        // Real probe against an isolated pipe name, independent of a running daemon.
        let pipe_name = format!(r"\\.\pipe\openniri-test-singleton-{}", std::process::id());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();
        let result = rt.block_on(async {
            pipe_probe_result_indicates_running(
                tokio::net::windows::named_pipe::ClientOptions::new()
                    .open(&pipe_name)
                    .map(|_| ()),
            )
        });
        assert!(
            !result,
            "No pipe server exists, so connect should fail (no daemon)"
        );
    }
}
