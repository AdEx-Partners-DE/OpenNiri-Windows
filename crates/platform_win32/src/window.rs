//! Window enumeration, manageability filters and per-window/process queries.

use crate::error::Win32Error;
use openniri_core_layout::{Rect, WindowId};
use std::ffi::c_void;
use windows::core::BOOL;
use windows::Win32::Foundation::{CloseHandle, E_ACCESSDENIED, HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::ProcessStatus::K32GetModuleFileNameExW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
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
/// Filters out:
/// - Invisible windows
/// - Tool windows (WS_EX_TOOLWINDOW without WS_EX_APPWINDOW)
/// - Windows with empty titles
/// - Cloaked windows
/// - Windows with WS_EX_NOACTIVATE
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

    // Skip invisible windows
    if !IsWindowVisible(hwnd).as_bool() {
        return TRUE;
    }

    // Get window styles
    let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;

    // Skip if not visible style
    if style & WS_VISIBLE.0 == 0 {
        return TRUE;
    }

    // Skip tool windows (unless they have WS_EX_APPWINDOW)
    let is_tool_window = ex_style & WS_EX_TOOLWINDOW.0 != 0;
    let is_app_window = ex_style & WS_EX_APPWINDOW.0 != 0;
    if is_tool_window && !is_app_window {
        return TRUE;
    }

    // Skip windows with WS_EX_NOACTIVATE (tooltips, popups, etc.)
    if ex_style & WS_EX_NOACTIVATE.0 != 0 {
        return TRUE;
    }

    // Skip owned windows (dialogs, secondary windows)
    if let Ok(owner) = GetWindow(hwnd, GW_OWNER) {
        if !owner.is_invalid() {
            return TRUE;
        }
    }

    // Skip cloaked windows (e.g., on other virtual desktops)
    if is_window_cloaked(hwnd) {
        return TRUE;
    }

    // Get window title
    let title_len = GetWindowTextLengthW(hwnd);
    if title_len == 0 {
        return TRUE; // Skip windows with no title
    }

    let mut title_buf: Vec<u16> = vec![0; (title_len + 1) as usize];
    let actual_len = GetWindowTextW(hwnd, &mut title_buf);
    if actual_len == 0 {
        return TRUE;
    }
    let title = String::from_utf16_lossy(&title_buf[..actual_len as usize]);

    // Skip known system windows by title
    if should_skip_window_by_title(&title) {
        return TRUE;
    }

    // Get class name
    let mut class_buf: Vec<u16> = vec![0; 256];
    let class_len = GetClassNameW(hwnd, &mut class_buf);
    let class_name = if class_len > 0 {
        String::from_utf16_lossy(&class_buf[..class_len as usize])
    } else {
        String::new()
    };

    // Skip known system classes
    if should_skip_window_by_class(&class_name) {
        return TRUE;
    }

    // Get process ID
    let mut process_id: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut process_id));

    // Get window rect
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

pub(crate) fn normalize_to_root_window(hwnd: HWND) -> HWND {
    let root_hwnd = unsafe { GetAncestor(hwnd, GA_ROOT) };
    if root_hwnd.0.is_null() {
        hwnd
    } else {
        root_hwnd
    }
}

/// Check if a window should be skipped based on its title.
pub(crate) fn should_skip_window_by_title(title: &str) -> bool {
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
pub(crate) fn should_skip_window_by_class(class_name: &str) -> bool {
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
    unsafe {
        // Open the process with limited query rights
        let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return None,
        };

        // Get the executable path
        let mut buffer: Vec<u16> = vec![0; 260]; // MAX_PATH
        let len = K32GetModuleFileNameExW(Some(handle), None, &mut buffer);

        // Close the handle
        let _ = CloseHandle(handle);

        if len == 0 {
            return None;
        }

        // Convert to string and extract filename
        let path = String::from_utf16_lossy(&buffer[..len as usize]);
        path.rsplit('\\').next().map(|s| s.to_string())
    }
}

/// Mandatory integrity level RID (for example `0x2000` medium, `0x3000` high) of a process.
fn process_integrity_rid(
    process: windows::Win32::Foundation::HANDLE,
) -> Result<u32, windows::core::Error> {
    use windows::Win32::Security::{
        GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TokenIntegrityLevel,
        TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::OpenProcessToken;

    unsafe {
        let mut token = windows::Win32::Foundation::HANDLE::default();
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
/// "Access is denied" (INC-49). A process whose token cannot be read at all is
/// treated as more privileged.
pub fn is_process_more_privileged(pid: u32) -> bool {
    let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
    else {
        return false;
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
}
