//! Cloaking and uncloaking windows, daemon cloak tracking, the cloak journal and
//! the uncloak-everything recovery entry points.

use crate::error::{recover_poisoned_mutex, Win32Error};
use crate::focus::reset_window_border_color;
use crate::offscreen::{
    bring_windows_into_view, restore_all_windows_moved_offscreen_best_effort,
    restore_windows_moved_offscreen,
};
use crate::placement::HideStrategy;
use crate::shell_cloak;
use crate::window::{enumerate_windows, window_id_to_hwnd};
use openniri_core_layout::{WindowId, WindowPlacement};
use std::collections::HashSet;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::BOOL;
use windows::Win32::Foundation::{E_ACCESSDENIED, HWND, LPARAM, TRUE};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DwmSetWindowAttribute, DWMWA_CLOAK, DWMWA_CLOAKED,
};
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, IsWindow};

/// Windows this process has successfully cloaked and not yet uncloaked.
///
/// Uncloak is only attempted for windows in this set or windows DWM reports as
/// app-cloaked. Calling `DWMWA_CLOAK` on a window that was never cloaked fails
/// for most foreign windows, and those failures used to fail every layout pass.
static DAEMON_CLOAKED_WINDOWS: std::sync::Mutex<Option<HashSet<WindowId>>> =
    std::sync::Mutex::new(None);

/// Set once DWM rejects a cloak request with `E_ACCESSDENIED`.
///
/// DWM only lets the owning process cloak a window, so after the first denial
/// off-screen windows are moved off-screen instead of cloaked.
static CLOAK_ACCESS_DENIED: AtomicBool = AtomicBool::new(false);

/// `DWM_CLOAKED_APP`: the window was cloaked by an application.
const DWM_CLOAKED_APP_FLAG: u32 = 0x1;

pub(crate) fn track_daemon_cloak(window_id: WindowId, cloaked: bool) {
    let mut guard = DAEMON_CLOAKED_WINDOWS
        .lock()
        .unwrap_or_else(recover_poisoned_mutex);
    let set = guard.get_or_insert_with(HashSet::new);
    let changed = if cloaked {
        set.insert(window_id)
    } else {
        set.remove(&window_id)
    };
    if changed {
        write_cloak_journal(set);
    }
}

// ----------------------------------------------------------------------------
// Cloak journal
// ----------------------------------------------------------------------------
//
// Shell-cloaked windows look exactly like windows on other virtual desktops
// (DWM_CLOAKED_SHELL), so after a crash or hard kill they cannot be told apart
// by their state. The daemon therefore records the windows it cloaked in a small
// file that recovery (CLI `emergency-uncloak`, the panic hook, the next daemon
// start) reads to uncloak exactly those windows.

static CLOAK_JOURNAL_PATH_OVERRIDE: std::sync::Mutex<Option<std::path::PathBuf>> =
    std::sync::Mutex::new(None);

fn cloak_journal_path() -> Option<std::path::PathBuf> {
    if let Some(path) = CLOAK_JOURNAL_PATH_OVERRIDE
        .lock()
        .unwrap_or_else(recover_poisoned_mutex)
        .clone()
    {
        return Some(path);
    }
    std::env::var_os("LOCALAPPDATA").map(|dir| {
        std::path::PathBuf::from(dir)
            .join("openniri")
            .join("cloaked-windows.txt")
    })
}

fn format_cloak_journal(window_ids: &HashSet<WindowId>) -> String {
    let mut ids: Vec<WindowId> = window_ids.iter().copied().collect();
    ids.sort_unstable();
    ids.iter().map(|id| format!("{}\n", id)).collect()
}

fn parse_cloak_journal(text: &str) -> Vec<WindowId> {
    text.lines()
        .filter_map(|line| line.trim().parse::<WindowId>().ok())
        .filter(|id| *id != 0)
        .collect()
}

fn write_cloak_journal(window_ids: &HashSet<WindowId>) {
    let Some(path) = cloak_journal_path() else {
        return;
    };
    let result = if window_ids.is_empty() {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    } else {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(&path, format_cloak_journal(window_ids))
    };
    if let Err(e) = result {
        tracing::warn!("Failed to update cloak journal {}: {}", path.display(), e);
    }
}

fn read_cloak_journal() -> Vec<WindowId> {
    cloak_journal_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| parse_cloak_journal(&text))
        .unwrap_or_default()
}

/// Uncloak every window recorded in the cloak journal (left cloaked by a daemon
/// that crashed or was killed) and clear the journal. Returns how many windows
/// were uncloaked.
pub fn restore_journaled_cloaked_windows() -> usize {
    let journaled = read_cloak_journal();
    if journaled.is_empty() {
        return 0;
    }
    let mut restored = 0;
    for window_id in journaled {
        let Ok(hwnd) = window_id_to_hwnd(window_id) else {
            continue;
        };
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            continue;
        }
        match shell_cloak::set_cloak(window_id, false) {
            Ok(()) => restored += 1,
            Err(e) => tracing::warn!(
                "Failed to uncloak journaled window {} through the shell: {}",
                window_id,
                e
            ),
        }
    }
    {
        let mut guard = DAEMON_CLOAKED_WINDOWS
            .lock()
            .unwrap_or_else(recover_poisoned_mutex);
        if let Some(set) = guard.as_mut() {
            set.clear();
        }
    }
    write_cloak_journal(&HashSet::new());
    if restored > 0 {
        tracing::info!("Uncloaked {} journaled window(s)", restored);
    }
    restored
}

fn is_daemon_cloaked(window_id: WindowId) -> bool {
    let guard = DAEMON_CLOAKED_WINDOWS
        .lock()
        .unwrap_or_else(recover_poisoned_mutex);
    guard.as_ref().is_some_and(|set| set.contains(&window_id))
}

fn note_cloak_access_denied() {
    // With shell cloaking available, a DWM denial only affects that one window.
    if shell_cloak::available() {
        return;
    }
    if !CLOAK_ACCESS_DENIED.swap(true, Ordering::SeqCst) {
        tracing::warn!(
            "DWM denied cloaking a foreign window (E_ACCESSDENIED); hiding off-screen windows by moving them off-screen instead"
        );
    }
}

/// Whether DWM has rejected cloaking for this process.
pub fn cloak_access_denied() -> bool {
    CLOAK_ACCESS_DENIED.load(Ordering::SeqCst)
}

/// Whether DWM reports the window as cloaked by an application (`DWM_CLOAKED_APP`).
fn is_window_app_cloaked(window_id: WindowId) -> bool {
    let Ok(hwnd) = window_id_to_hwnd(window_id) else {
        return false;
    };
    let mut cloaked: u32 = 0;
    let result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        )
    };
    result.is_ok() && cloaked & DWM_CLOAKED_APP_FLAG != 0
}

/// Keep only uncloak targets that are actually cloaked.
pub(crate) fn filter_uncloak_targets<F>(targets: Vec<WindowId>, is_cloaked: F) -> Vec<WindowId>
where
    F: Fn(WindowId) -> bool,
{
    targets.into_iter().filter(|id| is_cloaked(*id)).collect()
}

pub(crate) fn needs_uncloak(window_id: WindowId) -> bool {
    is_daemon_cloaked(window_id) || is_window_app_cloaked(window_id)
}

pub(crate) fn collect_uncloak_targets(
    visible: &[&WindowPlacement],
    offscreen: &[&WindowPlacement],
    hide_strategy: HideStrategy,
) -> Vec<WindowId> {
    let mut targets: Vec<WindowId> = visible
        .iter()
        .map(|placement| placement.window_id)
        .collect();
    if hide_strategy == HideStrategy::MoveOffScreen {
        for placement in offscreen {
            if !targets.contains(&placement.window_id) {
                targets.push(placement.window_id);
            }
        }
    }
    targets
}

/// Cloak a window (hide from view but keep in Alt-Tab).
///
/// Cloaked windows are hidden visually but remain in the taskbar
/// and can still receive focus via Alt-Tab.
pub fn cloak_window(hwnd: WindowId) -> Result<(), Win32Error> {
    let window_id = hwnd;
    let hwnd = window_id_to_hwnd(window_id)?;
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return Err(Win32Error::WindowNotFound(window_id));
        }

        // The shell can cloak any window; DWM only windows of this process.
        match shell_cloak::set_cloak(window_id, true) {
            Ok(()) => {
                track_daemon_cloak(window_id, true);
                return Ok(());
            }
            Err(e) => tracing::debug!(
                "Shell cloak failed for window {}: {}; trying DWM",
                window_id,
                e
            ),
        }

        let cloak_value: u32 = 1;

        let result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CLOAK,
            &cloak_value as *const u32 as *const c_void,
            std::mem::size_of::<u32>() as u32,
        );

        if let Err(e) = result {
            if !IsWindow(Some(hwnd)).as_bool() {
                return Err(Win32Error::WindowNotFound(window_id));
            }
            if e.code() == E_ACCESSDENIED {
                note_cloak_access_denied();
            }
            return Err(Win32Error::CloakFailed(format!(
                "DwmSetWindowAttribute(CLOAK=1) failed for {:?}: {} (HRESULT 0x{:08X})",
                hwnd,
                e,
                e.code().0 as u32
            )));
        }
    }
    track_daemon_cloak(window_id, true);
    Ok(())
}

/// Uncloak a window (make visible again).
pub fn uncloak_window(hwnd: WindowId) -> Result<(), Win32Error> {
    let window_id = hwnd;
    let hwnd = window_id_to_hwnd(window_id)?;
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            track_daemon_cloak(window_id, false);
            return Err(Win32Error::WindowNotFound(window_id));
        }

        // Only windows this daemon cloaked go through the shell; a window cloaked
        // by its own application (or on another virtual desktop) must stay hidden.
        if is_daemon_cloaked(window_id) {
            match shell_cloak::set_cloak(window_id, false) {
                Ok(()) => {
                    track_daemon_cloak(window_id, false);
                    return Ok(());
                }
                Err(e) => tracing::debug!(
                    "Shell uncloak failed for window {}: {}; trying DWM",
                    window_id,
                    e
                ),
            }
        }

        let cloak_value: u32 = 0;

        let result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CLOAK,
            &cloak_value as *const u32 as *const c_void,
            std::mem::size_of::<u32>() as u32,
        );

        if let Err(e) = result {
            if !IsWindow(Some(hwnd)).as_bool() {
                track_daemon_cloak(window_id, false);
                return Err(Win32Error::WindowNotFound(window_id));
            }
            return Err(Win32Error::CloakFailed(format!(
                "DwmSetWindowAttribute(CLOAK=0) failed for {:?}: {} (HRESULT 0x{:08X})",
                hwnd,
                e,
                e.code().0 as u32
            )));
        }
    }
    track_daemon_cloak(window_id, false);
    Ok(())
}

/// Uncloak a list of managed windows, best-effort.
///
/// Iterates through the provided window IDs and uncloaks each one.
/// Logs warnings for failures but never panics. Also resets border colors and
/// restores windows parked at MoveOffScreen sentinel coordinates.
pub fn uncloak_all_managed_windows(window_ids: &[WindowId]) {
    restore_journaled_cloaked_windows();
    for &wid in window_ids {
        if wid == 0 {
            continue;
        }
        // Uncloaking a window that is not cloaked fails for foreign windows.
        if needs_uncloak(wid) {
            if let Err(e) = uncloak_window(wid) {
                tracing::warn!("Failed to uncloak window {} during shutdown: {}", wid, e);
            }
        }
        // Best-effort border reset
        let _ = reset_window_border_color(wid);
    }

    if let Err(e) = restore_windows_moved_offscreen(window_ids) {
        tracing::warn!(
            "MoveOffScreen shutdown recovery had one or more failures: {}",
            e
        );
    }

    // Managed windows at the edge of the strip are partly off-screen; after a
    // stop every managed window should be fully visible.
    bring_windows_into_view(window_ids, 0.95);

    tracing::info!(
        "Uncloaked {} managed windows during shutdown",
        window_ids.len()
    );
}

/// Uncloak all visible windows on the system, best-effort.
///
/// Uses `EnumWindows` to find all top-level windows and uncloaks them.
/// Also restores any windows parked at MoveOffScreen sentinel coordinates.
/// This does not require AppState and works even if state is poisoned,
/// making it suitable for use in panic hooks.
pub fn uncloak_all_visible_windows() {
    restore_journaled_cloaked_windows();
    unsafe {
        let _ = EnumWindows(Some(uncloak_all_callback), LPARAM(0));
    }
    let _ = restore_all_windows_moved_offscreen_best_effort();
    // Without the daemon's window list, only move windows that are mostly off-screen.
    if let Ok(windows) = enumerate_windows() {
        let ids: Vec<WindowId> = windows.iter().map(|w| w.hwnd).collect();
        bring_windows_into_view(&ids, 0.5);
    }
    // eprintln because tracing may not work in a panic hook
    eprintln!("[openniri] Emergency uncloak of all windows complete");
}

/// Callback for `EnumWindows` that uncloaks every window.
unsafe extern "system" fn uncloak_all_callback(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    let wid = hwnd.0 as WindowId;
    if wid != 0 {
        // Best-effort uncloak — ignore errors
        if needs_uncloak(wid) {
            let _ = uncloak_window(wid);
        }
        let _ = reset_window_border_color(wid);
    }
    TRUE // continue enumeration
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::placement;
    use openniri_core_layout::Visibility;

    #[test]
    fn test_collect_uncloak_targets_move_offscreen_includes_offscreen() {
        let visible = [placement(1, Visibility::Visible)];
        let offscreen = [
            placement(2, Visibility::OffScreenLeft),
            placement(1, Visibility::OffScreenLeft),
        ];
        let visible_refs: Vec<_> = visible.iter().collect();
        let offscreen_refs: Vec<_> = offscreen.iter().collect();

        let cloak_targets =
            collect_uncloak_targets(&visible_refs, &offscreen_refs, HideStrategy::Cloak);
        assert_eq!(cloak_targets, vec![1]);

        let move_targets =
            collect_uncloak_targets(&visible_refs, &offscreen_refs, HideStrategy::MoveOffScreen);
        assert_eq!(move_targets, vec![1, 2]);
    }

    #[test]
    fn test_filter_uncloak_targets_skips_windows_that_are_not_cloaked() {
        let targets = vec![1, 2, 3];
        assert_eq!(
            filter_uncloak_targets(targets.clone(), |id| id == 2),
            vec![2]
        );
        assert!(filter_uncloak_targets(targets, |_| false).is_empty());
    }

    #[test]
    fn test_cloak_journal_format_round_trip() {
        let ids: HashSet<WindowId> = [3, 1, 2].into_iter().collect();
        let text = format_cloak_journal(&ids);
        assert_eq!(text, "1\n2\n3\n");
        assert_eq!(parse_cloak_journal(&text), vec![1, 2, 3]);
        // Tolerates a partially written or edited file.
        assert_eq!(
            parse_cloak_journal("7\n\ngarbage\n0\n 9 \n12"),
            vec![7, 9, 12]
        );
    }

    #[test]
    fn test_daemon_cloak_tracking_round_trip() {
        let journal =
            std::env::temp_dir().join(format!("openniri-cloak-journal-{}.txt", std::process::id()));
        *CLOAK_JOURNAL_PATH_OVERRIDE
            .lock()
            .unwrap_or_else(recover_poisoned_mutex) = Some(journal.clone());

        let window_id: WindowId = 0xDEAD_0001;
        assert!(!is_daemon_cloaked(window_id));
        track_daemon_cloak(window_id, true);
        assert!(is_daemon_cloaked(window_id));
        assert!(read_cloak_journal().contains(&window_id));
        track_daemon_cloak(window_id, false);
        assert!(!is_daemon_cloaked(window_id));
        // Deletion can be pending briefly on Windows (e.g. an antivirus scan holds
        // the file open), so check what recovery would read, not the file itself.
        assert!(
            read_cloak_journal().is_empty(),
            "an empty journal records no windows"
        );

        *CLOAK_JOURNAL_PATH_OVERRIDE
            .lock()
            .unwrap_or_else(recover_poisoned_mutex) = None;
    }
}
