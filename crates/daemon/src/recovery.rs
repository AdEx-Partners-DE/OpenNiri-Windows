//! Shutdown cleanup, window visibility recovery, panic handling and
//! privileged-window helpers.

use crate::state::AppState;
use openniri_ipc::IpcCommand;
use openniri_platform_win32::{
    enumerate_windows, restore_windows_moved_offscreen, uncloak_all_managed_windows,
    uncloak_all_visible_windows,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Poll interval for cooperative timed thread joins.
const JOIN_WITH_TIMEOUT_POLL_INTERVAL: Duration = Duration::from_millis(10);
/// Retry count for shutdown visibility recovery when an apply worker fails to exit in time.
const SHUTDOWN_RECOVERY_RETRY_ATTEMPTS: usize = 3;
/// Delay between additional shutdown visibility recovery attempts.
const SHUTDOWN_RECOVERY_RETRY_DELAY: Duration = Duration::from_millis(250);
/// Final bounded wait per lingering apply worker before daemon exit.
const SHUTDOWN_FINAL_JOIN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShutdownMode {
    Graceful,
    PanicRevert,
}

impl ShutdownMode {
    fn should_save_state(self) -> bool {
        matches!(self, Self::Graceful)
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Graceful => "graceful",
            Self::PanicRevert => "panic_revert",
        }
    }
}

pub(crate) fn shutdown_mode_for_command(cmd: &IpcCommand) -> Option<ShutdownMode> {
    match cmd {
        IpcCommand::Stop => Some(ShutdownMode::Graceful),
        IpcCommand::PanicRevert => Some(ShutdownMode::PanicRevert),
        _ => None,
    }
}

/// Whether a window's process is more privileged than the daemon (see
/// [`openniri_platform_win32::is_process_more_privileged`]). Tests use fake
/// process ids, so the check is disabled there.
pub(crate) fn is_process_more_privileged_than_daemon(process_id: u32) -> bool {
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

pub(crate) fn log_skipped_privileged_window(title: &str) {
    info!(
        "Not managing window '{}': OpenNiri cannot read its process's privilege level or it runs with higher privileges (for example as administrator), so Windows would block moving it",
        title
    );
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

pub(crate) fn run_visibility_recovery_pass(managed_window_ids: &[u64], context_label: &str) {
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
pub(crate) async fn run_shutdown_cleanup(state: &Arc<Mutex<AppState>>, mode: ShutdownMode) {
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

/// Join a thread with a timeout. Returns true if the thread joined within the deadline,
/// false if it timed out. The join handle remains available on timeout so callers can retry
/// later without losing ownership.
pub(crate) fn join_with_timeout(
    handle: &mut Option<std::thread::JoinHandle<()>>,
    timeout: Duration,
) -> bool {
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

/// Install a panic hook that uncloaks all windows and writes a crash report.
pub(crate) fn install_panic_hook() {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use openniri_core_layout::Rect;

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
}
