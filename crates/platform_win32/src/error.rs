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
}
