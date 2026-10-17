//! Foreground focus, closing windows and DWM border colours.

use crate::error::Win32Error;
use crate::window::window_id_to_hwnd;
use openniri_core_layout::WindowId;
use std::ffi::c_void;
use windows::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindow,
    PostMessageW, SetForegroundWindow, ShowWindow, SW_RESTORE,
};

/// Set the foreground window using Win32 SetForegroundWindow.
///
/// Uses AttachThreadInput trick to reliably set foreground even when
/// the calling process is not the foreground process.
pub fn set_foreground_window(hwnd: WindowId) -> Result<bool, Win32Error> {
    let window_id = hwnd;
    let hwnd = window_id_to_hwnd(window_id)?;

    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return Err(Win32Error::WindowNotFound(window_id));
        }

        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            if IsIconic(hwnd).as_bool() {
                return Err(Win32Error::SetPositionFailed(format!(
                    "Failed to restore minimized window {} before setting foreground",
                    window_id
                )));
            }
        }

        let target_thread = GetWindowThreadProcessId(hwnd, None);
        if target_thread == 0 {
            return Err(Win32Error::SetPositionFailed(format!(
                "GetWindowThreadProcessId returned 0 for window {}",
                window_id
            )));
        }
        let current_thread = GetCurrentThreadId();
        let mut diagnostics: Vec<String> = Vec::new();

        let foreground_hwnd = GetForegroundWindow();
        if foreground_hwnd == hwnd {
            return Ok(true);
        }
        // Windows only lets the foreground process change the foreground window.
        // Attaching to the input queue of the thread that owns the current
        // foreground window lifts that restriction. Attaching to the target's
        // thread does not, because the target is not in the foreground; that is
        // why focus-follows-mouse focus changes were reverted.
        let attach_thread = if foreground_hwnd.0.is_null() {
            target_thread
        } else {
            match GetWindowThreadProcessId(foreground_hwnd, None) {
                0 => target_thread,
                thread => thread,
            }
        };

        let mut attached = false;
        if attach_thread != current_thread {
            if windows::Win32::System::Threading::AttachThreadInput(
                current_thread,
                attach_thread,
                true,
            )
            .as_bool()
            {
                attached = true;
            } else {
                diagnostics.push(format!(
                    "AttachThreadInput attach failed (current_thread={}, attach_thread={})",
                    current_thread, attach_thread
                ));
            }
        }

        let mut foreground_set = SetForegroundWindow(hwnd).as_bool();

        // If SetForegroundWindow failed, try BringWindowToTop as fallback
        if !foreground_set {
            match BringWindowToTop(hwnd) {
                Ok(()) => {
                    foreground_set = SetForegroundWindow(hwnd).as_bool();
                    if !foreground_set {
                        diagnostics.push(
                            "SetForegroundWindow returned FALSE after BringWindowToTop fallback"
                                .to_string(),
                        );
                    }
                }
                Err(e) => diagnostics.push(format!("BringWindowToTop failed: {}", e)),
            }
        }

        // Detach thread input
        if attached
            && !windows::Win32::System::Threading::AttachThreadInput(
                current_thread,
                attach_thread,
                false,
            )
            .as_bool()
        {
            diagnostics.push(format!(
                "AttachThreadInput detach failed (current_thread={}, attach_thread={})",
                current_thread, attach_thread
            ));
        }

        if foreground_set {
            if !diagnostics.is_empty() {
                tracing::warn!(
                    "Foreground set for window {} with warnings: {}",
                    window_id,
                    diagnostics.join("; ")
                );
            }
            return Ok(true);
        }

        if diagnostics.is_empty() {
            // No explicit API error, but Windows denied foreground change.
            return Ok(false);
        }

        Err(Win32Error::SetPositionFailed(format!(
            "Failed to set foreground window {}: {}",
            window_id,
            diagnostics.join("; ")
        )))
    }
}

/// Close a window by posting WM_CLOSE.
///
/// This is a graceful close that allows the application to handle cleanup.
pub fn close_window(hwnd: WindowId) -> Result<(), Win32Error> {
    let hwnd = window_id_to_hwnd(hwnd)?;
    unsafe {
        const WM_CLOSE: u32 = 0x0010;
        PostMessageW(
            Some(hwnd),
            WM_CLOSE,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        )
        .map_err(|e| {
            Win32Error::SetPositionFailed(format!("PostMessageW(WM_CLOSE) failed: {}", e))
        })?;
    }
    Ok(())
}

/// Set the DWM border color for a window (Windows 11+).
///
/// Returns Ok(true) if the border was set, Ok(false) if the API is unsupported.
pub fn set_window_border_color(hwnd: WindowId, color: u32) -> Result<bool, Win32Error> {
    let window_id = hwnd;
    let hwnd = window_id_to_hwnd(window_id)?;
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return Err(Win32Error::WindowNotFound(window_id));
        }

        // DWMWA_BORDER_COLOR = 34
        const DWMWA_BORDER_COLOR: u32 = 34;
        let colorref = color;
        let result = DwmSetWindowAttribute(
            hwnd,
            windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE(DWMWA_BORDER_COLOR as i32),
            &colorref as *const u32 as *const c_void,
            std::mem::size_of::<u32>() as u32,
        );
        match result {
            Ok(()) => Ok(true),
            Err(e) => {
                if !IsWindow(Some(hwnd)).as_bool() {
                    return Err(Win32Error::WindowNotFound(window_id));
                }

                if is_border_color_unsupported_hresult(e.code()) {
                    return Ok(false);
                }

                Err(Win32Error::SetPositionFailed(format!(
                    "DwmSetWindowAttribute(DWMWA_BORDER_COLOR) failed for {:?}: {}",
                    hwnd, e
                )))
            }
        }
    }
}

/// Reset the DWM border color for a window to the default.
///
/// Returns Ok(true) if the border was reset, Ok(false) if the API is unsupported.
pub fn reset_window_border_color(hwnd: WindowId) -> Result<bool, Win32Error> {
    // DWMWA_COLOR_DEFAULT = 0xFFFFFFFF
    set_window_border_color(hwnd, 0xFFFFFFFF)
}

fn is_border_color_unsupported_hresult(code: windows::core::HRESULT) -> bool {
    const E_INVALIDARG_HRESULT: i32 = 0x8007_0057u32 as i32;
    const E_NOTIMPL_HRESULT: i32 = 0x8000_4001u32 as i32;
    code.0 == E_INVALIDARG_HRESULT || code.0 == E_NOTIMPL_HRESULT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::{is_valid_window, window_id_to_hwnd};

    #[test]
    fn test_is_border_color_unsupported_hresult_mapping() {
        assert!(is_border_color_unsupported_hresult(windows::core::HRESULT(
            0x8007_0057u32 as i32
        )));
        assert!(is_border_color_unsupported_hresult(windows::core::HRESULT(
            0x8000_4001u32 as i32
        )));
        assert!(!is_border_color_unsupported_hresult(
            windows::core::HRESULT(0x8000_4005u32 as i32)
        ));
    }

    #[test]
    fn test_invalid_window_ids_are_rejected() {
        let not_found = |result: Result<(), Win32Error>, id: WindowId| matches!(result, Err(Win32Error::WindowNotFound(got)) if got == id);
        assert!(!is_valid_window(0));
        assert!(not_found(window_id_to_hwnd(0).map(|_| ()), 0));
        assert!(not_found(set_foreground_window(0).map(|_| ()), 0));
        assert!(not_found(
            set_foreground_window(u64::MAX).map(|_| ()),
            u64::MAX
        ));
        assert!(not_found(close_window(0), 0));
        assert!(not_found(
            set_window_border_color(0, 0x4285F4).map(|_| ()),
            0
        ));
        assert!(not_found(
            set_window_border_color(u64::MAX, 0x4285F4).map(|_| ()),
            u64::MAX
        ));
        assert!(not_found(reset_window_border_color(0).map(|_| ()), 0));
    }
}
