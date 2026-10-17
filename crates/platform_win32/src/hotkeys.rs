//! Global hotkeys, hotkey string parsing, virtual key codes and display-change
//! forwarding from the hotkey window.

use crate::error::{recover_poisoned_mutex, Win32Error};
use crate::events::WindowEvent;
use std::ffi::c_void;
use std::sync::mpsc;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostMessageW,
    PostThreadMessageW, RegisterClassW, UnregisterClassW, MSG, WM_HOTKEY, WM_USER, WNDCLASSW,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};

// Window message for display configuration changes
const WM_DISPLAYCHANGE: u32 = 0x007E;

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
        Self {
            win: true,
            ..Default::default()
        }
    }

    /// Create modifiers with Win + Shift.
    pub fn win_shift() -> Self {
        Self {
            win: true,
            shift: true,
            ..Default::default()
        }
    }

    /// Create modifiers with Alt.
    pub fn alt() -> Self {
        Self {
            alt: true,
            ..Default::default()
        }
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

/// Clear the hotkey sender. The display-change sender is left alone: it outlives
/// hotkey re-registration on config reload, and clearing it here used to stop
/// monitor-change detection after every reload.
fn clear_hotkey_globals() {
    let mut sender = HOTKEY_SENDER.lock().unwrap_or_else(recover_poisoned_mutex);
    *sender = None;
}

/// Drop the display-change sender so its forwarding thread can exit at shutdown.
pub fn clear_display_change_sender() {
    let mut sender = DISPLAY_CHANGE_SENDER
        .lock()
        .unwrap_or_else(recover_poisoned_mutex);
    *sender = None;
}

fn request_hotkey_thread_shutdown(hwnd: HWND, thread_id: u32) -> bool {
    let mut shutdown_signal_sent = unsafe {
        PostMessageW(
            Some(hwnd),
            WM_QUIT_HOTKEY_THREAD,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        )
        .is_ok()
    };

    if !shutdown_signal_sent {
        tracing::warn!(
            "PostMessageW quit signal failed for hotkey window {:?}; attempting thread message fallback",
            hwnd
        );
        shutdown_signal_sent = unsafe {
            PostThreadMessageW(
                thread_id,
                WM_QUIT_HOTKEY_THREAD,
                windows::Win32::Foundation::WPARAM(0),
                windows::Win32::Foundation::LPARAM(0),
            )
            .is_ok()
        };
        if !shutdown_signal_sent {
            tracing::warn!(
                "PostThreadMessageW quit signal failed for hotkey thread {}; proceeding without blocking join",
                thread_id
            );
        }
    }

    shutdown_signal_sent
}

/// Handle for the hotkey message window and thread.
///
/// Dropping this handle will unregister all hotkeys and stop the message loop.
pub struct HotkeyHandle {
    hwnd: HWND,
    thread_id: u32,
    thread: Option<std::thread::JoinHandle<()>>,
    registered_ids: Vec<HotkeyId>,
}

impl HotkeyHandle {
    /// Whether the hotkey with this id was registered.
    pub fn is_registered(&self, id: HotkeyId) -> bool {
        self.registered_ids.contains(&id)
    }

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
        let shutdown_signal_sent = request_hotkey_thread_shutdown(self.hwnd, self.thread_id);

        // Join if finished; otherwise detach to avoid hanging shutdown.
        if let Some(thread) = self.thread.take() {
            if shutdown_signal_sent {
                const HOTKEY_THREAD_WAIT_POLLS: usize = 30;
                const HOTKEY_THREAD_WAIT_MS: u64 = 10;
                for _ in 0..HOTKEY_THREAD_WAIT_POLLS {
                    if thread.is_finished() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(HOTKEY_THREAD_WAIT_MS));
                }
            }

            if thread.is_finished() {
                let _ = thread.join();
            } else {
                tracing::warn!(
                    "Hotkey thread did not exit promptly after shutdown signal; detaching to avoid hang"
                );
            }
        }

        // Clear the global senders to allow re-registration (recover from mutex poisoning)
        clear_hotkey_globals();
    }
}

/// Register a sender for display change events.
///
/// This allows the hotkey window to forward WM_DISPLAYCHANGE messages
/// to the window event channel. Call this before `register_hotkeys`.
pub fn set_display_change_sender(sender: mpsc::Sender<WindowEvent>) -> Result<(), Win32Error> {
    let mut guard = DISPLAY_CHANGE_SENDER.lock().map_err(|_| {
        Win32Error::HookInstallFailed("Display change sender mutex poisoned".to_string())
    })?;
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
        let mut sender = HOTKEY_SENDER.lock().map_err(|_| {
            Win32Error::HotkeyRegistrationFailed("Hotkey sender mutex poisoned".to_string())
        })?;
        if sender.is_some() {
            return Err(Win32Error::HotkeyRegistrationFailed(
                "Hotkey sender already initialized - drop existing HotkeyHandle first".to_string(),
            ));
        }
        *sender = Some(tx);
    }

    // Create the message window and register hotkeys on a separate thread
    // We send isize (raw pointer value) instead of HWND because HWND is !Send
    let (init_tx, init_rx) =
        std::sync::mpsc::channel::<Result<(isize, u32, Vec<HotkeyId>), Win32Error>>();
    let hotkeys_clone = hotkeys.clone();

    let thread = std::thread::spawn(move || {
        unsafe {
            let thread_id = GetCurrentThreadId();

            // Register window class
            let class_name: Vec<u16> = "OpenNiriHotkeyClass\0".encode_utf16().collect();
            let wc = WNDCLASSW {
                lpfnWndProc: Some(hotkey_window_proc),
                lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            RegisterClassW(&wc);

            // Create a hidden top-level window.
            // WM_DISPLAYCHANGE is broadcast to top-level windows, but not to message-only windows.
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                windows::core::PCWSTR(class_name.as_ptr()),
                None,
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
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
            let _ = init_tx.send(Ok((hwnd_raw, thread_id, registered_ids)));

            // Message loop
            let mut msg = MSG::default();
            loop {
                let get_message_result = GetMessageW(&mut msg, None, 0, 0).0;
                if get_message_result <= 0 {
                    break;
                }
                if msg.message == WM_QUIT_HOTKEY_THREAD {
                    break;
                }
                let _ = DispatchMessageW(&msg);
            }

            let _ = DestroyWindow(hwnd);
            let _ = UnregisterClassW(windows::core::PCWSTR(class_name.as_ptr()), None);
        }
    });

    // Wait for initialization
    let init_result = init_rx.recv().map_err(|_| {
        clear_hotkey_globals();
        Win32Error::HotkeyRegistrationFailed("Thread initialization failed".to_string())
    })?;
    let (hwnd_raw, thread_id, registered_ids) = match init_result {
        Ok(v) => v,
        Err(e) => {
            clear_hotkey_globals();
            if thread.is_finished() {
                let _ = thread.join();
            }
            return Err(e);
        }
    };

    // Reconstruct HWND from raw pointer
    let hwnd = HWND(hwnd_raw as *mut c_void);

    // Keep every hotkey that registered. Rejecting the whole set when one combo
    // was taken (Win+L can never be registered) left users with no shortcuts at
    // all, including the emergency panic key.
    if !hotkeys.is_empty() && registered_ids.is_empty() {
        let _ = request_hotkey_thread_shutdown(hwnd, thread_id);
        if thread.is_finished() {
            let _ = thread.join();
        }
        clear_hotkey_globals();
        return Err(Win32Error::HotkeyRegistrationFailed(format!(
            "None of the {} hotkeys could be registered",
            hotkeys.len()
        )));
    }
    if registered_ids.len() != hotkeys.len() {
        tracing::warn!(
            "Registered {}/{} hotkeys; the others are already taken by Windows or another application",
            registered_ids.len(),
            hotkeys.len()
        );
    } else if !hotkeys.is_empty() {
        tracing::info!(
            "Registered {}/{} hotkeys",
            registered_ids.len(),
            hotkeys.len()
        );
    }

    Ok((
        HotkeyHandle {
            hwnd,
            thread_id,
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

            // Send event through channel (recover from mutex poisoning)
            let sender_guard = HOTKEY_SENDER.lock().unwrap_or_else(recover_poisoned_mutex);
            if let Some(sender) = sender_guard.as_ref() {
                let _ = sender.send(HotkeyEvent { id: hotkey_id });
            }

            windows::Win32::Foundation::LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            tracing::info!("Display configuration changed (WM_DISPLAYCHANGE)");

            // Send display change event through window event channel (recover from mutex poisoning)
            let sender_guard = DISPLAY_CHANGE_SENDER
                .lock()
                .unwrap_or_else(recover_poisoned_mutex);
            if let Some(sender) = sender_guard.as_ref() {
                let _ = sender.send(WindowEvent::DisplayChange);
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
    pub const MINUS: u32 = 0xBD; // '-'
    pub const EQUALS: u32 = 0xBB; // '='
    pub const BRACKET_LEFT: u32 = 0xDB; // '['
    pub const BRACKET_RIGHT: u32 = 0xDD; // ']'
    pub const COMMA: u32 = 0xBC; // ','
    pub const PERIOD: u32 = 0xBE; // '.'
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::GLOBAL_SENDER_TEST_LOCK;

    #[test]
    fn test_clear_hotkey_globals_keeps_display_change_sender() {
        let _guard = GLOBAL_SENDER_TEST_LOCK
            .lock()
            .unwrap_or_else(recover_poisoned_mutex);

        let (hotkey_tx, _hotkey_rx) = mpsc::channel::<HotkeyEvent>();
        let (display_tx, _display_rx) = mpsc::channel::<WindowEvent>();

        {
            let mut sender_guard = HOTKEY_SENDER.lock().unwrap_or_else(recover_poisoned_mutex);
            *sender_guard = Some(hotkey_tx);
        }
        {
            let mut sender_guard = DISPLAY_CHANGE_SENDER
                .lock()
                .unwrap_or_else(recover_poisoned_mutex);
            *sender_guard = Some(display_tx);
        }

        clear_hotkey_globals();

        let hotkey_sender = HOTKEY_SENDER.lock().unwrap_or_else(recover_poisoned_mutex);
        assert!(hotkey_sender.is_none());
        drop(hotkey_sender);
        {
            let display_sender = DISPLAY_CHANGE_SENDER
                .lock()
                .unwrap_or_else(recover_poisoned_mutex);
            assert!(
                display_sender.is_some(),
                "hotkey re-registration on reload must not disable display-change detection"
            );
        }

        clear_display_change_sender();
        let display_sender = DISPLAY_CHANGE_SENDER
            .lock()
            .unwrap_or_else(recover_poisoned_mutex);
        assert!(display_sender.is_none());
    }

    #[test]
    fn test_hotkey_window_proc_forwards_hotkey_and_display_change_events() {
        let _guard = GLOBAL_SENDER_TEST_LOCK
            .lock()
            .unwrap_or_else(recover_poisoned_mutex);
        let timeout = std::time::Duration::from_millis(100);

        let (hotkey_tx, hotkey_rx) = mpsc::channel::<HotkeyEvent>();
        *HOTKEY_SENDER.lock().unwrap_or_else(recover_poisoned_mutex) = Some(hotkey_tx);
        let (display_tx, display_rx) = mpsc::channel::<WindowEvent>();
        set_display_change_sender(display_tx).unwrap();

        let lresult = hotkey_window_proc_inner(
            HWND(std::ptr::null_mut()),
            WM_HOTKEY,
            windows::Win32::Foundation::WPARAM(77),
            windows::Win32::Foundation::LPARAM(0),
        );
        assert_eq!(lresult.0, 0);
        let event = hotkey_rx
            .recv_timeout(timeout)
            .expect("hotkey event should be forwarded");
        assert_eq!(event.id, 77);

        let lresult = hotkey_window_proc_inner(
            HWND(std::ptr::null_mut()),
            WM_DISPLAYCHANGE,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        );
        assert_eq!(lresult.0, 0);
        let event = display_rx
            .recv_timeout(timeout)
            .expect("display change event should be forwarded");
        assert!(matches!(event, WindowEvent::DisplayChange));

        *HOTKEY_SENDER.lock().unwrap_or_else(recover_poisoned_mutex) = None;
        clear_display_change_sender();
    }

    #[test]
    fn test_hotkey_handle_drop_does_not_block_when_quit_post_fails() {
        let _guard = GLOBAL_SENDER_TEST_LOCK
            .lock()
            .unwrap_or_else(recover_poisoned_mutex);

        let sleeping_thread = std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(1));
        });

        let start = std::time::Instant::now();
        drop(HotkeyHandle {
            hwnd: HWND(std::ptr::null_mut()),
            thread_id: 0,
            thread: Some(sleeping_thread),
            registered_ids: Vec::new(),
        });
        assert!(start.elapsed() < std::time::Duration::from_millis(500));
    }

    #[test]
    fn test_parse_hotkey_string() {
        let none = Modifiers::default();
        let cases = [
            ("Alt+H", Modifiers { alt: true, ..none }, vk::H),
            (
                "alt+shift+l",
                Modifiers {
                    alt: true,
                    shift: true,
                    ..none
                },
                vk::L,
            ),
            (
                "Win+Ctrl+Escape",
                Modifiers {
                    win: true,
                    ctrl: true,
                    ..none
                },
                vk::ESCAPE,
            ),
            (
                "Ctrl+Alt+Left",
                Modifiers {
                    ctrl: true,
                    alt: true,
                    ..none
                },
                vk::LEFT,
            ),
            ("Alt+1", Modifiers { alt: true, ..none }, vk::N1),
            ("Win+F12", Modifiers { win: true, ..none }, vk::F12),
            ("Alt+Space", Modifiers { alt: true, ..none }, vk::SPACE),
            ("Alt+Tab", Modifiers { alt: true, ..none }, vk::TAB),
            ("Alt+Enter", Modifiers { alt: true, ..none }, vk::ENTER),
        ];
        for (input, mods, key) in cases {
            let (parsed_mods, parsed_key) =
                parse_hotkey_string(input).unwrap_or_else(|| panic!("{input} should parse"));
            assert_eq!(parsed_mods, mods, "modifiers of {input}");
            assert_eq!(parsed_key, key, "key of {input}");
        }
        for invalid in ["Foo+H", "Win+InvalidKey", "Alt+F13", "Alt+"] {
            assert!(parse_hotkey_string(invalid).is_none(), "{invalid}");
        }

        let flags = Modifiers {
            ctrl: true,
            alt: true,
            ..none
        }
        .to_win32();
        assert!(flags.contains(MOD_CONTROL) && flags.contains(MOD_ALT));
        assert!(!flags.contains(MOD_WIN) && !flags.contains(MOD_SHIFT));
        assert!(
            flags.contains(MOD_NOREPEAT),
            "held keys must not auto-repeat commands"
        );
    }
}
