//! Error type and small helpers shared by the platform modules.

use openniri_core_layout::WindowId;
use thiserror::Error;

/// Recover from a poisoned mutex, logging a warning.
///
/// When a thread panics while holding a mutex, the mutex becomes "poisoned".
/// This helper logs the event and recovers the inner data so the application
/// can continue operating.
pub(crate) fn recover_poisoned_mutex<T>(
    err: std::sync::PoisonError<std::sync::MutexGuard<'_, T>>,
) -> std::sync::MutexGuard<'_, T> {
    eprintln!("[openniri] WARNING: Mutex poisoned, recovering");
    err.into_inner()
}

pub(crate) fn combine_operation_failures(context: &str, failures: Vec<String>) -> Win32Error {
    debug_assert!(!failures.is_empty());
    Win32Error::SetPositionFailed(format!(
        "{} ({} failures): {}",
        context,
        failures.len(),
        failures.join("; ")
    ))
}

/// Whether an operation failure is a benign race where the window vanished.
///
/// These races are expected under concurrent window lifecycle churn and should
/// not fail the entire placement batch.
pub(crate) fn is_benign_window_race_error(error: &Win32Error) -> bool {
    matches!(error, Win32Error::WindowNotFound(window_id) if *window_id != 0)
}

/// Per-window failures of a batch operation.
///
/// Benign races (the window vanished) are counted and logged at debug level;
/// every other failure is logged and reported by [`FailureLog::into_result`].
#[derive(Debug, Default)]
pub(crate) struct FailureLog {
    failures: Vec<String>,
    benign_races: usize,
}

impl FailureLog {
    pub(crate) fn record(&mut self, operation: &str, window_id: WindowId, error: Win32Error) {
        if is_benign_window_race_error(&error) {
            self.benign_races += 1;
            tracing::debug!(
                "Ignoring benign race during {} for window {}: {}",
                operation,
                window_id,
                error
            );
        } else {
            tracing::warn!("Failed to {} window {}: {}", operation, window_id, error);
            self.failures
                .push(format!("{} {} failed: {}", operation, window_id, error));
        }
    }

    pub(crate) fn into_result(self, context: &str) -> Result<(), Win32Error> {
        if self.benign_races > 0 {
            tracing::debug!("{}: ignored {} benign race(s)", context, self.benign_races);
        }
        if self.failures.is_empty() {
            Ok(())
        } else {
            Err(combine_operation_failures(context, self.failures))
        }
    }
}

/// Errors that can occur during Win32 operations.
#[derive(Debug, Error)]
pub enum Win32Error {
    #[error("Failed to enumerate windows: {0}")]
    EnumerationFailed(String),

    #[error("Failed to enumerate monitors: {0}")]
    MonitorEnumerationFailed(String),

    #[error("Failed to set window position: {0}")]
    SetPositionFailed(String),

    #[error("Failed to cloak window: {0}")]
    CloakFailed(String),

    #[error("Failed to install event hook: {0}")]
    HookInstallFailed(String),

    #[error("Failed to register hotkey: {0}")]
    HotkeyRegistrationFailed(String),

    #[error("Window not found: {0}")]
    WindowNotFound(WindowId),

    #[error("Failed to set foreground window: {0}")]
    ForegroundFailed(String),

    #[error("Failed to close window: {0}")]
    CloseFailed(String),

    #[error("Failed to set window border color: {0}")]
    BorderColorFailed(String),

    #[error("Failed to create window: {0}")]
    WindowCreationFailed(String),

    #[error("Failed to start platform thread: {0}")]
    ThreadFailed(String),

    #[error("Failed to set up pipe security: {0}")]
    PipeSecurityFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_benign_window_race_error_only_for_nonzero_not_found() {
        assert!(is_benign_window_race_error(&Win32Error::WindowNotFound(
            123
        )));
        assert!(!is_benign_window_race_error(&Win32Error::WindowNotFound(0)));
        assert!(!is_benign_window_race_error(
            &Win32Error::SetPositionFailed("hard failure".to_string())
        ));
    }

    #[test]
    fn test_failure_log_reports_only_hard_failures() {
        let mut log = FailureLog::default();
        log.record("move", 7, Win32Error::WindowNotFound(7));
        assert!(
            log.into_result("pass").is_ok(),
            "a vanished window is benign"
        );

        let mut log = FailureLog::default();
        log.record("move", 7, Win32Error::WindowNotFound(7));
        log.record("cloak", 8, Win32Error::CloakFailed("denied".to_string()));
        let message = log.into_result("pass").unwrap_err().to_string();
        assert!(message.contains("1 failures"), "{message}");
        assert!(message.contains("cloak 8 failed"), "{message}");
    }
}
