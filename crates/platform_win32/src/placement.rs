//! Applying layout placements: batched and immediate positioning, hiding off-screen
//! windows, foreground protection and invisible-border compensation.

use crate::cloak::{
    cloak_access_denied, cloak_window, collect_uncloak_targets, filter_uncloak_targets,
    needs_uncloak, uncloak_window,
};
use crate::error::{combine_operation_failures, is_benign_window_race_error, Win32Error};
use crate::focus::set_foreground_window;
use crate::offscreen::{move_offscreen_rect_for, restore_windows_moved_offscreen};
use crate::window::{normalize_to_root_window, window_id_to_hwnd};
use openniri_core_layout::{Rect, Visibility, WindowId, WindowPlacement};
use std::ffi::c_void;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::UI::WindowsAndMessaging::{
    BeginDeferWindowPos, DeferWindowPos, EndDeferWindowPos, GetForegroundWindow, GetWindowRect,
    IsIconic, IsWindow, SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER,
};

/// Strategy for hiding off-screen windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HideStrategy {
    /// Use DWM cloaking (preferred, keeps window in Alt-Tab).
    #[default]
    Cloak,
    /// Move windows off-screen (alternative when cloaking is disabled).
    /// Windows are moved far off-screen rather than cloaked.
    MoveOffScreen,
}

/// Configuration for the Win32 platform layer.
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    /// Strategy for hiding off-screen windows.
    pub hide_strategy: HideStrategy,
    /// Whether to use DeferWindowPos for batched moves.
    pub use_deferred_positioning: bool,
}

impl Default for PlatformConfig {
    fn default() -> Self {
        Self {
            hide_strategy: HideStrategy::default(),
            use_deferred_positioning: true,
        }
    }
}

fn ensure_foreground_window_remains_visible(
    visible: &[&WindowPlacement],
    offscreen: &[&WindowPlacement],
) -> Option<WindowId> {
    if offscreen.is_empty() {
        return None;
    }

    let foreground_window_id = unsafe {
        let raw_hwnd = GetForegroundWindow();
        if raw_hwnd.0.is_null() {
            return None;
        }
        let hwnd = normalize_to_root_window(raw_hwnd);
        if hwnd.0.is_null() {
            return None;
        }
        hwnd.0 as usize as WindowId
    };

    let foreground_will_be_hidden = offscreen_contains_window(offscreen, foreground_window_id);
    if !foreground_will_be_hidden {
        return None;
    }

    let fallback_window_id = select_visible_fallback_window(visible, foreground_window_id);

    if let Some(window_id) = fallback_window_id {
        match set_foreground_window(window_id) {
            Ok(true) => return None,
            Ok(false) => {
                tracing::warn!(
                    "Foreground switch from window {} to visible fallback {} was denied by the OS; keeping foreground window visible",
                    foreground_window_id,
                    window_id
                );
                return Some(foreground_window_id);
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to move foreground focus from window {} to visible fallback {} before hide: {}; keeping foreground window visible",
                    foreground_window_id,
                    window_id,
                    e
                );
                return Some(foreground_window_id);
            }
        }
    }

    tracing::warn!(
        "No visible fallback window was available while foreground window {} would be hidden; keeping foreground window visible",
        foreground_window_id
    );
    Some(foreground_window_id)
}

fn offscreen_contains_window(offscreen: &[&WindowPlacement], window_id: WindowId) -> bool {
    offscreen
        .iter()
        .any(|placement| placement.window_id == window_id)
}

fn select_visible_fallback_window(
    visible: &[&WindowPlacement],
    foreground_window_id: WindowId,
) -> Option<WindowId> {
    visible
        .iter()
        .find(|placement| placement.window_id != foreground_window_id)
        .or_else(|| visible.first())
        .map(|placement| placement.window_id)
}

/// Undo visibility changes made by a failed `apply_placements` call.
///
/// This deliberately does not change the foreground window. A failed apply can
/// repeat on every animation frame, and forcing focus each time locked users out
/// of their own windows (INC-49).
fn rollback_apply_visibility_changes(
    cloaked_window_ids: &[WindowId],
    moved_offscreen_window_ids: &[WindowId],
) {
    for window_id in cloaked_window_ids {
        if let Err(e) = uncloak_window(*window_id) {
            tracing::warn!(
                "Rollback failed while uncloaking window {} after apply error: {}",
                window_id,
                e
            );
        }
    }

    if !moved_offscreen_window_ids.is_empty() {
        if let Err(e) = restore_windows_moved_offscreen(moved_offscreen_window_ids) {
            tracing::warn!(
                "Rollback failed while restoring moved-offscreen windows after apply error: {}",
                e
            );
        }
    }
}

fn record_apply_side_effect_failure(
    side_effect_failures: &mut Vec<String>,
    benign_race_count: &mut usize,
    operation: &str,
    window_id: WindowId,
    error: Win32Error,
) {
    if is_benign_window_race_error(&error) {
        *benign_race_count += 1;
        tracing::debug!(
            "Ignoring benign race during {} for window {}: {}",
            operation,
            window_id,
            error
        );
    } else {
        tracing::warn!("Failed to {} window {}: {}", operation, window_id, error);
        side_effect_failures.push(format!("{} {} failed: {}", operation, window_id, error));
    }
}

/// Apply window placements from the layout engine.
///
/// This function:
/// 1. Groups placements by visibility
/// 2. Uses DeferWindowPos for visible windows (batched move)
/// 3. Applies cloaking/uncloaking based on visibility changes
pub fn apply_placements(
    placements: &[WindowPlacement],
    config: &PlatformConfig,
) -> Result<(), Win32Error> {
    if placements.is_empty() {
        return Ok(());
    }

    // Separate visible and off-screen windows
    let (visible, offscreen): (Vec<_>, Vec<_>) = placements
        .iter()
        .partition(|p| p.visibility == Visibility::Visible);
    let mut side_effect_failures: Vec<String> = Vec::new();
    let mut benign_side_effect_races: usize = 0;
    let mut cloaked_window_ids: Vec<WindowId> = Vec::new();
    let mut moved_offscreen_window_ids: Vec<WindowId> = Vec::new();

    // Apply positions for visible windows
    if !visible.is_empty() {
        if config.use_deferred_positioning {
            apply_placements_deferred(&visible)?;
        } else {
            apply_placements_immediate(&visible)?;
        }
    }

    // Keep windows visible under the active hide strategy.
    let uncloak_targets = filter_uncloak_targets(
        collect_uncloak_targets(&visible, &offscreen, config.hide_strategy),
        needs_uncloak,
    );
    for window_id in uncloak_targets {
        if let Err(e) = uncloak_window(window_id) {
            record_apply_side_effect_failure(
                &mut side_effect_failures,
                &mut benign_side_effect_races,
                "uncloak",
                window_id,
                e,
            );
        }
    }

    let protected_foreground_window =
        ensure_foreground_window_remains_visible(&visible, &offscreen);

    if let Some(window_id) = protected_foreground_window {
        if config.hide_strategy == HideStrategy::Cloak && needs_uncloak(window_id) {
            if let Err(e) = uncloak_window(window_id) {
                record_apply_side_effect_failure(
                    &mut side_effect_failures,
                    &mut benign_side_effect_races,
                    "uncloak protected foreground",
                    window_id,
                    e,
                );
            }
        }
        // Cloak mode falls back to moving windows off-screen, so restore in both modes.
        if let Err(e) = restore_windows_moved_offscreen(&[window_id]) {
            record_apply_side_effect_failure(
                &mut side_effect_failures,
                &mut benign_side_effect_races,
                "restore protected foreground",
                window_id,
                e,
            );
        }
    }

    // Hide off-screen windows. Cloak when the strategy allows it; when DWM
    // rejects the cloak, move the window off-screen instead so one foreign
    // window cannot fail the whole pass.
    for placement in &offscreen {
        if protected_foreground_window == Some(placement.window_id) {
            tracing::warn!(
                "Skipping hide for foreground window {} because focus handoff was not confirmed",
                placement.window_id
            );
            continue;
        }
        if config.hide_strategy == HideStrategy::Cloak && !cloak_access_denied() {
            match cloak_window(placement.window_id) {
                Ok(()) => {
                    cloaked_window_ids.push(placement.window_id);
                    continue;
                }
                Err(e) if is_benign_window_race_error(&e) => {
                    benign_side_effect_races += 1;
                    continue;
                }
                Err(e) => tracing::debug!(
                    "Cloak failed for window {}: {}; moving it off-screen instead",
                    placement.window_id,
                    e
                ),
            }
        }
        let offscreen_placement = WindowPlacement {
            window_id: placement.window_id,
            rect: move_offscreen_rect_for(&placement.rect),
            visibility: Visibility::OffScreenLeft,
            column_index: placement.column_index,
        };
        if let Err(e) = set_window_pos_immediate(&offscreen_placement) {
            record_apply_side_effect_failure(
                &mut side_effect_failures,
                &mut benign_side_effect_races,
                "move off-screen",
                placement.window_id,
                e,
            );
        } else {
            moved_offscreen_window_ids.push(placement.window_id);
        }
    }

    tracing::debug!(
        "Applied {} visible placements, {} off-screen",
        visible.len(),
        offscreen.len()
    );

    if !side_effect_failures.is_empty() {
        rollback_apply_visibility_changes(&cloaked_window_ids, &moved_offscreen_window_ids);
        return Err(combine_operation_failures(
            "apply_placements completed with side-effect failures",
            side_effect_failures,
        ));
    }

    if benign_side_effect_races > 0 {
        tracing::debug!(
            "apply_placements ignored {} benign side-effect race(s)",
            benign_side_effect_races
        );
    }

    Ok(())
}

/// Apply placements using DeferWindowPos for batched positioning.
///
/// This function uses the Windows DeferWindowPos API to batch multiple
/// window positioning operations into a single screen update, reducing
/// flicker and improving performance.
///
/// If EndDeferWindowPos fails, falls back to individual SetWindowPos calls
/// for all windows and returns an error if fallback cannot fully recover.
/// If individual DeferWindowPos calls fail during the batch, those placements
/// are tracked and retried individually after the batch; unrecovered failures
/// are returned as an error.
fn apply_placements_deferred(placements: &[&WindowPlacement]) -> Result<(), Win32Error> {
    unsafe {
        let hdwp =
            BeginDeferWindowPos(placements.len().min(i32::MAX as usize) as i32).map_err(|e| {
                Win32Error::SetPositionFailed(format!("BeginDeferWindowPos failed: {}", e))
            })?;

        let mut current_hdwp = hdwp;
        let mut failed_placements: Vec<&WindowPlacement> = Vec::new();

        for placement in placements {
            let hwnd = match window_id_to_hwnd(placement.window_id) {
                Ok(h) => h,
                Err(err) => {
                    tracing::warn!(
                        "Skipping deferred placement for invalid window ID {}: {}",
                        placement.window_id,
                        err
                    );
                    failed_placements.push(placement);
                    continue;
                }
            };
            let rect = compensate_invisible_border(&placement.rect, invisible_border(hwnd));

            match DeferWindowPos(
                current_hdwp,
                hwnd,
                None, // HWND_TOP equivalent - no z-order change with SWP_NOZORDER
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            ) {
                Ok(new_hdwp) => {
                    current_hdwp = new_hdwp;
                }
                Err(e) => {
                    tracing::warn!(
                        "DeferWindowPos failed for window {}: {}, will retry individually",
                        placement.window_id,
                        e
                    );
                    failed_placements.push(placement);
                }
            }
        }

        // Try to commit the batch
        if let Err(e) = EndDeferWindowPos(current_hdwp) {
            tracing::warn!(
                "EndDeferWindowPos failed: {}. Falling back to individual positioning for all windows.",
                e
            );
            // Fall back to individual positioning for ALL windows
            let mut fallback_failures: Vec<String> = Vec::new();
            let mut benign_races: usize = 0;
            for placement in placements {
                if let Err(fallback_err) = set_window_pos_immediate(placement) {
                    if is_benign_window_race_error(&fallback_err) {
                        benign_races += 1;
                        tracing::debug!(
                            "Ignoring benign race during deferred fallback for window {}: {}",
                            placement.window_id,
                            fallback_err
                        );
                    } else {
                        tracing::warn!(
                            "Individual SetWindowPos also failed for {}: {}",
                            placement.window_id,
                            fallback_err
                        );
                        fallback_failures
                            .push(format!("window {}: {}", placement.window_id, fallback_err));
                    }
                }
            }
            if !fallback_failures.is_empty() {
                fallback_failures.insert(0, format!("EndDeferWindowPos failed: {}", e));
                return Err(combine_operation_failures(
                    "Deferred positioning fallback could not fully recover",
                    fallback_failures,
                ));
            }
            if benign_races > 0 {
                tracing::debug!(
                    "Deferred fallback ignored {} benign window race(s)",
                    benign_races
                );
            }
        } else {
            // Batch succeeded, now handle any that failed during deferral
            let mut fallback_failures: Vec<String> = Vec::new();
            let mut benign_races: usize = 0;
            for placement in failed_placements {
                if let Err(fallback_err) = set_window_pos_immediate(placement) {
                    if is_benign_window_race_error(&fallback_err) {
                        benign_races += 1;
                        tracing::debug!(
                            "Ignoring benign race during deferred retry for window {}: {}",
                            placement.window_id,
                            fallback_err
                        );
                    } else {
                        tracing::warn!(
                            "Fallback SetWindowPos failed for {}: {}",
                            placement.window_id,
                            fallback_err
                        );
                        fallback_failures
                            .push(format!("window {}: {}", placement.window_id, fallback_err));
                    }
                }
            }
            if !fallback_failures.is_empty() {
                return Err(combine_operation_failures(
                    "Deferred positioning had unrecovered per-window failures",
                    fallback_failures,
                ));
            }
            if benign_races > 0 {
                tracing::debug!(
                    "Deferred retry ignored {} benign window race(s)",
                    benign_races
                );
            }
        }
    }

    Ok(())
}

/// Apply placements using immediate SetWindowPos calls.
fn apply_placements_immediate(placements: &[&WindowPlacement]) -> Result<(), Win32Error> {
    for placement in placements {
        if let Err(e) = set_window_pos_immediate(placement) {
            if is_benign_window_race_error(&e) {
                tracing::debug!(
                    "Ignoring benign race during immediate placement for window {}: {}",
                    placement.window_id,
                    e
                );
                continue;
            }
            return Err(e);
        }
    }
    Ok(())
}

/// Invisible frame around a window, per side (left, top, right, bottom).
///
/// On Windows 10/11 `GetWindowRect` includes transparent resize borders (about
/// 7 px at 100% scale) that `DWMWA_EXTENDED_FRAME_BOUNDS` excludes. Without
/// compensation the visible gaps between tiled windows are larger than
/// configured and uneven between applications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct InvisibleBorder {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// Borders larger than this are not resize borders (e.g. a window mid-animation).
const MAX_INVISIBLE_BORDER: i32 = 32;

fn invisible_border_from_rects(window: &RECT, frame: &RECT) -> InvisibleBorder {
    let border = InvisibleBorder {
        left: frame.left - window.left,
        top: frame.top - window.top,
        right: window.right - frame.right,
        bottom: window.bottom - frame.bottom,
    };
    let sane = |v: i32| (0..=MAX_INVISIBLE_BORDER).contains(&v);
    if sane(border.left) && sane(border.top) && sane(border.right) && sane(border.bottom) {
        border
    } else {
        InvisibleBorder::default()
    }
}

fn invisible_border(hwnd: HWND) -> InvisibleBorder {
    unsafe {
        if IsIconic(hwnd).as_bool() {
            return InvisibleBorder::default();
        }
        let mut window = RECT::default();
        let mut frame = RECT::default();
        if GetWindowRect(hwnd, &mut window).is_err()
            || DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut frame as *mut RECT as *mut c_void,
                std::mem::size_of::<RECT>() as u32,
            )
            .is_err()
        {
            return InvisibleBorder::default();
        }
        invisible_border_from_rects(&window, &frame)
    }
}

/// Window rectangle that puts the window's *visible* frame at `rect`.
fn compensate_invisible_border(rect: &Rect, border: InvisibleBorder) -> Rect {
    Rect::new(
        rect.x - border.left,
        rect.y - border.top,
        rect.width + border.left + border.right,
        rect.height + border.top + border.bottom,
    )
}

/// Set window position immediately using SetWindowPos.
fn set_window_pos_immediate(placement: &WindowPlacement) -> Result<(), Win32Error> {
    let window_id = placement.window_id;
    let hwnd = window_id_to_hwnd(window_id)?;
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return Err(Win32Error::WindowNotFound(window_id));
        }

        let rect = compensate_invisible_border(&placement.rect, invisible_border(hwnd));

        if let Err(e) = SetWindowPos(
            hwnd,
            None,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        ) {
            if !IsWindow(Some(hwnd)).as_bool() {
                return Err(Win32Error::WindowNotFound(window_id));
            }
            return Err(Win32Error::SetPositionFailed(format!(
                "SetWindowPos failed for window {}: {}",
                window_id, e
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::placement;

    #[test]
    fn test_select_visible_fallback_window_prefers_non_foreground() {
        let visible = [
            placement(1, Visibility::Visible),
            placement(2, Visibility::Visible),
        ];
        let visible_refs: Vec<_> = visible.iter().collect();
        assert_eq!(select_visible_fallback_window(&visible_refs, 1), Some(2));
        assert_eq!(select_visible_fallback_window(&visible_refs, 2), Some(1));
        // Only the foreground window is visible: it is still the fallback.
        assert_eq!(
            select_visible_fallback_window(&visible_refs[..1], 1),
            Some(1)
        );
        assert_eq!(select_visible_fallback_window(&[], 42), None);
    }

    #[test]
    fn test_apply_placements_reports_move_offscreen_errors() {
        let config = PlatformConfig {
            hide_strategy: HideStrategy::MoveOffScreen,
            use_deferred_positioning: false,
        };
        let placements = vec![WindowPlacement {
            window_id: 0,
            rect: Rect::new(0, 0, 800, 600),
            visibility: Visibility::OffScreenLeft,
            column_index: 0,
        }];

        let result = apply_placements(&placements, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_invisible_border_compensation() {
        let window = RECT {
            left: 93,
            top: 10,
            right: 507,
            bottom: 1057,
        };
        let frame = RECT {
            left: 100,
            top: 10,
            right: 500,
            bottom: 1050,
        };
        let border = invisible_border_from_rects(&window, &frame);
        assert_eq!(
            border,
            InvisibleBorder {
                left: 7,
                top: 0,
                right: 7,
                bottom: 7
            }
        );
        // The visible frame lands exactly on the layout rectangle.
        assert_eq!(
            compensate_invisible_border(&Rect::new(10, 10, 400, 1040), border),
            Rect::new(3, 10, 414, 1047)
        );
        // Implausible values (e.g. mid-animation) are ignored.
        let odd = RECT {
            left: 0,
            top: 0,
            right: 1000,
            bottom: 1000,
        };
        let tiny = RECT {
            left: 200,
            top: 200,
            right: 300,
            bottom: 300,
        };
        assert_eq!(
            invisible_border_from_rects(&odd, &tiny),
            InvisibleBorder::default()
        );
    }
}
