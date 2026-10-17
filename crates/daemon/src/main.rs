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

mod commands;
mod config;
mod hotkeys;
mod ipc_server;
mod recovery;
mod startup;
mod state;
#[cfg(test)]
mod test_support;
mod tray;
mod window_events;

use anyhow::Result;
use clap::Parser;
use config::Config;
use hotkeys::{setup_hotkeys, HotkeyState};
use ipc_server::{check_already_running, run_ipc_server};
use openniri_core_layout::Rect;
use openniri_ipc::{pipe_name_candidates, preferred_pipe_name, IpcCommand, IpcResponse};
use openniri_platform_win32::{
    enumerate_monitors, overlay::OverlayWindow, set_display_change_sender, set_dpi_awareness,
    spawn_event_hook_thread, spawn_gesture_thread, spawn_mouse_hook_thread,
    uncloak_all_visible_windows, GestureEvent, HotkeyEvent, MonitorId, MonitorInfo, WindowEvent,
};
use recovery::{
    install_panic_hook, join_with_timeout, run_shutdown_cleanup, shutdown_mode_for_command,
    ShutdownMode,
};
use startup::{init_logging, print_startup_banner, Args, StartupInfo};
use state::{
    AppState, FALLBACK_VIEWPORT_HEIGHT, FALLBACK_VIEWPORT_WIDTH, FALLBACK_WORK_AREA_HEIGHT,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::{debug, error, info, warn};

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
    init_logging(&config)?;

    // Validate and clamp config values
    let config_warnings = config.validate();
    for w in &config_warnings {
        warn!("Config: {} - {}", w.field, w.message);
    }

    // Install panic hook to uncloak all windows and write a crash report
    install_panic_hook();

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
}
