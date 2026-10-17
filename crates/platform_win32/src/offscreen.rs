//! MoveOffScreen sentinel handling, restoring parked windows and bringing windows
//! that ended up outside every monitor back into view.

use crate::error::{combine_operation_failures, is_benign_window_race_error, Win32Error};
use crate::monitors::{enumerate_monitors, get_primary_monitor, MonitorInfo};
use crate::window::window_id_to_hwnd;
use openniri_core_layout::{Rect, WindowId};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT, TRUE};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowRect, IsIconic, IsWindow, SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER,
};

/// Sentinel coordinate used by MoveOffScreen strategy.
///
/// Windows clamps window positions to the 16-bit range, so a window moved to
/// -100000 lands at -32768. The old -100000 sentinel was therefore never found
/// again and parked windows stayed lost after stop or emergency recovery.
/// -30000 is representable and distinct from the minimized position (-32000).
pub const MOVE_OFFSCREEN_SENTINEL_COORD: i32 = -30_000;

/// Coordinates at or below this on both axes count as parked off-screen. This also
/// covers windows parked by older versions, which ended up at -32768.
const MOVE_OFFSCREEN_DETECTION_THRESHOLD: i32 = -29_000;

/// Position Windows gives minimized windows; never treat it as parked.
const MINIMIZED_WINDOW_COORD: i32 = -32_000;

/// Check whether coordinates indicate MoveOffScreen sentinel placement.
pub fn is_move_offscreen_sentinel_position(x: i32, y: i32) -> bool {
    x <= MOVE_OFFSCREEN_DETECTION_THRESHOLD
        && y <= MOVE_OFFSCREEN_DETECTION_THRESHOLD
        && !(x == MINIMIZED_WINDOW_COORD && y == MINIMIZED_WINDOW_COORD)
}

/// Check whether a rectangle indicates MoveOffScreen sentinel placement.
pub fn is_move_offscreen_sentinel_rect(rect: &Rect) -> bool {
    is_move_offscreen_sentinel_position(rect.x, rect.y)
}

pub(crate) fn move_offscreen_rect_for(rect: &Rect) -> Rect {
    Rect::new(
        MOVE_OFFSCREEN_SENTINEL_COORD,
        MOVE_OFFSCREEN_SENTINEL_COORD,
        rect.width,
        rect.height,
    )
}

fn compute_restore_rect_from_offscreen(current_rect: &Rect, work_area: &Rect) -> Rect {
    let max_width = work_area.width.max(1);
    let max_height = work_area.height.max(1);
    let width = current_rect.width.max(1).min(max_width);
    let height = current_rect.height.max(1).min(max_height);
    Rect::new(work_area.x, work_area.y, width, height)
}

fn restore_window_if_offscreen_to_work_area(
    window_id: WindowId,
    work_area: &Rect,
) -> Result<bool, Win32Error> {
    let hwnd = window_id_to_hwnd(window_id)?;

    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return Err(Win32Error::WindowNotFound(window_id));
        }

        let mut current_rect = RECT::default();
        GetWindowRect(hwnd, &mut current_rect).map_err(|e| {
            Win32Error::SetPositionFailed(format!(
                "GetWindowRect failed for window {}: {}",
                window_id, e
            ))
        })?;

        let current_rect = Rect::new(
            current_rect.left,
            current_rect.top,
            current_rect.right - current_rect.left,
            current_rect.bottom - current_rect.top,
        );

        if !is_move_offscreen_sentinel_rect(&current_rect) || IsIconic(hwnd).as_bool() {
            return Ok(false);
        }

        let restore_rect = compute_restore_rect_from_offscreen(&current_rect, work_area);

        if let Err(e) = SetWindowPos(
            hwnd,
            None,
            restore_rect.x,
            restore_rect.y,
            restore_rect.width,
            restore_rect.height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        ) {
            if !IsWindow(Some(hwnd)).as_bool() {
                return Err(Win32Error::WindowNotFound(window_id));
            }
            return Err(Win32Error::SetPositionFailed(format!(
                "Failed to restore off-screen window {}: {}",
                window_id, e
            )));
        }
    }

    Ok(true)
}

/// Fraction of `rect`'s area that lies on any of the given monitor rectangles.
fn visible_fraction(rect: &Rect, monitors: &[Rect]) -> f64 {
    let area = rect.width.max(0) as f64 * rect.height.max(0) as f64;
    if area <= 0.0 {
        return 0.0;
    }
    let visible: f64 = monitors
        .iter()
        .map(|m| {
            let w = (rect.x + rect.width).min(m.x + m.width) - rect.x.max(m.x);
            let h = (rect.y + rect.height).min(m.y + m.height) - rect.y.max(m.y);
            if w > 0 && h > 0 {
                w as f64 * h as f64
            } else {
                0.0
            }
        })
        .sum();
    (visible / area).min(1.0)
}

/// Move `rect` inside `work_area`, shrinking it if it is larger.
fn clamp_rect_into(rect: &Rect, work_area: &Rect) -> Rect {
    let width = rect.width.clamp(1, work_area.width.max(1));
    let height = rect.height.clamp(1, work_area.height.max(1));
    let x = rect
        .x
        .clamp(work_area.x, work_area.x + work_area.width - width);
    let y = rect
        .y
        .clamp(work_area.y, work_area.y + work_area.height - height);
    Rect::new(x, y, width, height)
}

/// Monitor whose area overlaps `rect` most, or whose center is nearest.
fn nearest_monitor<'a>(rect: &Rect, monitors: &'a [MonitorInfo]) -> Option<&'a MonitorInfo> {
    let overlap = |m: &MonitorInfo| visible_fraction(rect, std::slice::from_ref(&m.rect));
    let center = |r: &Rect| {
        (
            r.x as i64 + r.width as i64 / 2,
            r.y as i64 + r.height as i64 / 2,
        )
    };
    let (cx, cy) = center(rect);
    monitors.iter().max_by(|a, b| {
        overlap(a)
            .partial_cmp(&overlap(b))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let (ax, ay) = center(&a.rect);
                let (bx, by) = center(&b.rect);
                let da = (ax - cx).pow(2) + (ay - cy).pow(2);
                let db = (bx - cx).pow(2) + (by - cy).pow(2);
                db.cmp(&da)
            })
    })
}

/// Move windows that are (partly) outside every monitor back onto the nearest
/// monitor's work area. A window is moved when less than `min_visible_fraction`
/// of it is on screen; minimized windows are left alone. Returns how many moved.
///
/// After a stop, crash or emergency recovery, windows at the edge of the strip
/// would otherwise stay (mostly) outside the screen.
pub fn bring_windows_into_view(window_ids: &[WindowId], min_visible_fraction: f64) -> usize {
    let monitors = match enumerate_monitors() {
        Ok(monitors) if !monitors.is_empty() => monitors,
        _ => return 0,
    };
    let monitor_rects: Vec<Rect> = monitors.iter().map(|m| m.rect).collect();
    let mut moved = 0;
    for &window_id in window_ids {
        let Ok(hwnd) = window_id_to_hwnd(window_id) else {
            continue;
        };
        unsafe {
            if !IsWindow(Some(hwnd)).as_bool() || IsIconic(hwnd).as_bool() {
                continue;
            }
            let mut r = RECT::default();
            if GetWindowRect(hwnd, &mut r).is_err() {
                continue;
            }
            let rect = Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top);
            if visible_fraction(&rect, &monitor_rects) >= min_visible_fraction {
                continue;
            }
            let Some(monitor) = nearest_monitor(&rect, &monitors) else {
                continue;
            };
            let target = clamp_rect_into(&rect, &monitor.work_area);
            if SetWindowPos(
                hwnd,
                None,
                target.x,
                target.y,
                target.width,
                target.height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .is_ok()
            {
                moved += 1;
            }
        }
    }
    if moved > 0 {
        tracing::info!("Moved {} window(s) back into view", moved);
    }
    moved
}

/// Restore one window from MoveOffScreen sentinel coordinates to the primary monitor.
///
/// Returns `Ok(true)` if the window was restored, `Ok(false)` if it was not at
/// sentinel coordinates, and `Err` if restore operations failed.
pub fn restore_window_moved_offscreen(window_id: WindowId) -> Result<bool, Win32Error> {
    let primary = get_primary_monitor()?;
    restore_window_if_offscreen_to_work_area(window_id, &primary.work_area)
}

fn restore_windows_moved_offscreen_with_work_area<F>(
    window_ids: &[WindowId],
    work_area: &Rect,
    mut restore_one: F,
) -> (usize, Vec<String>)
where
    F: FnMut(WindowId, &Rect) -> Result<bool, Win32Error>,
{
    let mut restored_count: usize = 0;
    let mut failures: Vec<String> = Vec::new();

    for &window_id in window_ids {
        match restore_one(window_id, work_area) {
            Ok(true) => restored_count += 1,
            Ok(false) => {}
            Err(e) if is_benign_window_race_error(&e) => {
                tracing::debug!(
                    "Ignoring benign race during MoveOffScreen restore for {}: {}",
                    window_id,
                    e
                );
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to restore off-screen window {} during shutdown recovery: {}",
                    window_id,
                    e
                );
                failures.push(format!("window {}: {}", window_id, e));
            }
        }
    }

    (restored_count, failures)
}

/// Restore all windows currently parked at MoveOffScreen sentinel coordinates.
///
/// Returns the number of restored windows. If any window restore fails, this
/// returns an aggregated error after attempting all windows.
pub fn restore_windows_moved_offscreen(window_ids: &[WindowId]) -> Result<usize, Win32Error> {
    if window_ids.is_empty() {
        return Ok(0);
    }

    let primary = get_primary_monitor()?;
    let (restored_count, failures) = restore_windows_moved_offscreen_with_work_area(
        window_ids,
        &primary.work_area,
        restore_window_if_offscreen_to_work_area,
    );

    if !failures.is_empty() {
        return Err(combine_operation_failures(
            "Failed to restore one or more MoveOffScreen windows",
            failures,
        ));
    }

    Ok(restored_count)
}

unsafe extern "system" fn collect_all_window_ids_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let window_ids = &mut *(lparam.0 as *mut Vec<WindowId>);
    let window_id = hwnd.0 as WindowId;
    if window_id != 0 {
        window_ids.push(window_id);
    }
    TRUE
}

fn collect_all_top_level_window_ids() -> Vec<WindowId> {
    let mut window_ids: Vec<WindowId> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(collect_all_window_ids_callback),
            LPARAM((&mut window_ids as *mut Vec<WindowId>) as isize),
        );
    }
    window_ids
}

/// Restore any top-level window parked at MoveOffScreen sentinel coordinates.
///
/// This helper is panic-safe and best-effort, making it suitable for panic
/// hooks where daemon state may be unavailable or poisoned.
pub fn restore_all_windows_moved_offscreen_best_effort() -> usize {
    let primary = match get_primary_monitor() {
        Ok(primary) => primary,
        Err(e) => {
            eprintln!(
                "[openniri] Emergency MoveOffScreen restore skipped: no primary monitor ({})",
                e
            );
            return 0;
        }
    };

    let window_ids = collect_all_top_level_window_ids();
    let (restored_count, failures) = restore_windows_moved_offscreen_with_work_area(
        &window_ids,
        &primary.work_area,
        restore_window_if_offscreen_to_work_area,
    );

    if !failures.is_empty() {
        eprintln!(
            "[openniri] Emergency MoveOffScreen restore had {} hard failure(s)",
            failures.len()
        );
    }

    if restored_count > 0 {
        eprintln!(
            "[openniri] Emergency MoveOffScreen restore recovered {} window(s)",
            restored_count
        );
    }

    restored_count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::monitor;

    #[test]
    fn test_move_offscreen_sentinel_fits_win32_coordinate_range() {
        assert!(MOVE_OFFSCREEN_SENTINEL_COORD >= i16::MIN as i32);
        assert!(is_move_offscreen_sentinel_position(
            MOVE_OFFSCREEN_SENTINEL_COORD,
            MOVE_OFFSCREEN_SENTINEL_COORD
        ));
    }

    #[test]
    fn test_move_offscreen_sentinel_detection() {
        assert!(is_move_offscreen_sentinel_position(
            MOVE_OFFSCREEN_SENTINEL_COORD - 1,
            MOVE_OFFSCREEN_SENTINEL_COORD - 500
        ));
        // Windows clamps positions to 16 bits; older versions' parked windows sit here.
        assert!(is_move_offscreen_sentinel_position(-32_768, -32_768));
        // Windows commonly reports minimized windows at (-32000, -32000).
        assert!(!is_move_offscreen_sentinel_position(-32_000, -32_000));
        assert!(!is_move_offscreen_sentinel_position(
            MOVE_OFFSCREEN_DETECTION_THRESHOLD + 1,
            MOVE_OFFSCREEN_SENTINEL_COORD
        ));
        assert!(!is_move_offscreen_sentinel_position(
            MOVE_OFFSCREEN_SENTINEL_COORD,
            MOVE_OFFSCREEN_DETECTION_THRESHOLD + 1
        ));
    }

    #[test]
    fn test_move_offscreen_restore_rect_clamps_size() {
        let offscreen = Rect::new(
            MOVE_OFFSCREEN_SENTINEL_COORD,
            MOVE_OFFSCREEN_SENTINEL_COORD,
            5000,
            0,
        );
        let work_area = Rect::new(100, 200, 1920, 1080);
        let restored = compute_restore_rect_from_offscreen(&offscreen, &work_area);

        assert_eq!(restored, Rect::new(100, 200, 1920, 1));
        assert!(is_move_offscreen_sentinel_rect(&offscreen));
        assert!(!is_move_offscreen_sentinel_rect(&restored));
    }

    #[test]
    fn test_restore_windows_moved_offscreen_ignores_races_and_reports_hard_failures() {
        let window_ids = [10, 20, 30, 40];
        let work_area = Rect::new(0, 0, 1920, 1080);
        let mut seen: Vec<WindowId> = Vec::new();
        let (restored, failures) = restore_windows_moved_offscreen_with_work_area(
            &window_ids,
            &work_area,
            |window_id, _| {
                seen.push(window_id);
                match window_id {
                    10 => Ok(true),
                    20 => Err(Win32Error::WindowNotFound(20)),
                    30 => Ok(false),
                    40 => Err(Win32Error::SetPositionFailed("boom".to_string())),
                    _ => unreachable!(),
                }
            },
        );

        assert_eq!(seen, window_ids, "one failure must not stop the others");
        assert_eq!(restored, 1);
        assert_eq!(failures.len(), 1, "a vanished window is not a failure");
        assert!(failures[0].contains("window 40"));
        assert!(failures[0].contains("boom"));
    }

    #[test]
    fn test_visible_fraction_and_clamp() {
        let monitors = [Rect::new(0, 0, 1000, 800), Rect::new(1000, 0, 1000, 800)];
        assert_eq!(
            visible_fraction(&Rect::new(100, 100, 200, 200), &monitors),
            1.0
        );
        // Spans both monitors: still fully visible.
        assert_eq!(
            visible_fraction(&Rect::new(900, 0, 200, 100), &monitors),
            1.0
        );
        assert_eq!(
            visible_fraction(&Rect::new(-150, 0, 200, 100), &monitors),
            0.25
        );
        assert_eq!(
            visible_fraction(&Rect::new(-30_000, -30_000, 200, 100), &monitors),
            0.0
        );

        let work_area = Rect::new(0, 0, 1000, 760);
        assert_eq!(
            clamp_rect_into(&Rect::new(-150, 10, 200, 700), &work_area),
            Rect::new(0, 10, 200, 700)
        );
        assert_eq!(
            clamp_rect_into(&Rect::new(900, 10, 300, 700), &work_area),
            Rect::new(700, 10, 300, 700)
        );
        // Larger than the work area: shrink to fit.
        assert_eq!(
            clamp_rect_into(&Rect::new(-50, -50, 2000, 2000), &work_area),
            Rect::new(0, 0, 1000, 760)
        );
    }

    #[test]
    fn test_nearest_monitor_prefers_overlap_then_distance() {
        let monitors = [monitor(1, 0), monitor(2, 1920)];
        let nearest = |x: i32| {
            nearest_monitor(&Rect::new(x, 0, 100, 100), &monitors)
                .unwrap()
                .id
        };
        assert_eq!(nearest(2500), 2);
        assert_eq!(nearest(-500), 1);
        assert_eq!(nearest(5000), 2);
    }
}
