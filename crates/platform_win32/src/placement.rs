//! Applying layout placements: batched and immediate positioning, hiding off-screen
//! windows, foreground protection and invisible-border compensation.

use crate::cloak::{
    cloak_access_denied, cloak_windows, collect_uncloak_targets, filter_uncloak_targets,
    flush_cloak_journal, needs_uncloak, uncloak_window,
};
use crate::error::{is_benign_window_race_error, FailureLog, Win32Error};
use crate::focus::set_foreground_window;
use crate::offscreen::{
    is_move_offscreen_sentinel_rect, move_offscreen_rect_for, restore_windows_moved_offscreen,
};
use crate::window::{normalize_to_root_window, window_id_to_hwnd};
use openniri_core_layout::{Rect, Visibility, WindowId, WindowPlacement};
use std::ffi::c_void;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::UI::WindowsAndMessaging::{
    BeginDeferWindowPos, DeferWindowPos, EndDeferWindowPos, GetForegroundWindow, GetWindowRect,
    IsHungAppWindow, IsIconic, IsWindow, SetWindowPos, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE,
    SWP_NOZORDER,
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
    focused_window: Option<WindowId>,
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

    let fallback_window_id =
        select_visible_fallback_window(visible, foreground_window_id, focused_window);

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

/// Visible window that takes the foreground when the foreground window is hidden.
///
/// The layout's focused window is the right choice when it is visible; without
/// it, the first other visible window is used.
fn select_visible_fallback_window(
    visible: &[&WindowPlacement],
    foreground_window_id: WindowId,
    focused_window: Option<WindowId>,
) -> Option<WindowId> {
    let is_visible = |id: WindowId| visible.iter().any(|placement| placement.window_id == id);
    if let Some(focused) =
        focused_window.filter(|focused| *focused != foreground_window_id && is_visible(*focused))
    {
        return Some(focused);
    }
    visible
        .iter()
        .find(|placement| placement.window_id != foreground_window_id)
        .or_else(|| visible.first())
        .map(|placement| placement.window_id)
}

/// Apply window placements from the layout engine.
///
/// Same as [`apply_placements_with_focus`] without a focused window.
pub fn apply_placements(
    placements: &[WindowPlacement],
    config: &PlatformConfig,
) -> Result<(), Win32Error> {
    apply_placements_with_focus(placements, config, None)
}

/// Apply window placements from the layout engine.
///
/// This function:
/// 1. Positions visible windows (batched with DeferWindowPos when configured)
/// 2. Uncloaks windows that become visible
/// 3. Hides off-screen windows: cloaks them, or moves them off-screen when the
///    strategy says so or cloaking fails
///
/// When the foreground window is about to be hidden, focus first moves to
/// `focused_window` (the layout's focused window) if it is visible, otherwise
/// to another visible window. Windows whose application is hung are skipped,
/// because positioning them blocks until the application responds.
///
/// A failure for one window does not stop the others and nothing is rolled
/// back: every window that should be hidden ends up either cloaked (and
/// tracked for recovery) or moved off-screen, or stays where it was.
pub fn apply_placements_with_focus(
    placements: &[WindowPlacement],
    config: &PlatformConfig,
    focused_window: Option<WindowId>,
) -> Result<(), Win32Error> {
    if placements.is_empty() {
        return Ok(());
    }

    let (visible, offscreen): (Vec<_>, Vec<_>) = placements
        .iter()
        .partition(|p| p.visibility == Visibility::Visible);
    let mut failures = FailureLog::default();

    if !visible.is_empty() {
        if config.use_deferred_positioning {
            apply_placements_deferred(&visible, &mut failures);
        } else {
            apply_placements_immediate(&visible, &mut failures);
        }
    }

    // Keep windows visible under the active hide strategy.
    let uncloak_targets = filter_uncloak_targets(
        collect_uncloak_targets(&visible, &offscreen, config.hide_strategy),
        needs_uncloak,
    );
    for window_id in uncloak_targets {
        if let Err(e) = uncloak_window(window_id) {
            failures.record("uncloak", window_id, e);
        }
    }

    let protected_foreground_window =
        ensure_foreground_window_remains_visible(&visible, &offscreen, focused_window);

    if let Some(window_id) = protected_foreground_window {
        if config.hide_strategy == HideStrategy::Cloak && needs_uncloak(window_id) {
            if let Err(e) = uncloak_window(window_id) {
                failures.record("uncloak protected foreground", window_id, e);
            }
        }
        // Cloak mode falls back to moving windows off-screen, so restore in both modes.
        if let Err(e) = restore_windows_moved_offscreen(&[window_id]) {
            failures.record("restore protected foreground", window_id, e);
        }
    }

    let to_hide: Vec<&WindowPlacement> = offscreen
        .iter()
        .copied()
        .filter(|placement| {
            let protected = protected_foreground_window == Some(placement.window_id);
            if protected {
                tracing::warn!(
                    "Skipping hide for foreground window {} because focus handoff was not confirmed",
                    placement.window_id
                );
            }
            !protected
        })
        .collect();

    // Hide off-screen windows. Cloak when the strategy allows it; when the
    // cloak fails, move the window off-screen instead so one foreign window
    // cannot fail the whole pass.
    let mut move_offscreen: Vec<&WindowPlacement> = Vec::new();
    if config.hide_strategy == HideStrategy::Cloak && !cloak_access_denied() {
        let ids: Vec<WindowId> = to_hide.iter().map(|p| p.window_id).collect();
        for (result, placement) in cloak_windows(&ids).into_iter().zip(&to_hide) {
            let window_id = placement.window_id;
            match result {
                Ok(()) => {}
                Err(e) if is_benign_window_race_error(&e) => failures.record("cloak", window_id, e),
                Err(e) => {
                    tracing::debug!(
                        "Cloak failed for window {}: {}; moving it off-screen instead",
                        window_id,
                        e
                    );
                    move_offscreen.push(placement);
                }
            }
        }
    } else {
        move_offscreen = to_hide;
    }
    for placement in move_offscreen {
        if let Err(e) = move_window_offscreen(placement) {
            failures.record("move off-screen", placement.window_id, e);
        }
    }

    flush_cloak_journal();

    tracing::debug!(
        "Applied {} visible placements, {} off-screen",
        visible.len(),
        offscreen.len()
    );

    failures.into_result("apply_placements completed with failures")
}

/// Park a window at the MoveOffScreen sentinel unless it is already there.
fn move_window_offscreen(placement: &WindowPlacement) -> Result<(), Win32Error> {
    let hwnd = window_id_to_hwnd(placement.window_id)?;
    let mut current = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut current) }.is_ok()
        && is_move_offscreen_sentinel_rect(&Rect::new(
            current.left,
            current.top,
            current.right - current.left,
            current.bottom - current.top,
        ))
    {
        return Ok(());
    }
    set_window_pos_immediate(&WindowPlacement {
        window_id: placement.window_id,
        rect: move_offscreen_rect_for(&placement.rect),
        visibility: Visibility::OffScreenLeft,
        column_index: placement.column_index,
    })
}

/// Whether a window's application is hung. Positioning such a window blocks
/// the caller until the application responds (or forever).
fn is_window_hung(window_id: WindowId, hwnd: HWND) -> bool {
    let hung = unsafe { IsHungAppWindow(hwnd) }.as_bool();
    if hung {
        tracing::debug!(
            "Skipping placement of window {}: its application is not responding",
            window_id
        );
    }
    hung
}

/// Apply placements using DeferWindowPos for batched positioning.
///
/// Batching moves all windows in a single screen update, reducing flicker. If
/// the batch cannot be built or committed, every window is positioned
/// individually instead (a failed `DeferWindowPos` invalidates the batch, so it
/// must not be committed).
fn apply_placements_deferred(placements: &[&WindowPlacement], failures: &mut FailureLog) {
    let mut targets: Vec<(&WindowPlacement, HWND)> = Vec::with_capacity(placements.len());
    for placement in placements {
        let hwnd = match window_id_to_hwnd(placement.window_id) {
            Ok(hwnd) => hwnd,
            Err(e) => {
                failures.record("position", placement.window_id, e);
                continue;
            }
        };
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            failures.record(
                "position",
                placement.window_id,
                Win32Error::WindowNotFound(placement.window_id),
            );
            continue;
        }
        if !is_window_hung(placement.window_id, hwnd) {
            targets.push((placement, hwnd));
        }
    }
    if targets.is_empty() || commit_deferred_batch(&targets) {
        return;
    }
    for (placement, _) in targets {
        if let Err(e) = set_window_pos_immediate(placement) {
            failures.record("position", placement.window_id, e);
        }
    }
}

/// Position all targets in one DeferWindowPos batch. Returns false if the batch
/// failed and the windows must be positioned individually.
fn commit_deferred_batch(targets: &[(&WindowPlacement, HWND)]) -> bool {
    unsafe {
        let mut hdwp = match BeginDeferWindowPos(targets.len().min(i32::MAX as usize) as i32) {
            Ok(hdwp) => hdwp,
            Err(e) => {
                tracing::warn!(
                    "BeginDeferWindowPos failed: {}; positioning individually",
                    e
                );
                return false;
            }
        };
        for (placement, hwnd) in targets {
            let rect = compensate_invisible_border(&placement.rect, invisible_border(*hwnd));
            match DeferWindowPos(
                hdwp,
                *hwnd,
                None, // no z-order change with SWP_NOZORDER
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            ) {
                Ok(next) => hdwp = next,
                Err(e) => {
                    tracing::warn!(
                        "DeferWindowPos failed for window {}: {}; positioning individually",
                        placement.window_id,
                        e
                    );
                    return false;
                }
            }
        }
        if let Err(e) = EndDeferWindowPos(hdwp) {
            tracing::warn!("EndDeferWindowPos failed: {}; positioning individually", e);
            return false;
        }
    }
    true
}

/// Apply placements using immediate SetWindowPos calls.
fn apply_placements_immediate(placements: &[&WindowPlacement], failures: &mut FailureLog) {
    for placement in placements {
        if let Err(e) = set_window_pos_immediate(placement) {
            failures.record("position", placement.window_id, e);
        }
    }
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
///
/// Windows of hung applications are skipped. `SWP_ASYNCWINDOWPOS` posts the
/// request to the window's thread instead of waiting for it, so a slow
/// application cannot block the placement worker either.
fn set_window_pos_immediate(placement: &WindowPlacement) -> Result<(), Win32Error> {
    let window_id = placement.window_id;
    let hwnd = window_id_to_hwnd(window_id)?;
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return Err(Win32Error::WindowNotFound(window_id));
        }
        if is_window_hung(window_id, hwnd) {
            return Ok(());
        }

        let rect = compensate_invisible_border(&placement.rect, invisible_border(hwnd));

        if let Err(e) = SetWindowPos(
            hwnd,
            None,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
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
        assert_eq!(
            select_visible_fallback_window(&visible_refs, 1, None),
            Some(2)
        );
        assert_eq!(
            select_visible_fallback_window(&visible_refs, 2, None),
            Some(1)
        );
        // Only the foreground window is visible: it is still the fallback.
        assert_eq!(
            select_visible_fallback_window(&visible_refs[..1], 1, None),
            Some(1)
        );
        assert_eq!(select_visible_fallback_window(&[], 42, None), None);
    }

    #[test]
    fn test_select_visible_fallback_window_prefers_visible_focused_window() {
        let visible = [
            placement(1, Visibility::Visible),
            placement(2, Visibility::Visible),
            placement(3, Visibility::Visible),
        ];
        let visible_refs: Vec<_> = visible.iter().collect();
        assert_eq!(
            select_visible_fallback_window(&visible_refs, 9, Some(3)),
            Some(3)
        );
        // A focused window that is not visible, or is the foreground window
        // being hidden, falls back to the old choice.
        assert_eq!(
            select_visible_fallback_window(&visible_refs, 9, Some(4)),
            Some(1)
        );
        assert_eq!(
            select_visible_fallback_window(&visible_refs, 3, Some(3)),
            Some(1)
        );
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
