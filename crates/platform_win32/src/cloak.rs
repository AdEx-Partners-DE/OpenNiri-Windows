//! Cloaking and uncloaking windows, daemon cloak tracking, the cloak journal and
//! the uncloak-everything recovery entry points.

use crate::error::{recover_poisoned_mutex, Win32Error};
use crate::focus::reset_window_border_color;
use crate::offscreen::{
    bring_windows_into_view, restore_all_windows_moved_offscreen_best_effort,
    restore_windows_moved_offscreen,
};
use crate::placement::HideStrategy;
use crate::shell_cloak::{self, ShellCloakError};
use crate::window::{
    enumerate_windows, is_window_cloaked, window_id_to_hwnd, window_process_identity,
    ProcessIdentity,
};
use openniri_core_layout::{WindowId, WindowPlacement};
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::core::BOOL;
use windows::Win32::Foundation::{E_ACCESSDENIED, HWND, LPARAM, TRUE};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DwmSetWindowAttribute, DWMWA_CLOAK, DWMWA_CLOAKED,
};
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, IsWindow};

/// Windows this process has cloaked (or asked the shell to cloak) and not yet
/// uncloaked, with the identity of their owning process.
///
/// Uncloak is only attempted for windows in this set or windows DWM reports as
/// app-cloaked. Calling `DWMWA_CLOAK` on a window that was never cloaked fails
/// for most foreign windows, and those failures used to fail every layout pass.
struct CloakTracking {
    windows: BTreeMap<WindowId, Option<ProcessIdentity>>,
    /// The journal file does not reflect `windows` yet.
    dirty: bool,
}

static TRACKING: std::sync::Mutex<CloakTracking> = std::sync::Mutex::new(CloakTracking {
    windows: BTreeMap::new(),
    dirty: false,
});

fn tracking() -> std::sync::MutexGuard<'static, CloakTracking> {
    TRACKING.lock().unwrap_or_else(recover_poisoned_mutex)
}

/// Set once DWM rejects a cloak request with `E_ACCESSDENIED`.
///
/// DWM only lets the owning process cloak a window, so after the first denial
/// off-screen windows are moved off-screen instead of cloaked.
static CLOAK_ACCESS_DENIED: AtomicBool = AtomicBool::new(false);

/// `DWM_CLOAKED_APP`: the window was cloaked by an application.
const DWM_CLOAKED_APP_FLAG: u32 = 0x1;

/// Record that this process cloaked a window. Returns true if it was not tracked.
///
/// Only marks the journal dirty; [`flush_cloak_journal`] writes it.
pub(crate) fn track_daemon_cloak(window_id: WindowId, owner: Option<ProcessIdentity>) -> bool {
    let mut tracking = tracking();
    if tracking.windows.contains_key(&window_id) {
        return false;
    }
    tracking.windows.insert(window_id, owner);
    tracking.dirty = true;
    true
}

/// Forget a window (uncloaked or destroyed). Only marks the journal dirty, so
/// this is cheap enough for hook callbacks; a stale journal entry is harmless.
pub(crate) fn untrack_daemon_cloak(window_id: WindowId) {
    let mut tracking = tracking();
    if tracking.windows.remove(&window_id).is_some() {
        tracking.dirty = true;
    }
}

fn is_daemon_cloaked(window_id: WindowId) -> bool {
    tracking().windows.contains_key(&window_id)
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
//
// One line per window: `<hwnd> <pid> <process start time>`. Window handles are
// reused, also across reboots, so recovery only acts on an entry whose window is
// still owned by the recorded process. Lines with a bare `<hwnd>` (written by
// older versions) carry no owner and are acted on as before.
//
// The file is written whole (temporary file plus rename) at batch boundaries:
// before windows disappear (additions), and at the end of a layout pass or
// recovery run (removals). Hook callbacks never write it.

/// A journal line: a cloaked window and, when known, its owning process.
type JournalEntry = (WindowId, Option<ProcessIdentity>);

static CLOAK_JOURNAL_PATH_OVERRIDE: std::sync::Mutex<Option<std::path::PathBuf>> =
    std::sync::Mutex::new(None);

/// Serialises journal writers so an older snapshot never overwrites a newer one.
static JOURNAL_WRITE: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

fn format_cloak_journal(entries: &[JournalEntry]) -> String {
    let mut sorted = entries.to_vec();
    sorted.sort_unstable_by_key(|(id, _)| *id);
    sorted
        .iter()
        .map(|(id, owner)| match owner {
            Some(owner) => format!("{} {} {}\n", id, owner.pid, owner.start_time),
            None => format!("{}\n", id),
        })
        .collect()
}

fn parse_cloak_journal(text: &str) -> Vec<JournalEntry> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let id = fields
                .next()?
                .parse::<WindowId>()
                .ok()
                .filter(|id| *id != 0)?;
            let owner = match fields.next() {
                None => None,
                Some(pid) => Some(ProcessIdentity {
                    pid: pid.parse().ok()?,
                    start_time: match fields.next() {
                        Some(start) => start.parse().ok()?,
                        None => 0,
                    },
                }),
            };
            Some((id, owner))
        })
        .collect()
}

/// Whether a journal entry still refers to the window it was written for.
///
/// `current` is the owner of the live window with that handle (`None` if there
/// is no such window). Entries without an owner (old format) only need a live
/// window. A start time of 0 means unknown and is not compared.
fn journal_entry_matches(
    recorded: Option<ProcessIdentity>,
    current: Option<ProcessIdentity>,
) -> bool {
    match (recorded, current) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some(recorded), Some(current)) => {
            recorded.pid == current.pid
                && (recorded.start_time == 0
                    || current.start_time == 0
                    || recorded.start_time == current.start_time)
        }
    }
}

fn write_cloak_journal(entries: &[JournalEntry]) -> std::io::Result<()> {
    let Some(path) = cloak_journal_path() else {
        return Ok(());
    };
    if entries.is_empty() {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Recovery must never read a half-written file: write a temporary file and
    // move it over the journal. The process id keeps the CLI and the daemon
    // from sharing a temporary file.
    let mut temp = path.clone().into_os_string();
    temp.push(format!(".{}.tmp", std::process::id()));
    let temp = std::path::PathBuf::from(temp);
    std::fs::write(&temp, format_cloak_journal(entries))?;
    std::fs::rename(&temp, &path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

/// Write the journal if tracking changed since the last write.
pub(crate) fn flush_cloak_journal() {
    let _writer = JOURNAL_WRITE.lock().unwrap_or_else(recover_poisoned_mutex);
    let snapshot: Vec<JournalEntry> = {
        let mut tracking = tracking();
        if !tracking.dirty {
            return;
        }
        tracking.dirty = false;
        tracking
            .windows
            .iter()
            .map(|(id, owner)| (*id, *owner))
            .collect()
    };
    if let Err(e) = write_cloak_journal(&snapshot) {
        tracking().dirty = true;
        tracing::warn!("Failed to update cloak journal: {}", e);
    }
}

fn read_cloak_journal() -> Vec<JournalEntry> {
    cloak_journal_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| parse_cloak_journal(&text))
        .unwrap_or_default()
}

/// Uncloak every window recorded in the cloak journal or tracked by this
/// process (left cloaked by a daemon that crashed or was killed). Returns how
/// many windows were uncloaked.
///
/// Entries are dropped once their window is uncloaked, gone, or owned by a
/// different process than recorded. Windows whose uncloak failed stay tracked
/// and journaled, so later recovery (and [`uncloak_all_managed_windows`])
/// retries them.
pub fn restore_journaled_cloaked_windows() -> usize {
    let mut candidates: BTreeMap<WindowId, Option<ProcessIdentity>> =
        read_cloak_journal().into_iter().collect();
    for (id, owner) in tracking().windows.iter() {
        candidates.insert(*id, *owner);
    }
    if candidates.is_empty() {
        return 0;
    }

    let mut restored = 0;
    let mut still_cloaked: Vec<JournalEntry> = Vec::new();
    for (window_id, recorded) in &candidates {
        let current = window_process_identity(*window_id);
        if !journal_entry_matches(*recorded, current) {
            if current.is_some() {
                tracing::debug!(
                    "Ignoring journaled window {}: its handle now belongs to another process",
                    window_id
                );
            }
            continue;
        }
        match shell_cloak::set_cloak(*window_id, false) {
            Ok(()) => restored += 1,
            Err(e) => {
                tracing::warn!(
                    "Failed to uncloak journaled window {} through the shell: {}",
                    window_id,
                    e
                );
                still_cloaked.push((*window_id, *recorded));
            }
        }
    }

    {
        let mut tracking = tracking();
        for window_id in candidates.keys() {
            tracking.windows.remove(window_id);
        }
        // A pending (timed-out) uncloak is untracked by the shell thread when it
        // completes; until then the window counts as cloaked.
        tracking.windows.extend(still_cloaked);
        tracking.dirty = true;
    }
    flush_cloak_journal();
    if restored > 0 {
        tracing::info!("Uncloaked {} journaled window(s)", restored);
    }
    restored
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

/// Whether a window that should be hidden still needs a cloak request.
///
/// Layout passes run on every animation frame, and each request is a COM round
/// trip to Explorer. A window this process tracks as cloaked and that DWM
/// reports cloaked is left alone; anything else (for example a cloak lost when
/// Explorer restarted) is cloaked again.
fn needs_cloak_request(tracked: bool, cloaked_now: bool) -> bool {
    !(tracked && cloaked_now)
}

fn dwm_set_cloak(window_id: WindowId, hwnd: HWND, cloak: bool) -> Result<(), Win32Error> {
    let value: u32 = u32::from(cloak);
    let result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_CLOAK,
            &value as *const u32 as *const c_void,
            std::mem::size_of::<u32>() as u32,
        )
    };
    result.map_err(|e| {
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return Win32Error::WindowNotFound(window_id);
        }
        if cloak && e.code() == E_ACCESSDENIED {
            note_cloak_access_denied();
        }
        Win32Error::CloakFailed(format!(
            "DwmSetWindowAttribute(CLOAK={}) failed for {:?}: {} (HRESULT 0x{:08X})",
            value,
            hwnd,
            e,
            e.code().0 as u32
        ))
    })
}

/// Cloak several windows, skipping those already cloaked by this process.
///
/// Every window is tracked and journaled *before* it is asked to disappear, so
/// a crash at any point leaves no invisible window that recovery does not know
/// about. A window stays tracked while its shell request is still pending; the
/// shell thread records the eventual outcome.
///
/// Returns one result per window, in the order of `window_ids`.
pub(crate) fn cloak_windows(window_ids: &[WindowId]) -> Vec<Result<(), Win32Error>> {
    // Ok(Some(hwnd)) marks a window that still needs a cloak request.
    let prepared: Vec<Result<Option<HWND>, Win32Error>> = window_ids
        .iter()
        .map(|&window_id| {
            let hwnd = window_id_to_hwnd(window_id)?;
            if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
                return Err(Win32Error::WindowNotFound(window_id));
            }
            if !needs_cloak_request(is_daemon_cloaked(window_id), is_window_cloaked(hwnd)) {
                return Ok(None);
            }
            track_daemon_cloak(window_id, window_process_identity(window_id));
            Ok(Some(hwnd))
        })
        .collect();
    flush_cloak_journal();

    prepared
        .into_iter()
        .zip(window_ids)
        .map(|(prepared, &window_id)| match prepared? {
            Some(hwnd) => cloak_tracked_window(window_id, hwnd),
            None => Ok(()),
        })
        .collect()
}

fn cloak_tracked_window(window_id: WindowId, hwnd: HWND) -> Result<(), Win32Error> {
    // The shell can cloak any window; DWM only windows of this process.
    let shell_pending = match shell_cloak::set_cloak(window_id, true) {
        Ok(()) => return Ok(()),
        Err(e) => {
            tracing::debug!(
                "Shell cloak failed for window {}: {}; trying DWM",
                window_id,
                e
            );
            matches!(e, ShellCloakError::Pending(_))
        }
    };
    match dwm_set_cloak(window_id, hwnd, true) {
        // The shell thread may have untracked the window after its own failure.
        Ok(()) => {
            track_daemon_cloak(window_id, window_process_identity(window_id));
            Ok(())
        }
        Err(e) => {
            // A pending shell request may still cloak the window; the shell
            // thread untracks it if that fails.
            if !shell_pending {
                untrack_daemon_cloak(window_id);
            }
            Err(e)
        }
    }
}

/// Cloak a window (hide from view but keep in Alt-Tab).
///
/// Cloaked windows are hidden visually but remain in the taskbar
/// and can still receive focus via Alt-Tab. Cloaking a window this process
/// already cloaked is a no-op.
pub fn cloak_window(hwnd: WindowId) -> Result<(), Win32Error> {
    cloak_windows(&[hwnd]).pop().unwrap_or(Ok(()))
}

/// Uncloak a window (make visible again).
///
/// Removes this process's shell cloak (if it cloaked the window) and any
/// application (DWM) cloak. Windows cloaked by the shell for other reasons,
/// such as being on another virtual desktop, are left alone.
pub fn uncloak_window(hwnd: WindowId) -> Result<(), Win32Error> {
    let window_id = hwnd;
    let hwnd = window_id_to_hwnd(window_id)?;
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        untrack_daemon_cloak(window_id);
        return Err(Win32Error::WindowNotFound(window_id));
    }

    // On success the shell thread untracks the window.
    let shell_error = if is_daemon_cloaked(window_id) {
        shell_cloak::set_cloak(window_id, false).err()
    } else {
        None
    };
    if is_window_app_cloaked(window_id) {
        dwm_set_cloak(window_id, hwnd, false)?;
    }
    match shell_error {
        None => {
            untrack_daemon_cloak(window_id);
            Ok(())
        }
        // Still queued: the window counts as cloaked until the shell completes.
        Some(e @ ShellCloakError::Pending(_)) => Err(Win32Error::CloakFailed(format!(
            "shell uncloak of window {}: {}",
            window_id, e
        ))),
        Some(e) if is_window_cloaked(hwnd) => Err(Win32Error::CloakFailed(format!(
            "shell uncloak failed for window {}: {}",
            window_id, e
        ))),
        // The shell failed, but nothing cloaks the window any more.
        Some(_) => {
            untrack_daemon_cloak(window_id);
            Ok(())
        }
    }
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
    flush_cloak_journal();

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
    flush_cloak_journal();
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
    use crate::test_support::{placement, GLOBAL_SENDER_TEST_LOCK};
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
    fn test_cloak_request_is_skipped_only_for_tracked_cloaked_windows() {
        assert!(!needs_cloak_request(true, true));
        // Tracked, but the cloak is gone (Explorer restarted): cloak again.
        assert!(needs_cloak_request(true, false));
        // Cloaked by someone else: this process must track its own cloak.
        assert!(needs_cloak_request(false, true));
        assert!(needs_cloak_request(false, false));
    }

    #[test]
    fn test_cloak_journal_format_round_trip_with_owner() {
        let owner = |pid, start_time| Some(ProcessIdentity { pid, start_time });
        let entries = vec![(3, owner(30, 300)), (1, None), (2, owner(20, 0))];
        let text = format_cloak_journal(&entries);
        assert_eq!(text, "1\n2 20 0\n3 30 300\n");
        assert_eq!(
            parse_cloak_journal(&text),
            vec![(1, None), (2, owner(20, 0)), (3, owner(30, 300))]
        );
    }

    #[test]
    fn test_cloak_journal_reads_old_format_and_skips_garbage() {
        // Bare handles from older versions; tolerates a partially written or edited file.
        assert_eq!(
            parse_cloak_journal("7\n\ngarbage\n0\n 9 \n12\n5 notapid\n6 60\n"),
            vec![
                (7, None),
                (9, None),
                (12, None),
                (
                    6,
                    Some(ProcessIdentity {
                        pid: 60,
                        start_time: 0
                    })
                )
            ]
        );
    }

    #[test]
    fn test_journal_entry_matches_only_the_recorded_owner() {
        let owner = |pid, start_time| Some(ProcessIdentity { pid, start_time });
        // Window gone.
        assert!(!journal_entry_matches(owner(1, 10), None));
        assert!(!journal_entry_matches(None, None));
        // Old format: any live window.
        assert!(journal_entry_matches(None, owner(5, 50)));
        assert!(journal_entry_matches(owner(1, 10), owner(1, 10)));
        // Handle reused by another process, or by a restarted process with the same pid.
        assert!(!journal_entry_matches(owner(1, 10), owner(2, 10)));
        assert!(!journal_entry_matches(owner(1, 10), owner(1, 11)));
        // Unknown start time on either side: the pid decides.
        assert!(journal_entry_matches(owner(1, 0), owner(1, 11)));
        assert!(journal_entry_matches(owner(1, 10), owner(1, 0)));
    }

    #[test]
    fn test_daemon_cloak_tracking_round_trip() {
        let _guard = GLOBAL_SENDER_TEST_LOCK
            .lock()
            .unwrap_or_else(recover_poisoned_mutex);
        let journal =
            std::env::temp_dir().join(format!("openniri-cloak-journal-{}.txt", std::process::id()));
        *CLOAK_JOURNAL_PATH_OVERRIDE
            .lock()
            .unwrap_or_else(recover_poisoned_mutex) = Some(journal.clone());

        let window_id: WindowId = 0xDEAD_0001;
        let owner = Some(ProcessIdentity {
            pid: 4242,
            start_time: 7,
        });
        assert!(!is_daemon_cloaked(window_id));
        assert!(track_daemon_cloak(window_id, owner));
        assert!(!track_daemon_cloak(window_id, None), "already tracked");
        assert!(is_daemon_cloaked(window_id));
        flush_cloak_journal();
        assert!(read_cloak_journal().contains(&(window_id, owner)));

        // A dead handle is dropped from tracking and journal by recovery.
        assert_eq!(restore_journaled_cloaked_windows(), 0);
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
