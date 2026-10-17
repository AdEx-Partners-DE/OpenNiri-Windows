//! Window enumeration, manageability filters and per-window/process queries.

use crate::error::Win32Error;
use openniri_core_layout::{Rect, WindowId};
use std::ffi::c_void;
use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, E_ACCESSDENIED, FILETIME, HANDLE, HWND, LPARAM, RECT,
    TRUE,
};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetAncestor, GetClassNameW, GetWindow, GetWindowLongW, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindow, IsWindowVisible,
    GA_ROOT, GWL_EXSTYLE, GWL_STYLE, GW_OWNER, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_VISIBLE,
};

/// Convert a WindowId to an HWND safely, returning an error for null (zero) IDs.
///
/// A WindowId of 0 would produce a null HWND pointer, which is invalid for
/// most Win32 window operations.
pub(crate) fn window_id_to_hwnd(id: WindowId) -> Result<HWND, Win32Error> {
    if id == 0 {
        return Err(Win32Error::WindowNotFound(id));
    }
    Ok(HWND(id as *mut c_void))
}

/// Information about a managed window.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// The window handle (HWND) as u64.
    pub hwnd: WindowId,
    /// Window title.
    pub title: String,
    /// Window class name.
    pub class_name: String,
    /// Process ID.
    pub process_id: u32,
    /// Current window rectangle.
    pub rect: Rect,
    /// Whether the window is visible.
    pub visible: bool,
}

/// Enumerate all top-level windows that should be managed.
///
/// Only windows that pass [`WindowFilter::MANAGEABLE`] and have a non-empty
/// rectangle are returned.
pub fn enumerate_windows() -> Result<Vec<WindowInfo>, Win32Error> {
    let mut windows: Vec<WindowInfo> = Vec::new();

    unsafe {
        // EnumWindows callback receives a raw pointer to our Vec
        let windows_ptr = &mut windows as *mut Vec<WindowInfo>;

        let result = EnumWindows(Some(enum_windows_callback), LPARAM(windows_ptr as isize));

        if result.is_err() {
            return Err(Win32Error::EnumerationFailed(
                "EnumWindows failed".to_string(),
            ));
        }
    }

    tracing::debug!("Enumerated {} manageable windows", windows.len());
    Ok(windows)
}

/// Callback for EnumWindows that filters and collects window info.
unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let windows = &mut *(lparam.0 as *mut Vec<WindowInfo>);

    let Some((title, class_name)) = manageable_window_names(hwnd, WindowFilter::MANAGEABLE) else {
        return TRUE;
    };

    let mut process_id: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut process_id));

    let mut win_rect = RECT::default();
    if GetWindowRect(hwnd, &mut win_rect).is_err() {
        return TRUE;
    }

    let rect = Rect::new(
        win_rect.left,
        win_rect.top,
        win_rect.right - win_rect.left,
        win_rect.bottom - win_rect.top,
    );

    // Skip zero-size windows
    if rect.width == 0 || rect.height == 0 {
        return TRUE;
    }

    windows.push(WindowInfo {
        hwnd: hwnd.0 as WindowId,
        title,
        class_name,
        process_id,
        rect,
        visible: true,
    });

    TRUE
}

/// Which manageability checks a top-level window must pass.
///
/// Enumeration and every event source share [`manageable_window_names`], so a
/// window the daemon would never manage produces no events either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowFilter {
    /// The window must be visible (`IsWindowVisible` and `WS_VISIBLE`).
    pub(crate) require_visible: bool,
    /// The window must have a title that is not on the skip list.
    pub(crate) require_title: bool,
    /// The window must not be cloaked (for example on another virtual desktop).
    pub(crate) filter_cloaked: bool,
}

impl WindowFilter {
    /// Every check: the windows the daemon tiles.
    pub(crate) const MANAGEABLE: Self = Self {
        require_visible: true,
        require_title: true,
        filter_cloaked: true,
    };
}

/// Title and class name of a window that passes `filter`, or `None`.
///
/// Always skipped: tool windows (unless `WS_EX_APPWINDOW`), `WS_EX_NOACTIVATE`
/// windows, owned windows (dialogs) and known system classes. The title is only
/// read (and is otherwise empty) when `filter.require_title` is set.
pub(crate) fn manageable_window_names(
    hwnd: HWND,
    filter: WindowFilter,
) -> Option<(String, String)> {
    // Keep this best-effort and non-panicking: it runs inside hook callbacks.
    unsafe {
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;

        if filter.require_visible && (!IsWindowVisible(hwnd).as_bool() || style & WS_VISIBLE.0 == 0)
        {
            return None;
        }

        let is_tool_window = ex_style & WS_EX_TOOLWINDOW.0 != 0;
        let is_app_window = ex_style & WS_EX_APPWINDOW.0 != 0;
        if is_tool_window && !is_app_window {
            return None;
        }

        // Tooltips, popups and other windows that never take focus.
        if ex_style & WS_EX_NOACTIVATE.0 != 0 {
            return None;
        }

        // Owned windows (dialogs, secondary windows) float with their owner.
        if let Ok(owner) = GetWindow(hwnd, GW_OWNER) {
            if !owner.is_invalid() {
                return None;
            }
        }

        if filter.filter_cloaked && is_window_cloaked(hwnd) {
            return None;
        }

        let mut title = String::new();
        if filter.require_title {
            let title_len = GetWindowTextLengthW(hwnd);
            if title_len <= 0 {
                return None;
            }
            let mut title_buf: Vec<u16> = vec![0; (title_len + 1) as usize];
            let actual_len = GetWindowTextW(hwnd, &mut title_buf);
            if actual_len <= 0 {
                return None;
            }
            title = String::from_utf16_lossy(&title_buf[..actual_len as usize]);
            if should_skip_window_by_title(&title) {
                return None;
            }
        }

        let mut class_buf: Vec<u16> = vec![0; 256];
        let class_len = GetClassNameW(hwnd, &mut class_buf).max(0) as usize;
        let class_name = String::from_utf16_lossy(&class_buf[..class_len]);
        if should_skip_window_by_class(&class_name) {
            return None;
        }

        Some((title, class_name))
    }
}

pub(crate) fn normalize_to_root_window(hwnd: HWND) -> HWND {
    let root_hwnd = unsafe { GetAncestor(hwnd, GA_ROOT) };
    if root_hwnd.0.is_null() {
        hwnd
    } else {
        root_hwnd
    }
}

/// Check if a window should be skipped based on its title.
fn should_skip_window_by_title(title: &str) -> bool {
    const SKIP_TITLES: &[&str] = &[
        "Program Manager",
        "Windows Input Experience",
        "Microsoft Text Input Application",
        "Settings",
        // Add more system window titles as needed
    ];

    SKIP_TITLES.contains(&title)
}

/// Check if a window is cloaked (hidden by DWM).
///
/// Cloaked windows should be skipped during enumeration since they're
/// not actually visible to the user (e.g., windows on other virtual desktops).
pub(crate) fn is_window_cloaked(hwnd: HWND) -> bool {
    unsafe {
        let mut cloaked: u32 = 0;
        let result = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        );
        match result {
            Ok(()) => cloaked != 0,
            Err(e) => {
                let window_is_valid = IsWindow(Some(hwnd)).as_bool();
                let treat_as_cloaked = should_treat_cloak_query_failure_as_cloaked(window_is_valid);
                tracing::debug!(
                    "DwmGetWindowAttribute(DWMWA_CLOAKED) failed for {:?}: {}. window_is_valid={} -> treat_as_cloaked={}",
                    hwnd,
                    e,
                    window_is_valid,
                    treat_as_cloaked
                );
                treat_as_cloaked
            }
        }
    }
}

fn should_treat_cloak_query_failure_as_cloaked(window_is_valid: bool) -> bool {
    !window_is_valid
}

/// Check if a window should be skipped based on its class name.
fn should_skip_window_by_class(class_name: &str) -> bool {
    const SKIP_CLASSES: &[&str] = &[
        "Progman",                    // Program Manager
        "Shell_TrayWnd",              // Taskbar
        "Shell_SecondaryTrayWnd",     // Secondary taskbar
        "WorkerW",                    // Desktop worker
        "Windows.UI.Core.CoreWindow", // UWP system windows
        // ApplicationFrameWindow removed: allows tiling UWP apps (Calculator, Photos, etc.)
        // Empty/cloaked UWP frames are already filtered by the cloaked window check.
        "XamlExplorerHostIslandWindow", // XAML islands
        "TopLevelWindowForOverflowXamlIsland", // Overflow islands
                                        // Add more system classes as needed
    ];

    SKIP_CLASSES.contains(&class_name)
}

// ============================================================================
// Process Information
// ============================================================================

/// Get the executable name for a process by PID.
///
/// Returns just the filename (e.g., "notepad.exe"), not the full path.
/// Returns None if the process cannot be accessed or doesn't exist.
pub fn get_process_executable(pid: u32) -> Option<String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let path = process_image_path(process);
    let _ = unsafe { CloseHandle(process) };
    path?.rsplit('\\').next().map(|s| s.to_string())
}

/// Full Win32 image path of a process handle opened with
/// `PROCESS_QUERY_LIMITED_INFORMATION`, including paths longer than `MAX_PATH`.
fn process_image_path(process: HANDLE) -> Option<String> {
    // Win32 paths are capped at 32767 characters.
    let mut capacity: usize = 1024;
    loop {
        let mut buffer: Vec<u16> = vec![0; capacity];
        let mut len = capacity as u32;
        let result = unsafe {
            QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut len,
            )
        };
        match result {
            Ok(()) => return Some(String::from_utf16_lossy(&buffer[..len as usize])),
            Err(e) if e.code() == ERROR_INSUFFICIENT_BUFFER.to_hresult() && capacity < 32_768 => {
                capacity *= 2;
            }
            Err(_) => return None,
        }
    }
}

/// Identity of the process that owns a window.
///
/// Window handles are reused (also across reboots), so a handle recorded by an
/// earlier daemon run only refers to the same window if its owner still matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProcessIdentity {
    pub(crate) pid: u32,
    /// Process creation time in FILETIME ticks; 0 when it could not be read.
    pub(crate) start_time: u64,
}

/// Owner identity of a live window, or `None` if the handle is not a window.
pub(crate) fn window_process_identity(window_id: WindowId) -> Option<ProcessIdentity> {
    let hwnd = window_id_to_hwnd(window_id).ok()?;
    let mut pid: u32 = 0;
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return None;
        }
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == 0 {
        return None;
    }
    Some(ProcessIdentity {
        pid,
        start_time: process_start_time(pid).unwrap_or(0),
    })
}

fn process_start_time(pid: u32) -> Option<u64> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let result =
        unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) };
    let _ = unsafe { CloseHandle(process) };
    result.ok()?;
    Some((u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
}

/// Mandatory integrity level RID (for example `0x2000` medium, `0x3000` high) of a process.
pub(crate) fn process_integrity_rid(process: HANDLE) -> Result<u32, windows::core::Error> {
    use windows::Win32::Security::{
        GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TokenIntegrityLevel,
        TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::OpenProcessToken;

    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token)?;

        let mut len: u32 = 0;
        let _ = GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut len);
        // u64 storage keeps the TOKEN_MANDATORY_LABEL read aligned.
        let mut buf = vec![0u64; (len as usize).div_ceil(8).max(1)];
        let result = GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buf.as_mut_ptr() as *mut c_void),
            len,
            &mut len,
        );
        let _ = CloseHandle(token);
        result?;

        let label = &*(buf.as_ptr() as *const TOKEN_MANDATORY_LABEL);
        let sid = label.Label.Sid;
        let count = *GetSidSubAuthorityCount(sid);
        if count == 0 {
            return Err(windows::core::Error::from(E_ACCESSDENIED));
        }
        Ok(*GetSidSubAuthority(sid, u32::from(count) - 1))
    }
}

/// Integrity level of this process, read once.
fn own_integrity_rid() -> Option<u32> {
    static OWN: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    *OWN.get_or_init(|| {
        process_integrity_rid(unsafe { windows::Win32::System::Threading::GetCurrentProcess() })
            .ok()
    })
}

/// Whether a process runs with a higher integrity level than this one.
///
/// Windows blocks moving, resizing and cloaking windows of more privileged
/// processes (UIPI), for example an administrator terminal when OpenNiri runs
/// unelevated. Such windows must not be tiled: every placement fails with
/// "Access is denied" (INC-49).
///
/// When the integrity level cannot be read, access denial decides: a process
/// whose handle (`PROCESS_QUERY_LIMITED_INFORMATION`) or token is refused with
/// `E_ACCESSDENIED` is treated as more privileged (protected and system
/// processes). Any other failure, such as a process that already exited, is
/// treated as not more privileged.
pub fn is_process_more_privileged(pid: u32) -> bool {
    let process = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(process) => process,
        Err(e) => return e.code() == E_ACCESSDENIED,
    };
    let theirs = process_integrity_rid(process);
    let _ = unsafe { CloseHandle(process) };
    match (theirs, own_integrity_rid()) {
        (Ok(theirs), Some(ours)) => theirs > ours,
        (Err(e), _) => e.code() == E_ACCESSDENIED,
        (Ok(_), None) => false,
    }
}

/// Title, class name and process id of a window, read directly.
///
/// Unlike [`enumerate_windows`], this does not skip cloaked or untitled windows,
/// so it also works for managed windows the daemon has cloaked.
pub fn describe_window(window_id: WindowId) -> Option<(String, String, u32)> {
    let hwnd = window_id_to_hwnd(window_id).ok()?;
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return None;
        }
        let mut title_buf: Vec<u16> = vec![0; 512];
        let title_len = GetWindowTextW(hwnd, &mut title_buf).max(0) as usize;
        let mut class_buf: Vec<u16> = vec![0; 256];
        let class_len = GetClassNameW(hwnd, &mut class_buf).max(0) as usize;
        let mut process_id: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        Some((
            String::from_utf16_lossy(&title_buf[..title_len]),
            String::from_utf16_lossy(&class_buf[..class_len]),
            process_id,
        ))
    }
}

/// Check if a window handle is still valid.
///
/// This helps prevent race conditions where a window is destroyed
/// between receiving an event and processing it.
pub fn is_valid_window(hwnd: WindowId) -> bool {
    if hwnd == 0 {
        return false;
    }
    unsafe {
        let hwnd = HWND(hwnd as *mut c_void);
        IsWindow(Some(hwnd)).as_bool()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skip_lists() {
        for (class, skip) in [
            ("Shell_TrayWnd", true),
            ("Progman", true),
            ("WorkerW", true),
            ("Windows.UI.Core.CoreWindow", true),
            // UWP apps (Calculator, Photos) must stay tileable.
            ("ApplicationFrameWindow", false),
            ("Notepad", false),
        ] {
            assert_eq!(should_skip_window_by_class(class), skip, "class {class}");
        }
        for (title, skip) in [
            ("Program Manager", true),
            ("Windows Input Experience", true),
            ("Settings - Notepad", false),
        ] {
            assert_eq!(should_skip_window_by_title(title), skip, "title {title}");
        }
    }

    #[test]
    fn test_own_process_is_not_more_privileged_than_itself() {
        assert!(own_integrity_rid().is_some());
        assert!(!is_process_more_privileged(std::process::id()));
    }

    #[test]
    fn test_own_process_executable_and_start_time() {
        let exe = get_process_executable(std::process::id()).expect("own executable");
        assert!(exe.to_ascii_lowercase().ends_with(".exe"), "{exe}");
        assert!(process_start_time(std::process::id()).is_some_and(|t| t > 0));
    }
}
