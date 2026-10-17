//! Test helpers shared by the module test suites.

use crate::monitors::{MonitorId, MonitorInfo};
use openniri_core_layout::{Rect, Visibility, WindowId, WindowPlacement};

/// Serialises tests that install or clear the global event and hotkey senders.
pub(crate) static GLOBAL_SENDER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn monitor(id: MonitorId, x: i32) -> MonitorInfo {
    MonitorInfo {
        id,
        rect: Rect::new(x, 0, 1920, 1080),
        work_area: Rect::new(x, 0, 1920, 1040),
        is_primary: id == 1,
        device_name: format!("DISPLAY{}", id),
    }
}

pub(crate) fn placement(window_id: WindowId, visibility: Visibility) -> WindowPlacement {
    WindowPlacement {
        window_id,
        rect: Rect::new(0, 0, 100, 100),
        visibility,
        column_index: 0,
    }
}
