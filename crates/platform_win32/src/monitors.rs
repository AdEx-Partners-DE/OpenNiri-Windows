//! Monitor enumeration, hit-testing, left/right navigation and DPI awareness.

use crate::error::Win32Error;
use openniri_core_layout::Rect;
use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};

/// Unique identifier for a monitor (derived from HMONITOR handle).
pub type MonitorId = isize;

/// Information about a display monitor.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    /// Unique monitor identifier.
    pub id: MonitorId,
    /// Full monitor rectangle (entire display area).
    pub rect: Rect,
    /// Work area (excludes taskbar and other docked windows).
    pub work_area: Rect,
    /// Whether this is the primary monitor.
    pub is_primary: bool,
    /// Device name (e.g., `\\.\DISPLAY1`).
    pub device_name: String,
}

impl MonitorInfo {
    /// Check if a point is within this monitor's bounds.
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.rect.x
            && x < self.rect.x + self.rect.width
            && y >= self.rect.y
            && y < self.rect.y + self.rect.height
    }

    /// Check if a rectangle's center is within this monitor's bounds.
    pub fn contains_rect_center(&self, rect: &Rect) -> bool {
        let center_x = rect.x + rect.width / 2;
        let center_y = rect.y + rect.height / 2;
        self.contains_point(center_x, center_y)
    }
}

/// Get the primary monitor's information.
///
/// Returns the work area (excluding taskbar) which is suitable for window positioning.
pub fn get_primary_monitor() -> Result<MonitorInfo, Win32Error> {
    let monitors = enumerate_monitors()?;

    monitors
        .into_iter()
        .find(|m| m.is_primary)
        .ok_or_else(|| Win32Error::MonitorEnumerationFailed("No primary monitor found".to_string()))
}

/// Find which monitor contains the center of a given rectangle.
///
/// Returns the monitor info if found, or None if no monitor contains the point.
/// Falls back to primary monitor if no exact match.
pub fn find_monitor_for_rect<'a>(
    monitors: &'a [MonitorInfo],
    rect: &Rect,
) -> Option<&'a MonitorInfo> {
    // First, try to find a monitor that contains the rect's center
    let center_x = rect.x + rect.width / 2;
    let center_y = rect.y + rect.height / 2;

    monitors
        .iter()
        .find(|m| m.contains_point(center_x, center_y))
        .or_else(|| monitors.iter().find(|m| m.is_primary))
}

/// Find a monitor by its ID.
pub fn find_monitor_by_id(monitors: &[MonitorInfo], id: MonitorId) -> Option<&MonitorInfo> {
    monitors.iter().find(|m| m.id == id)
}

/// Get monitors sorted by position (left to right, then top to bottom).
pub fn monitors_by_position(monitors: &[MonitorInfo]) -> Vec<&MonitorInfo> {
    let mut sorted: Vec<_> = monitors.iter().collect();
    sorted.sort_by(|a, b| {
        // Sort by x first, then by y
        a.rect.x.cmp(&b.rect.x).then(a.rect.y.cmp(&b.rect.y))
    });
    sorted
}

/// Find the monitor to the left of the given monitor.
pub fn monitor_to_left(monitors: &[MonitorInfo], current_id: MonitorId) -> Option<&MonitorInfo> {
    let sorted = monitors_by_position(monitors);
    let current_idx = sorted.iter().position(|m| m.id == current_id)?;
    if current_idx > 0 {
        Some(sorted[current_idx - 1])
    } else {
        None
    }
}

/// Find the monitor to the right of the given monitor.
pub fn monitor_to_right(monitors: &[MonitorInfo], current_id: MonitorId) -> Option<&MonitorInfo> {
    let sorted = monitors_by_position(monitors);
    let current_idx = sorted.iter().position(|m| m.id == current_id)?;
    if current_idx + 1 < sorted.len() {
        Some(sorted[current_idx + 1])
    } else {
        None
    }
}

/// Enumerate all connected monitors.
///
/// Returns information about each display including work area (usable space
/// excluding taskbar and docked windows).
pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>, Win32Error> {
    let mut monitors: Vec<MonitorInfo> = Vec::new();

    unsafe {
        let monitors_ptr = &mut monitors as *mut Vec<MonitorInfo>;

        let result = EnumDisplayMonitors(
            None, // HDC - None to enumerate all monitors
            None, // lprcClip - None to not clip
            Some(enum_monitors_callback),
            LPARAM(monitors_ptr as isize),
        );

        if !result.as_bool() {
            return Err(Win32Error::MonitorEnumerationFailed(
                "EnumDisplayMonitors failed".to_string(),
            ));
        }
    }

    if monitors.is_empty() {
        return Err(Win32Error::MonitorEnumerationFailed(
            "No monitors found".to_string(),
        ));
    }

    tracing::debug!("Enumerated {} monitors", monitors.len());
    Ok(monitors)
}

/// Callback for EnumDisplayMonitors that collects monitor info.
unsafe extern "system" fn enum_monitors_callback(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _lprc_clip: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = &mut *(lparam.0 as *mut Vec<MonitorInfo>);

    // Initialize MONITORINFOEXW with correct size
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    if GetMonitorInfoW(hmonitor, &mut info as *mut MONITORINFOEXW as *mut _).as_bool() {
        let mon_rect = info.monitorInfo.rcMonitor;
        let work_rect = info.monitorInfo.rcWork;

        // Convert device name from wide string
        let device_name_len = info
            .szDevice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(info.szDevice.len());
        let device_name = String::from_utf16_lossy(&info.szDevice[..device_name_len]);

        monitors.push(MonitorInfo {
            id: hmonitor.0 as MonitorId,
            rect: Rect::new(
                mon_rect.left,
                mon_rect.top,
                mon_rect.right - mon_rect.left,
                mon_rect.bottom - mon_rect.top,
            ),
            work_area: Rect::new(
                work_rect.left,
                work_rect.top,
                work_rect.right - work_rect.left,
                work_rect.bottom - work_rect.top,
            ),
            // MONITORINFOF_PRIMARY = 1
            is_primary: info.monitorInfo.dwFlags & 1 != 0,
            device_name,
        });

        TRUE
    } else {
        // Continue enumeration even if one monitor fails
        TRUE
    }
}

/// Set the process DPI awareness to Per-Monitor Aware V2.
///
/// This must be called as early as possible in `main()`, before any
/// window or GDI operations. Returns `true` if the call succeeded.
pub fn set_dpi_awareness() -> bool {
    unsafe {
        use windows::Win32::UI::HiDpi::{
            SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        };
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::monitor;

    #[test]
    fn test_monitor_hit_testing() {
        let primary = monitor(1, 0);
        assert!(primary.contains_point(0, 0));
        assert!(primary.contains_point(1919, 540));
        assert!(
            !primary.contains_point(1920, 540),
            "right edge is exclusive"
        );
        assert!(!primary.contains_point(-1, 0));

        // Mostly off-screen, but the center is inside.
        assert!(primary.contains_rect_center(&Rect::new(-300, 100, 800, 600)));
        assert!(!primary.contains_rect_center(&Rect::new(1800, 100, 800, 600)));

        let monitors = vec![monitor(1, 0), monitor(2, 1920)];
        let on = |x: i32| {
            find_monitor_for_rect(&monitors, &Rect::new(x, 100, 800, 600))
                .unwrap()
                .id
        };
        assert_eq!(on(100), 1);
        assert_eq!(on(2000), 2);
    }

    #[test]
    fn test_monitor_left_right_navigation() {
        // Deliberately not in left-to-right order.
        let monitors = vec![monitor(2, 1920), monitor(1, 0)];
        let sorted: Vec<_> = monitors_by_position(&monitors)
            .iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(sorted, vec![1, 2]);

        assert_eq!(monitor_to_right(&monitors, 1).unwrap().id, 2);
        assert_eq!(monitor_to_left(&monitors, 2).unwrap().id, 1);
        assert!(monitor_to_left(&monitors, 1).is_none());
        assert!(monitor_to_right(&monitors, 2).is_none());
    }
}
