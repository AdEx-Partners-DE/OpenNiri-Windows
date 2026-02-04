//! OpenNiri Platform Win32
//!
//! Windows-specific window manipulation using Win32 APIs.
//!
//! This crate handles:
//! - Window enumeration and filtering
//! - Window positioning via SetWindowPos (with DeferWindowPos batching)
//! - Window cloaking/uncloaking via DWM APIs
//! - WinEvent hooks for window lifecycle events
//! - Visual overlay for snap hints

pub mod overlay;

use openniri_core_layout::{Rect, Visibility, WindowId, WindowPlacement};
use std::ffi::c_void;
use std::sync::mpsc;
use thiserror::Error;
use windows::Win32::Foundation::{BOOL, CloseHandle, HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DwmSetWindowAttribute, DWMWA_CLOAK, DWMWA_CLOAKED,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};
use windows::Win32::System::ProcessStatus::K32GetModuleFileNameExW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BeginDeferWindowPos, CallNextHookEx, CreateWindowExW, DeferWindowPos, DefWindowProcW,
    DispatchMessageW, EndDeferWindowPos, EnumWindows, GetAncestor, GetClassNameW, GetMessageW,
    GetWindowLongW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsWindow, IsWindowVisible, PostMessageW, RegisterClassW, SetWindowPos, SetWindowsHookExW,
    UnhookWindowsHookEx, WindowFromPoint, GA_ROOT, GWL_EXSTYLE, GWL_STYLE, HHOOK, HWND_MESSAGE,
    MSLLHOOKSTRUCT, MSG, SWP_NOACTIVATE, SWP_NOZORDER, WH_MOUSE_LL, WM_HOTKEY, WM_MOUSEMOVE,
    WM_USER, WNDCLASSW, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_VISIBLE,
};

// WinEvent constants (not all are exposed by windows-rs)
const EVENT_OBJECT_CREATE: u32 = 0x8000;
const EVENT_OBJECT_DESTROY: u32 = 0x8001;
const EVENT_OBJECT_FOCUS: u32 = 0x8005;
const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x0016;
const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x0017;
const EVENT_OBJECT_LOCATIONCHANGE: u32 = 0x800B;
const OBJID_WINDOW: i32 = 0;
const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;
const WINEVENT_SKIPOWNPROCESS: u32 = 0x0002;

// Window message for display configuration changes
const WM_DISPLAYCHANGE: u32 = 0x007E;

/// Errors that can occur during Win32 operations.
#[derive(Debug, Error)]
pub enum Win32Error {
    #[error("Failed to enumerate windows: {0}")]
    EnumerationFailed(String),

    #[error("Failed to enumerate monitors: {0}")]
    MonitorEnumerationFailed(String),

    #[error("Failed to set window position: {0}")]
    SetPositionFailed(String),

    #[error("Failed to cloak window: {0}")]
    CloakFailed(String),

    #[error("Failed to install event hook: {0}")]
    HookInstallFailed(String),

    #[error("Failed to register hotkey: {0}")]
    HotkeyRegistrationFailed(String),

    #[error("Window not found: {0}")]
    WindowNotFound(WindowId),
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

        let result = EnumWindows(
            Some(enum_windows_callback),
            LPARAM(windows_ptr as isize),
        );

        if result.is_err() {
            return Err(Win32Error::EnumerationFailed(
                "EnumWindows failed".to_string(),
            ));
        }
    }

    tracing::debug!("Enumerated {} manageable windows", windows.len());
    Ok(windows)
}

/// Get the primary monitor's information.
///
/// Returns the work area (excluding taskbar) which is suitable for window positioning.
pub fn get_primary_monitor() -> Result<MonitorInfo, Win32Error> {
    let monitors = enumerate_monitors()?;

    monitors
        .into_iter()
        .find(|m| m.is_primary)
        .ok_or_else(|| {
            Win32Error::MonitorEnumerationFailed("No primary monitor found".to_string())
        })
}

/// Find which monitor contains the center of a given rectangle.
///
/// Returns the monitor info if found, or None if no monitor contains the point.
/// Falls back to primary monitor if no exact match.
pub fn find_monitor_for_rect<'a>(monitors: &'a [MonitorInfo], rect: &Rect) -> Option<&'a MonitorInfo> {
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
fn is_window_cloaked(hwnd: HWND) -> bool {
    unsafe {
        let mut cloaked: u32 = 0;
        let result = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        );
        // If the call fails, assume not cloaked
        result.is_ok() && cloaked != 0
    }
}

/// Check if a window should be skipped based on its class name.
fn should_skip_window_by_class(class_name: &str) -> bool {
    const SKIP_CLASSES: &[&str] = &[
        "Progman",                          // Program Manager
        "Shell_TrayWnd",                    // Taskbar
        "Shell_SecondaryTrayWnd",           // Secondary taskbar
        "WorkerW",                          // Desktop worker
        "Windows.UI.Core.CoreWindow",       // UWP system windows
        "ApplicationFrameWindow",           // Some UWP containers
        "XamlExplorerHostIslandWindow",     // XAML islands
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

/// Check if a window handle is still valid.
///
/// This helps prevent race conditions where a window is destroyed
/// between receiving an event and processing it.
pub fn is_valid_window(hwnd: WindowId) -> bool {
    unsafe {
        let hwnd = HWND(hwnd as *mut c_void);
        IsWindow(Some(hwnd)).as_bool()
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

    // Apply positions for visible windows
    if !visible.is_empty() {
        if config.use_deferred_positioning {
            apply_placements_deferred(&visible)?;
        } else {
            apply_placements_immediate(&visible)?;
        }

        // Uncloak visible windows
        for placement in &visible {
            if let Err(e) = uncloak_window(placement.window_id) {
                tracing::warn!("Failed to uncloak window {}: {}", placement.window_id, e);
            }
        }
    }

    // Hide off-screen windows based on strategy
    match config.hide_strategy {
        HideStrategy::Cloak => {
            for placement in &offscreen {
                if let Err(e) = cloak_window(placement.window_id) {
                    tracing::warn!("Failed to cloak window {}: {}", placement.window_id, e);
                }
            }
        }
        HideStrategy::MoveOffScreen => {
            // Move windows far off-screen (don't cloak them)
            // They remain in Alt-Tab but aren't visible
            for placement in &offscreen {
                // Move to far off-screen position
                let offscreen_placement = WindowPlacement {
                    window_id: placement.window_id,
                    rect: Rect::new(-32000, -32000, placement.rect.width, placement.rect.height),
                    visibility: Visibility::OffScreenLeft,
                    column_index: placement.column_index,
                };
                if let Err(e) = set_window_pos_immediate(&offscreen_placement) {
                    tracing::warn!("Failed to move window {} off-screen: {}", placement.window_id, e);
                }
            }
        }
    }

    tracing::debug!(
        "Applied {} visible placements, {} off-screen",
        visible.len(),
        offscreen.len()
    );

    Ok(())
}

/// Apply placements using DeferWindowPos for batched positioning.
///
/// This function uses the Windows DeferWindowPos API to batch multiple
/// window positioning operations into a single screen update, reducing
/// flicker and improving performance.
///
/// If EndDeferWindowPos fails, falls back to individual SetWindowPos calls
/// for all windows. If individual DeferWindowPos calls fail during the batch,
/// those placements are tracked and retried individually after the batch.
fn apply_placements_deferred(placements: &[&WindowPlacement]) -> Result<(), Win32Error> {
    unsafe {
        let hdwp = BeginDeferWindowPos(placements.len() as i32)
            .map_err(|e| Win32Error::SetPositionFailed(format!("BeginDeferWindowPos failed: {}", e)))?;

        let mut current_hdwp = hdwp;
        let mut failed_placements: Vec<&WindowPlacement> = Vec::new();

        for placement in placements {
            let hwnd = HWND(placement.window_id as *mut c_void);
            let rect = &placement.rect;

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
            for placement in placements {
                if let Err(e) = set_window_pos_immediate(placement) {
                    tracing::warn!(
                        "Individual SetWindowPos also failed for {}: {}",
                        placement.window_id,
                        e
                    );
                }
            }
        } else {
            // Batch succeeded, now handle any that failed during deferral
            for placement in failed_placements {
                if let Err(e) = set_window_pos_immediate(placement) {
                    tracing::warn!(
                        "Fallback SetWindowPos failed for {}: {}",
                        placement.window_id,
                        e
                    );
                }
            }
        }
    }

    Ok(())
}

/// Apply placements using immediate SetWindowPos calls.
fn apply_placements_immediate(placements: &[&WindowPlacement]) -> Result<(), Win32Error> {
    for placement in placements {
        set_window_pos_immediate(placement)?;
    }
    Ok(())
}

/// Set window position immediately using SetWindowPos.
fn set_window_pos_immediate(placement: &WindowPlacement) -> Result<(), Win32Error> {
    unsafe {
        let hwnd = HWND(placement.window_id as *mut c_void);
        let rect = &placement.rect;

        SetWindowPos(
            hwnd,
            None, // No z-order change with SWP_NOZORDER
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
        .map_err(|e| {
            Win32Error::SetPositionFailed(format!(
                "SetWindowPos failed for window {}: {}",
                placement.window_id,
                e
            ))
        })?;
    }
    Ok(())
}

/// Cloak a window (hide from view but keep in Alt-Tab).
///
/// Cloaked windows are hidden visually but remain in the taskbar
/// and can still receive focus via Alt-Tab.
pub fn cloak_window(hwnd: WindowId) -> Result<(), Win32Error> {
    unsafe {
        let hwnd = HWND(hwnd as *mut c_void);
        let cloak_value: u32 = 1; // TRUE = cloak

        let result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CLOAK,
            &cloak_value as *const u32 as *const c_void,
            std::mem::size_of::<u32>() as u32,
        );

        if result.is_err() {
            return Err(Win32Error::CloakFailed(format!(
                "DwmSetWindowAttribute(CLOAK=1) failed for {:?}",
                hwnd
            )));
        }
    }
    Ok(())
}

/// Uncloak a window (make visible again).
pub fn uncloak_window(hwnd: WindowId) -> Result<(), Win32Error> {
    unsafe {
        let hwnd = HWND(hwnd as *mut c_void);
        let cloak_value: u32 = 0; // FALSE = uncloak

        let result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CLOAK,
            &cloak_value as *const u32 as *const c_void,
            std::mem::size_of::<u32>() as u32,
        );

        if result.is_err() {
            return Err(Win32Error::CloakFailed(format!(
                "DwmSetWindowAttribute(CLOAK=0) failed for {:?}",
                hwnd
            )));
        }
    }
    Ok(())
}

/// Window event types that the daemon needs to handle.
#[derive(Debug, Clone)]
pub enum WindowEvent {
    /// A new window was created.
    Created(WindowId),
    /// A window was destroyed.
    Destroyed(WindowId),
    /// A window received focus.
    Focused(WindowId),
    /// A window was minimized.
    Minimized(WindowId),
    /// A window was restored from minimized state.
    Restored(WindowId),
    /// A window was moved or resized by the user.
    MovedOrResized(WindowId),
    /// Display configuration changed (monitors added/removed/rearranged).
    DisplayChange,
    /// Mouse cursor entered a window (for focus-follows-mouse).
    MouseEnterWindow(WindowId),
}

/// Global sender for window events from WinEvent callbacks.
///
/// This uses a thread-safe channel because WinEvent callbacks run on Windows'
/// internal thread pool and we need to forward events to the async runtime.
static EVENT_SENDER: std::sync::OnceLock<mpsc::Sender<WindowEvent>> = std::sync::OnceLock::new();

/// Handle for installed event hooks.
///
/// Dropping this handle will unhook all installed event hooks.
pub struct EventHookHandle {
    hooks: Vec<HWINEVENTHOOK>,
}

impl Drop for EventHookHandle {
    fn drop(&mut self) {
        for hook in &self.hooks {
            unsafe {
                if !UnhookWinEvent(*hook).as_bool() {
                    tracing::warn!("Failed to unhook WinEvent: {:?}", hook);
                }
            }
        }
        tracing::debug!("Unhooked {} WinEvent hooks", self.hooks.len());
    }
}

/// Install WinEvent hooks to receive window lifecycle events.
///
/// Returns a handle that must be kept alive to receive events.
/// Also returns a receiver channel for the events.
///
/// # Events Hooked
/// - Window creation (EVENT_OBJECT_CREATE)
/// - Window destruction (EVENT_OBJECT_DESTROY)
/// - Foreground change (EVENT_SYSTEM_FOREGROUND)
/// - Minimize/restore (EVENT_SYSTEM_MINIMIZESTART/END)
/// - Move/resize (EVENT_OBJECT_LOCATIONCHANGE)
pub fn install_event_hooks() -> Result<(EventHookHandle, mpsc::Receiver<WindowEvent>), Win32Error> {
    // Create channel for events
    let (tx, rx) = mpsc::channel();

    // Store sender globally for callback access
    EVENT_SENDER
        .set(tx)
        .map_err(|_| Win32Error::HookInstallFailed("Event sender already initialized".to_string()))?;

    let mut hooks = Vec::new();

    // Define events to hook: (min_event, max_event)
    let event_ranges = [
        (EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY),      // Create/Destroy
        (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND), // Foreground
        (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND), // Minimize
        (EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE), // Move/Resize
        (EVENT_OBJECT_FOCUS, EVENT_OBJECT_FOCUS),         // Focus within app
    ];

    unsafe {
        for (min_event, max_event) in event_ranges {
            let hook = SetWinEventHook(
                min_event,
                max_event,
                None,                           // No DLL, use callback
                Some(win_event_callback),       // Our callback
                0,                              // All processes
                0,                              // All threads
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );

            if hook.is_invalid() {
                // Clean up any hooks we've installed
                for h in &hooks {
                    let _ = UnhookWinEvent(*h);
                }
                return Err(Win32Error::HookInstallFailed(format!(
                    "SetWinEventHook failed for events {}-{}",
                    min_event, max_event
                )));
            }

            hooks.push(hook);
        }
    }

    tracing::info!("Installed {} WinEvent hooks", hooks.len());
    Ok((EventHookHandle { hooks }, rx))
}

/// Callback function for WinEvent hooks.
///
/// This runs on Windows' thread pool, so we forward events to the channel.
/// Wrapped with catch_unwind to prevent panics from crashing the application.
unsafe extern "system" fn win_event_callback(
    hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    id_event_thread: u32,
    dwms_event_time: u32,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        win_event_callback_inner(hook, event, hwnd, id_object, id_child, id_event_thread, dwms_event_time)
    }));

    if let Err(e) = result {
        // Can't use tracing here safely in all contexts, use eprintln
        eprintln!("Panic in win_event_callback: {:?}", e);
    }
}

/// Inner implementation of WinEvent callback.
fn win_event_callback_inner(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    // Only handle window-level events (not child objects like menus)
    if id_object != OBJID_WINDOW {
        return;
    }

    // Ignore invalid HWNDs
    if hwnd.0.is_null() {
        return;
    }

    // Get the top-level window (in case we got a child window event)
    let root_hwnd = unsafe { GetAncestor(hwnd, GA_ROOT) };
    let hwnd = if root_hwnd.0.is_null() { hwnd } else { root_hwnd };

    let window_id = hwnd.0 as WindowId;

    // Map event to our WindowEvent type
    let window_event = match event {
        EVENT_OBJECT_CREATE => {
            // Quick filter: skip windows that don't look manageable
            if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
                return;
            }
            WindowEvent::Created(window_id)
        }
        EVENT_OBJECT_DESTROY => WindowEvent::Destroyed(window_id),
        EVENT_SYSTEM_FOREGROUND | EVENT_OBJECT_FOCUS => WindowEvent::Focused(window_id),
        EVENT_SYSTEM_MINIMIZESTART => WindowEvent::Minimized(window_id),
        EVENT_SYSTEM_MINIMIZEEND => WindowEvent::Restored(window_id),
        EVENT_OBJECT_LOCATIONCHANGE => {
            // Only track visible windows
            if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
                return;
            }
            WindowEvent::MovedOrResized(window_id)
        }
        _ => return,
    };

    // Send event through channel
    if let Some(sender) = EVENT_SENDER.get() {
        // Use try_send to avoid blocking if channel is full
        let _ = sender.send(window_event);
    }
}

// ============================================================================
// Global Hotkey Support
// ============================================================================

/// Unique identifier for a registered hotkey.
pub type HotkeyId = i32;

/// Keyboard modifiers for hotkeys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

impl Modifiers {
    /// Create modifiers with only the Win key.
    pub fn win() -> Self {
        Self { win: true, ..Default::default() }
    }

    /// Create modifiers with Win + Shift.
    pub fn win_shift() -> Self {
        Self { win: true, shift: true, ..Default::default() }
    }

    /// Create modifiers with Alt.
    pub fn alt() -> Self {
        Self { alt: true, ..Default::default() }
    }

    /// Convert to Win32 HOT_KEY_MODIFIERS flags.
    pub fn to_win32(&self) -> HOT_KEY_MODIFIERS {
        let mut mods = MOD_NOREPEAT; // Prevent key repeat
        if self.ctrl {
            mods |= MOD_CONTROL;
        }
        if self.alt {
            mods |= MOD_ALT;
        }
        if self.shift {
            mods |= MOD_SHIFT;
        }
        if self.win {
            mods |= MOD_WIN;
        }
        mods
    }
}

/// A hotkey definition with modifiers and virtual key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    /// The unique ID for this hotkey.
    pub id: HotkeyId,
    /// Modifier keys (Ctrl, Alt, Shift, Win).
    pub modifiers: Modifiers,
    /// Virtual key code (e.g., 'H' = 0x48).
    pub vk: u32,
}

impl Hotkey {
    /// Create a new hotkey definition.
    pub fn new(id: HotkeyId, modifiers: Modifiers, vk: u32) -> Self {
        Self { id, modifiers, vk }
    }
}

/// Event emitted when a hotkey is pressed.
#[derive(Debug, Clone, Copy)]
pub struct HotkeyEvent {
    /// The ID of the hotkey that was pressed.
    pub id: HotkeyId,
}

/// Global sender for hotkey events.
/// Uses Mutex to allow re-registration after dropping previous HotkeyHandle.
static HOTKEY_SENDER: std::sync::Mutex<Option<mpsc::Sender<HotkeyEvent>>> =
    std::sync::Mutex::new(None);

/// Global sender for display change events forwarded to window event channel.
/// Uses Mutex to allow re-registration after dropping previous EventHookHandle.
static DISPLAY_CHANGE_SENDER: std::sync::Mutex<Option<mpsc::Sender<WindowEvent>>> =
    std::sync::Mutex::new(None);

/// Custom message to signal the hotkey thread to stop.
const WM_QUIT_HOTKEY_THREAD: u32 = WM_USER + 1;

/// Handle for the hotkey message window and thread.
///
/// Dropping this handle will unregister all hotkeys and stop the message loop.
pub struct HotkeyHandle {
    hwnd: HWND,
    thread: Option<std::thread::JoinHandle<()>>,
    registered_ids: Vec<HotkeyId>,
}

impl HotkeyHandle {
    /// Returns the number of successfully registered hotkeys.
    pub fn registered_count(&self) -> usize {
        self.registered_ids.len()
    }
}

impl Drop for HotkeyHandle {
    fn drop(&mut self) {
        // Unregister all hotkeys
        unsafe {
            for id in &self.registered_ids {
                let _ = UnregisterHotKey(Some(self.hwnd), *id);
            }
        }
        tracing::debug!("Unregistered {} hotkeys", self.registered_ids.len());

        // Signal the message loop to quit
        unsafe {
            let _ = PostMessageW(
                Some(self.hwnd),
                WM_QUIT_HOTKEY_THREAD,
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            );
        }

        // Wait for thread to finish
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }

        // Clear the global senders to allow re-registration
        if let Ok(mut sender) = HOTKEY_SENDER.lock() {
            *sender = None;
        }
        if let Ok(mut sender) = DISPLAY_CHANGE_SENDER.lock() {
            *sender = None;
        }
    }
}

/// Register a sender for display change events.
///
/// This allows the hotkey window to forward WM_DISPLAYCHANGE messages
/// to the window event channel. Call this before `register_hotkeys`.
pub fn set_display_change_sender(sender: mpsc::Sender<WindowEvent>) -> Result<(), Win32Error> {
    let mut guard = DISPLAY_CHANGE_SENDER
        .lock()
        .map_err(|_| Win32Error::HookInstallFailed("Display change sender mutex poisoned".to_string()))?;
    *guard = Some(sender);
    Ok(())
}

/// Register global hotkeys and start listening for them.
///
/// Returns a handle that must be kept alive to receive hotkey events,
/// and a channel receiver for hotkey events.
///
/// # Arguments
/// * `hotkeys` - List of hotkeys to register
///
/// # Returns
/// * Handle to manage the hotkeys (drop to unregister)
/// * Receiver for hotkey press events
pub fn register_hotkeys(
    hotkeys: Vec<Hotkey>,
) -> Result<(HotkeyHandle, mpsc::Receiver<HotkeyEvent>), Win32Error> {
    // Create channel for events
    let (tx, rx) = mpsc::channel();

    // Store sender globally (check that it's not already set)
    {
        let mut sender = HOTKEY_SENDER
            .lock()
            .map_err(|_| Win32Error::HotkeyRegistrationFailed("Hotkey sender mutex poisoned".to_string()))?;
        if sender.is_some() {
            return Err(Win32Error::HotkeyRegistrationFailed(
                "Hotkey sender already initialized - drop existing HotkeyHandle first".to_string(),
            ));
        }
        *sender = Some(tx);
    }

    // Create the message window and register hotkeys on a separate thread
    // We send isize (raw pointer value) instead of HWND because HWND is !Send
    let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<(isize, Vec<HotkeyId>), Win32Error>>();
    let hotkeys_clone = hotkeys.clone();

    let thread = std::thread::spawn(move || {
        unsafe {
            // Register window class
            let class_name: Vec<u16> = "OpenNiriHotkeyClass\0".encode_utf16().collect();
            let wc = WNDCLASSW {
                lpfnWndProc: Some(hotkey_window_proc),
                lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            RegisterClassW(&wc);

            // Create message-only window
            let hwnd = CreateWindowExW(
                Default::default(),
                windows::core::PCWSTR(class_name.as_ptr()),
                None,
                Default::default(),
                0, 0, 0, 0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            );

            if hwnd.is_err() {
                let _ = init_tx.send(Err(Win32Error::HotkeyRegistrationFailed(
                    "Failed to create message window".to_string(),
                )));
                return;
            }

            let hwnd = hwnd.unwrap();
            let mut registered_ids = Vec::new();

            // Register all hotkeys
            for hotkey in &hotkeys_clone {
                let result = RegisterHotKey(
                    Some(hwnd),
                    hotkey.id,
                    hotkey.modifiers.to_win32(),
                    hotkey.vk,
                );

                if result.is_ok() {
                    registered_ids.push(hotkey.id);
                    tracing::debug!("Registered hotkey {} (vk=0x{:X})", hotkey.id, hotkey.vk);
                } else {
                    tracing::warn!(
                        "Failed to register hotkey {} (vk=0x{:X}) - may be in use",
                        hotkey.id,
                        hotkey.vk
                    );
                }
            }

            // Send initialization result (hwnd as isize for Send safety)
            let hwnd_raw = hwnd.0 as isize;
            let _ = init_tx.send(Ok((hwnd_raw, registered_ids)));

            // Message loop
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, Some(hwnd), 0, 0).as_bool() {
                if msg.message == WM_QUIT_HOTKEY_THREAD {
                    break;
                }
                let _ = DispatchMessageW(&msg);
            }
        }
    });

    // Wait for initialization
    let (hwnd_raw, registered_ids) = init_rx
        .recv()
        .map_err(|_| Win32Error::HotkeyRegistrationFailed("Thread initialization failed".to_string()))??;

    // Reconstruct HWND from raw pointer
    let hwnd = HWND(hwnd_raw as *mut c_void);

    if registered_ids.is_empty() && !hotkeys.is_empty() {
        tracing::warn!("No hotkeys were successfully registered");
    } else {
        tracing::info!(
            "Registered {}/{} hotkeys",
            registered_ids.len(),
            hotkeys.len()
        );
    }

    Ok((
        HotkeyHandle {
            hwnd,
            thread: Some(thread),
            registered_ids,
        },
        rx,
    ))
}

/// Window procedure for the hotkey message window.
///
/// Wrapped with catch_unwind to prevent panics from crashing the application.
unsafe extern "system" fn hotkey_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    // Wrap in catch_unwind to prevent panics from crashing
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        hotkey_window_proc_inner(hwnd, msg, wparam, lparam)
    }));

    match result {
        Ok(lresult) => lresult,
        Err(e) => {
            tracing::error!("Panic in hotkey_window_proc: {:?}", e);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

/// Inner implementation of hotkey window procedure.
fn hotkey_window_proc_inner(
    hwnd: HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    match msg {
        WM_HOTKEY => {
            let hotkey_id = wparam.0 as HotkeyId;
            tracing::debug!("Hotkey {} pressed", hotkey_id);

            // Send event through channel
            if let Ok(sender_guard) = HOTKEY_SENDER.lock() {
                if let Some(sender) = sender_guard.as_ref() {
                    let _ = sender.send(HotkeyEvent { id: hotkey_id });
                }
            }

            windows::Win32::Foundation::LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            tracing::info!("Display configuration changed (WM_DISPLAYCHANGE)");

            // Send display change event through window event channel
            if let Ok(sender_guard) = DISPLAY_CHANGE_SENDER.lock() {
                if let Some(sender) = sender_guard.as_ref() {
                    let _ = sender.send(WindowEvent::DisplayChange);
                }
            }

            windows::Win32::Foundation::LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Common virtual key codes for hotkey registration.
pub mod vk {
    // Letters
    pub const A: u32 = 0x41;
    pub const B: u32 = 0x42;
    pub const C: u32 = 0x43;
    pub const D: u32 = 0x44;
    pub const E: u32 = 0x45;
    pub const F: u32 = 0x46;
    pub const G: u32 = 0x47;
    pub const H: u32 = 0x48;
    pub const I: u32 = 0x49;
    pub const J: u32 = 0x4A;
    pub const K: u32 = 0x4B;
    pub const L: u32 = 0x4C;
    pub const M: u32 = 0x4D;
    pub const N: u32 = 0x4E;
    pub const O: u32 = 0x4F;
    pub const P: u32 = 0x50;
    pub const Q: u32 = 0x51;
    pub const R: u32 = 0x52;
    pub const S: u32 = 0x53;
    pub const T: u32 = 0x54;
    pub const U: u32 = 0x55;
    pub const V: u32 = 0x56;
    pub const W: u32 = 0x57;
    pub const X: u32 = 0x58;
    pub const Y: u32 = 0x59;
    pub const Z: u32 = 0x5A;

    // Numbers
    pub const N0: u32 = 0x30;
    pub const N1: u32 = 0x31;
    pub const N2: u32 = 0x32;
    pub const N3: u32 = 0x33;
    pub const N4: u32 = 0x34;
    pub const N5: u32 = 0x35;
    pub const N6: u32 = 0x36;
    pub const N7: u32 = 0x37;
    pub const N8: u32 = 0x38;
    pub const N9: u32 = 0x39;

    // Function keys
    pub const F1: u32 = 0x70;
    pub const F2: u32 = 0x71;
    pub const F3: u32 = 0x72;
    pub const F4: u32 = 0x73;
    pub const F5: u32 = 0x74;
    pub const F6: u32 = 0x75;
    pub const F7: u32 = 0x76;
    pub const F8: u32 = 0x77;
    pub const F9: u32 = 0x78;
    pub const F10: u32 = 0x79;
    pub const F11: u32 = 0x7A;
    pub const F12: u32 = 0x7B;

    // Navigation
    pub const LEFT: u32 = 0x25;
    pub const UP: u32 = 0x26;
    pub const RIGHT: u32 = 0x27;
    pub const DOWN: u32 = 0x28;

    // Other
    pub const TAB: u32 = 0x09;
    pub const SPACE: u32 = 0x20;
    pub const ENTER: u32 = 0x0D;
    pub const ESCAPE: u32 = 0x1B;

    // Punctuation (for common shortcuts)
    pub const MINUS: u32 = 0xBD;      // '-'
    pub const EQUALS: u32 = 0xBB;     // '='
    pub const BRACKET_LEFT: u32 = 0xDB;   // '['
    pub const BRACKET_RIGHT: u32 = 0xDD;  // ']'
    pub const COMMA: u32 = 0xBC;      // ','
    pub const PERIOD: u32 = 0xBE;     // '.'
}

/// Parse a virtual key code from a key name string.
///
/// Supports single letters (A-Z), numbers (0-9), function keys (F1-F12),
/// and special keys (Left, Right, Up, Down, Tab, Space, Enter, Escape).
pub fn parse_vk(key: &str) -> Option<u32> {
    let key = key.trim().to_uppercase();

    // Single letter
    if key.len() == 1 {
        let c = key.chars().next()?;
        if c.is_ascii_uppercase() {
            return Some(c as u32);
        }
        if c.is_ascii_digit() {
            return Some(c as u32);
        }
    }

    // Function keys
    if key.starts_with('F') && key.len() <= 3 {
        if let Ok(n) = key[1..].parse::<u32>() {
            if (1..=12).contains(&n) {
                return Some(0x6F + n); // F1=0x70, F2=0x71, ...
            }
        }
    }

    // Named keys
    match key.as_str() {
        "LEFT" => Some(vk::LEFT),
        "RIGHT" => Some(vk::RIGHT),
        "UP" => Some(vk::UP),
        "DOWN" => Some(vk::DOWN),
        "TAB" => Some(vk::TAB),
        "SPACE" => Some(vk::SPACE),
        "ENTER" | "RETURN" => Some(vk::ENTER),
        "ESCAPE" | "ESC" => Some(vk::ESCAPE),
        "MINUS" | "-" => Some(vk::MINUS),
        "EQUALS" | "PLUS" | "=" => Some(vk::EQUALS),
        _ => None,
    }
}

/// Parse a hotkey string like "Win+H" or "Ctrl+Alt+Left".
///
/// Returns modifiers and virtual key code if valid.
pub fn parse_hotkey_string(s: &str) -> Option<(Modifiers, u32)> {
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut modifiers = Modifiers::default();

    // Last part is the key, rest are modifiers
    for part in &parts[..parts.len() - 1] {
        match part.to_uppercase().as_str() {
            "CTRL" | "CONTROL" => modifiers.ctrl = true,
            "ALT" => modifiers.alt = true,
            "SHIFT" => modifiers.shift = true,
            "WIN" | "SUPER" | "META" => modifiers.win = true,
            _ => return None, // Unknown modifier
        }
    }

    // Parse the key
    let key = parts.last()?;
    let vk = parse_vk(key)?;

    Some((modifiers, vk))
}

// ============================================================================
// Touchpad Gesture Support
// ============================================================================

/// Gesture events detected from touchpad/pointer input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureEvent {
    /// Three-finger swipe left
    SwipeLeft,
    /// Three-finger swipe right
    SwipeRight,
    /// Three-finger swipe up
    SwipeUp,
    /// Three-finger swipe down
    SwipeDown,
}

/// Threshold in pixels for detecting a swipe gesture.
const GESTURE_THRESHOLD: i32 = 50;

/// Gesture detection state.
#[derive(Default)]
struct GestureState {
    /// Starting X position of the gesture
    start_x: i32,
    /// Starting Y position of the gesture
    start_y: i32,
    /// Whether a gesture is currently in progress
    active: bool,
}

/// Global sender for gesture events.
static GESTURE_SENDER: std::sync::Mutex<Option<mpsc::Sender<GestureEvent>>> =
    std::sync::Mutex::new(None);

/// Global gesture detection state.
static GESTURE_STATE: std::sync::Mutex<GestureState> =
    std::sync::Mutex::new(GestureState { start_x: 0, start_y: 0, active: false });

/// Handle for gesture detection.
///
/// Dropping this handle will stop gesture detection.
pub struct GestureHandle {
    hwnd: HWND,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for GestureHandle {
    fn drop(&mut self) {
        // Signal the message loop to quit
        unsafe {
            let _ = PostMessageW(
                Some(self.hwnd),
                WM_QUIT_HOTKEY_THREAD, // Reuse quit message
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            );
        }

        // Wait for thread to finish
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }

        // Clear the global sender
        if let Ok(mut sender) = GESTURE_SENDER.lock() {
            *sender = None;
        }

        tracing::debug!("Gesture detection stopped");
    }
}

/// WM_POINTER message constants (not all exposed by windows-rs)
const WM_POINTERDOWN: u32 = 0x0246;
#[allow(dead_code)] // Reserved for future pointer tracking
const WM_POINTERUPDATE: u32 = 0x0245;
const WM_POINTERUP: u32 = 0x0247;

/// Register for pointer input and start gesture detection.
///
/// Returns a handle that must be kept alive to receive gesture events,
/// and a channel receiver for gesture events.
///
/// Note: This uses a simplified approach - for production use, consider
/// using the Windows Precision Touchpad gesture API or raw input.
pub fn register_gestures() -> Result<(GestureHandle, mpsc::Receiver<GestureEvent>), Win32Error> {
    // Create channel for events
    let (tx, rx) = mpsc::channel();

    // Store sender globally
    {
        let mut sender = GESTURE_SENDER
            .lock()
            .map_err(|_| Win32Error::HookInstallFailed("Gesture sender mutex poisoned".to_string()))?;
        if sender.is_some() {
            return Err(Win32Error::HookInstallFailed(
                "Gesture sender already initialized - drop existing GestureHandle first".to_string(),
            ));
        }
        *sender = Some(tx);
    }

    // Create the message window on a separate thread
    let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<isize, Win32Error>>();

    let thread = std::thread::spawn(move || {
        unsafe {
            // Register window class for gesture detection
            let class_name: Vec<u16> = "OpenNiriGestureClass\0".encode_utf16().collect();
            let wc = WNDCLASSW {
                lpfnWndProc: Some(gesture_window_proc),
                lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            RegisterClassW(&wc);

            // Create a transparent, click-through overlay window for gesture detection
            // In practice, we'd use raw input or a low-level hook instead
            let hwnd = CreateWindowExW(
                Default::default(),
                windows::core::PCWSTR(class_name.as_ptr()),
                None,
                Default::default(),
                0, 0, 0, 0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            );

            if hwnd.is_err() {
                let _ = init_tx.send(Err(Win32Error::HookInstallFailed(
                    "Failed to create gesture message window".to_string(),
                )));
                return;
            }

            let hwnd = hwnd.unwrap();
            let hwnd_raw = hwnd.0 as isize;
            let _ = init_tx.send(Ok(hwnd_raw));

            // Message loop
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, Some(hwnd), 0, 0).as_bool() {
                if msg.message == WM_QUIT_HOTKEY_THREAD {
                    break;
                }
                let _ = DispatchMessageW(&msg);
            }
        }
    });

    // Wait for initialization
    let hwnd_raw = init_rx
        .recv()
        .map_err(|_| Win32Error::HookInstallFailed("Gesture thread initialization failed".to_string()))??;

    let hwnd = HWND(hwnd_raw as *mut c_void);

    tracing::info!("Gesture detection registered");

    Ok((
        GestureHandle {
            hwnd,
            thread: Some(thread),
        },
        rx,
    ))
}

/// Window procedure for gesture detection window.
///
/// Wrapped with catch_unwind to prevent panics from crashing the application.
unsafe extern "system" fn gesture_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    // Wrap in catch_unwind to prevent panics from crashing
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        gesture_window_proc_inner(hwnd, msg, wparam, lparam)
    }));

    match result {
        Ok(lresult) => lresult,
        Err(e) => {
            tracing::error!("Panic in gesture_window_proc: {:?}", e);
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

/// Inner implementation of gesture window procedure.
fn gesture_window_proc_inner(
    hwnd: HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    let _ = wparam; // Unused in current implementation
    match msg {
        WM_POINTERDOWN => {
            // Extract pointer position
            let x = (lparam.0 as i32) & 0xFFFF;
            let y = ((lparam.0 as i32) >> 16) & 0xFFFF;

            if let Ok(mut state) = GESTURE_STATE.lock() {
                state.start_x = x;
                state.start_y = y;
                state.active = true;
            }

            windows::Win32::Foundation::LRESULT(0)
        }
        WM_POINTERUP => {
            if let Ok(mut state) = GESTURE_STATE.lock() {
                if state.active {
                    // Extract end position
                    let x = (lparam.0 as i32) & 0xFFFF;
                    let y = ((lparam.0 as i32) >> 16) & 0xFFFF;

                    let delta_x = x - state.start_x;
                    let delta_y = y - state.start_y;

                    // Detect swipe direction
                    let gesture = if delta_x.abs() > delta_y.abs() {
                        // Horizontal swipe
                        if delta_x.abs() > GESTURE_THRESHOLD {
                            if delta_x < 0 {
                                Some(GestureEvent::SwipeLeft)
                            } else {
                                Some(GestureEvent::SwipeRight)
                            }
                        } else {
                            None
                        }
                    } else {
                        // Vertical swipe
                        if delta_y.abs() > GESTURE_THRESHOLD {
                            if delta_y < 0 {
                                Some(GestureEvent::SwipeUp)
                            } else {
                                Some(GestureEvent::SwipeDown)
                            }
                        } else {
                            None
                        }
                    };

                    // Send gesture event
                    if let Some(event) = gesture {
                        if let Ok(sender_guard) = GESTURE_SENDER.lock() {
                            if let Some(sender) = sender_guard.as_ref() {
                                let _ = sender.send(event);
                            }
                        }
                    }

                    state.active = false;
                }
            }

            windows::Win32::Foundation::LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

// ============================================================================
// Focus Follows Mouse (Low-Level Mouse Hook)
// ============================================================================

/// Global sender for mouse enter events.
static MOUSE_EVENT_SENDER: std::sync::Mutex<Option<mpsc::Sender<WindowEvent>>> =
    std::sync::Mutex::new(None);

/// Track the window the mouse is currently over.
static CURRENT_MOUSE_WINDOW: std::sync::Mutex<Option<WindowId>> = std::sync::Mutex::new(None);

/// Handle for the low-level mouse hook.
///
/// Dropping this handle will unhook the mouse hook.
pub struct MouseHookHandle {
    hook: HHOOK,
}

impl Drop for MouseHookHandle {
    fn drop(&mut self) {
        unsafe {
            if !self.hook.is_invalid() {
                let _ = UnhookWindowsHookEx(self.hook);
            }
        }
        tracing::debug!("Mouse hook uninstalled");

        // Clear the global sender
        if let Ok(mut sender) = MOUSE_EVENT_SENDER.lock() {
            *sender = None;
        }
    }
}

/// Install a low-level mouse hook for focus-follows-mouse functionality.
///
/// Returns a handle that must be kept alive to receive mouse events,
/// and registers the given sender to receive MouseEnterWindow events.
///
/// # Arguments
/// * `event_sender` - Sender for WindowEvent (specifically MouseEnterWindow)
pub fn install_mouse_hook(
    event_sender: mpsc::Sender<WindowEvent>,
) -> Result<MouseHookHandle, Win32Error> {
    // Store sender globally
    {
        let mut sender = MOUSE_EVENT_SENDER
            .lock()
            .map_err(|_| Win32Error::HookInstallFailed("Mouse sender mutex poisoned".to_string()))?;
        if sender.is_some() {
            return Err(Win32Error::HookInstallFailed(
                "Mouse sender already initialized - drop existing MouseHookHandle first".to_string(),
            ));
        }
        *sender = Some(event_sender);
    }

    // Install low-level mouse hook
    let hook = unsafe {
        SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(mouse_ll_hook_proc),
            None,
            0,
        )
        .map_err(|e| Win32Error::HookInstallFailed(format!("SetWindowsHookExW failed: {}", e)))?
    };

    tracing::info!("Low-level mouse hook installed for focus-follows-mouse");

    Ok(MouseHookHandle { hook })
}

/// Low-level mouse hook callback.
///
/// Tracks mouse movement and sends MouseEnterWindow events when the cursor
/// enters a different window.
unsafe extern "system" fn mouse_ll_hook_proc(
    ncode: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    // If ncode < 0, we must call CallNextHookEx without processing
    if ncode < 0 {
        return CallNextHookEx(None, ncode, wparam, lparam);
    }

    // Only process mouse move events
    if wparam.0 as u32 == WM_MOUSEMOVE {
        // Get the mouse position from the hook struct
        let mouse_struct = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let point = mouse_struct.pt;

        // Find the window at the cursor position
        let hwnd = WindowFromPoint(point);

        if !hwnd.is_invalid() {
            let window_id = hwnd.0 as WindowId;

            // Check if this is a different window than before
            if let Ok(mut current) = CURRENT_MOUSE_WINDOW.lock() {
                if *current != Some(window_id) {
                    *current = Some(window_id);

                    // Send MouseEnterWindow event
                    if let Ok(sender_guard) = MOUSE_EVENT_SENDER.lock() {
                        if let Some(sender) = sender_guard.as_ref() {
                            let _ = sender.send(WindowEvent::MouseEnterWindow(window_id));
                        }
                    }
                }
            }
        }
    }

    // Always call next hook in the chain
    CallNextHookEx(None, ncode, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_config_default() {
        let config = PlatformConfig::default();
        assert_eq!(config.hide_strategy, HideStrategy::Cloak);
        assert!(config.use_deferred_positioning);
    }

    #[test]
    #[ignore = "Requires display hardware - run with: cargo test -- --ignored"]
    fn test_enumerate_monitors() {
        let result = enumerate_monitors();
        if let Ok(monitors) = result {
            assert!(!monitors.is_empty(), "At least one monitor should exist");
            for monitor in &monitors {
                assert!(monitor.rect.width > 0, "Monitor width should be positive");
                assert!(monitor.rect.height > 0, "Monitor height should be positive");
                assert!(
                    monitor.work_area.width > 0,
                    "Work area width should be positive"
                );
                assert!(
                    monitor.work_area.height > 0,
                    "Work area height should be positive"
                );
            }
        }
    }

    #[test]
    #[ignore = "Requires display hardware - run with: cargo test -- --ignored"]
    fn test_get_primary_monitor() {
        let result = get_primary_monitor();
        if let Ok(primary) = result {
            assert!(primary.is_primary, "Primary monitor should be marked as primary");
            assert!(primary.rect.width > 0);
            assert!(primary.work_area.width > 0);
        }
    }

    #[test]
    fn test_monitor_contains_point() {
        let monitor = MonitorInfo {
            id: 1,
            rect: Rect::new(0, 0, 1920, 1080),
            work_area: Rect::new(0, 0, 1920, 1040),
            is_primary: true,
            device_name: "DISPLAY1".to_string(),
        };

        // Point inside monitor
        assert!(monitor.contains_point(960, 540));
        // Point at origin
        assert!(monitor.contains_point(0, 0));
        // Point just inside right edge
        assert!(monitor.contains_point(1919, 540));
        // Point outside (right edge)
        assert!(!monitor.contains_point(1920, 540));
        // Point outside (negative)
        assert!(!monitor.contains_point(-1, 0));
    }

    #[test]
    fn test_monitor_contains_rect_center() {
        let monitor = MonitorInfo {
            id: 1,
            rect: Rect::new(0, 0, 1920, 1080),
            work_area: Rect::new(0, 0, 1920, 1040),
            is_primary: true,
            device_name: "DISPLAY1".to_string(),
        };

        // Window centered in monitor
        let window = Rect::new(100, 100, 800, 600);
        assert!(monitor.contains_rect_center(&window));

        // Window mostly outside but center inside
        let window2 = Rect::new(-300, 100, 800, 600);
        assert!(monitor.contains_rect_center(&window2)); // Center at 100, 400

        // Window with center outside
        let window3 = Rect::new(1800, 100, 800, 600);
        assert!(!monitor.contains_rect_center(&window3)); // Center at 2200, 400
    }

    #[test]
    fn test_find_monitor_for_rect() {
        let monitors = vec![
            MonitorInfo {
                id: 1,
                rect: Rect::new(0, 0, 1920, 1080),
                work_area: Rect::new(0, 0, 1920, 1040),
                is_primary: true,
                device_name: "DISPLAY1".to_string(),
            },
            MonitorInfo {
                id: 2,
                rect: Rect::new(1920, 0, 1920, 1080),
                work_area: Rect::new(1920, 0, 1920, 1080),
                is_primary: false,
                device_name: "DISPLAY2".to_string(),
            },
        ];

        // Window on first monitor
        let window1 = Rect::new(100, 100, 800, 600);
        let found = find_monitor_for_rect(&monitors, &window1);
        assert_eq!(found.unwrap().id, 1);

        // Window on second monitor
        let window2 = Rect::new(2000, 100, 800, 600);
        let found = find_monitor_for_rect(&monitors, &window2);
        assert_eq!(found.unwrap().id, 2);
    }

    #[test]
    fn test_monitors_by_position() {
        let monitors = vec![
            MonitorInfo {
                id: 2,
                rect: Rect::new(1920, 0, 1920, 1080),
                work_area: Rect::new(1920, 0, 1920, 1080),
                is_primary: false,
                device_name: "DISPLAY2".to_string(),
            },
            MonitorInfo {
                id: 1,
                rect: Rect::new(0, 0, 1920, 1080),
                work_area: Rect::new(0, 0, 1920, 1040),
                is_primary: true,
                device_name: "DISPLAY1".to_string(),
            },
        ];

        let sorted = monitors_by_position(&monitors);
        assert_eq!(sorted[0].id, 1); // Left monitor first
        assert_eq!(sorted[1].id, 2); // Right monitor second
    }

    #[test]
    fn test_monitor_to_left_right() {
        let monitors = vec![
            MonitorInfo {
                id: 1,
                rect: Rect::new(0, 0, 1920, 1080),
                work_area: Rect::new(0, 0, 1920, 1040),
                is_primary: true,
                device_name: "DISPLAY1".to_string(),
            },
            MonitorInfo {
                id: 2,
                rect: Rect::new(1920, 0, 1920, 1080),
                work_area: Rect::new(1920, 0, 1920, 1080),
                is_primary: false,
                device_name: "DISPLAY2".to_string(),
            },
        ];

        // From monitor 1, go right
        let right = monitor_to_right(&monitors, 1);
        assert_eq!(right.unwrap().id, 2);

        // From monitor 2, go left
        let left = monitor_to_left(&monitors, 2);
        assert_eq!(left.unwrap().id, 1);

        // From monitor 1, can't go left (edge)
        let no_left = monitor_to_left(&monitors, 1);
        assert!(no_left.is_none());

        // From monitor 2, can't go right (edge)
        let no_right = monitor_to_right(&monitors, 2);
        assert!(no_right.is_none());
    }

    #[test]
    fn test_parse_vk() {
        // Letters
        assert_eq!(parse_vk("H"), Some(vk::H));
        assert_eq!(parse_vk("h"), Some(vk::H));
        assert_eq!(parse_vk("L"), Some(vk::L));

        // Numbers
        assert_eq!(parse_vk("1"), Some(vk::N1));
        assert_eq!(parse_vk("0"), Some(vk::N0));

        // Function keys
        assert_eq!(parse_vk("F1"), Some(vk::F1));
        assert_eq!(parse_vk("F12"), Some(vk::F12));
        assert_eq!(parse_vk("f5"), Some(vk::F5));

        // Navigation
        assert_eq!(parse_vk("Left"), Some(vk::LEFT));
        assert_eq!(parse_vk("RIGHT"), Some(vk::RIGHT));

        // Special keys
        assert_eq!(parse_vk("Tab"), Some(vk::TAB));
        assert_eq!(parse_vk("Space"), Some(vk::SPACE));
        assert_eq!(parse_vk("Enter"), Some(vk::ENTER));
        assert_eq!(parse_vk("Escape"), Some(vk::ESCAPE));

        // Invalid
        assert_eq!(parse_vk("Invalid"), None);
        assert_eq!(parse_vk("F13"), None);
    }

    #[test]
    fn test_parse_hotkey_string() {
        // Win+H
        let (mods, vk) = parse_hotkey_string("Win+H").unwrap();
        assert!(mods.win);
        assert!(!mods.ctrl);
        assert!(!mods.alt);
        assert!(!mods.shift);
        assert_eq!(vk, super::vk::H);

        // Ctrl+Alt+Left
        let (mods, vk) = parse_hotkey_string("Ctrl+Alt+Left").unwrap();
        assert!(mods.ctrl);
        assert!(mods.alt);
        assert!(!mods.win);
        assert_eq!(vk, super::vk::LEFT);

        // Win+Shift+L
        let (mods, vk) = parse_hotkey_string("Win+Shift+L").unwrap();
        assert!(mods.win);
        assert!(mods.shift);
        assert_eq!(vk, super::vk::L);

        // Case insensitive
        let (mods, _) = parse_hotkey_string("win+shift+h").unwrap();
        assert!(mods.win);
        assert!(mods.shift);

        // Invalid modifier
        assert!(parse_hotkey_string("Foo+H").is_none());

        // Invalid key
        assert!(parse_hotkey_string("Win+InvalidKey").is_none());
    }

    #[test]
    fn test_modifiers_to_win32() {
        let mods = Modifiers::win();
        let flags = mods.to_win32();
        assert!(flags.contains(MOD_WIN));
        assert!(flags.contains(MOD_NOREPEAT));
        assert!(!flags.contains(MOD_CONTROL));

        let mods = Modifiers { ctrl: true, alt: true, shift: true, win: false };
        let flags = mods.to_win32();
        assert!(flags.contains(MOD_CONTROL));
        assert!(flags.contains(MOD_ALT));
        assert!(flags.contains(MOD_SHIFT));
        assert!(!flags.contains(MOD_WIN));
    }

    #[test]
    fn test_win32_error_display() {
        // Verify error types have proper Display implementations
        let set_pos_err = Win32Error::SetPositionFailed("test error".to_string());
        let display = format!("{}", set_pos_err);
        assert!(display.contains("test error"));
        assert!(display.contains("position"));

        let cloak_err = Win32Error::CloakFailed("cloak failed".to_string());
        let display = format!("{}", cloak_err);
        assert!(display.contains("cloak"));

        let window_not_found = Win32Error::WindowNotFound(12345);
        let display = format!("{}", window_not_found);
        assert!(display.contains("12345"));
    }

    #[test]
    fn test_apply_placements_empty() {
        // Verify empty placements succeed without error
        let config = PlatformConfig::default();
        let result = apply_placements(&[], &config);
        assert!(result.is_ok());
    }
}
