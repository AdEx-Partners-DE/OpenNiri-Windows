//! Shared helpers for the daemon unit tests.

use crate::config::Config;
use crate::state::AppState;
use openniri_core_layout::Rect;
use openniri_platform_win32::{MonitorId, MonitorInfo};

pub(crate) fn test_config() -> Config {
    Config::default()
}

pub(crate) fn monitor(id: MonitorId, x: i32, is_primary: bool, name: &str) -> MonitorInfo {
    MonitorInfo {
        id,
        rect: Rect::new(x, 0, 1920, 1080),
        work_area: Rect::new(x, 0, 1920, 1040),
        is_primary,
        device_name: name.to_string(),
    }
}

pub(crate) fn test_monitors() -> Vec<MonitorInfo> {
    vec![monitor(1, 0, true, "DISPLAY1")]
}

pub(crate) fn two_monitors() -> Vec<MonitorInfo> {
    vec![
        monitor(1, 0, true, "DISPLAY1"),
        monitor(2, 1920, false, "DISPLAY2"),
    ]
}

/// State whose apply_layout() is a no-op, so tests never position real windows.
pub(crate) fn paused_state(monitors: Vec<MonitorInfo>) -> AppState {
    let mut state = AppState::new_with_config(test_config(), monitors);
    state.paused = true;
    state
}
