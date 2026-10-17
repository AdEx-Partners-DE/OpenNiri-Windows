//! Global hotkeys, hotkey string parsing, virtual key codes and display-change
//! forwarding (from the hotkey window and from the display watcher).

use crate::error::{recover_poisoned_mutex, Win32Error};
use crate::events::WindowEvent;
use crate::message_loop::{spawn_message_loop_thread, MessageLoopThread, ThreadWindow};
use std::cell::RefCell;
use std::sync::mpsc;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, SPI_SETWORKAREA, WM_DISPLAYCHANGE, WM_HOTKEY, WM_SETTINGCHANGE,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};

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

thread_local! {
    /// Sender for hotkeys registered on this thread's hotkey window.
    static HOTKEY_SENDER: RefCell<Option<mpsc::Sender<HotkeyEvent>>> =
        const { RefCell::new(None) };
    /// Sender of the display watcher running on this thread.
    static DISPLAY_WATCHER_SENDER: RefCell<Option<mpsc::Sender<WindowEvent>>> =
        const { RefCell::new(None) };
}

/// Sender for display change events seen by the hotkey window.
///
/// Set by the daemon thread and read by the hotkey thread, so it is shared. It
/// outlives hotkey re-registration on config reload; clearing it with the
/// hotkeys used to stop monitor-change detection after every reload.
static DISPLAY_CHANGE_SENDER: std::sync::Mutex<Option<mpsc::Sender<WindowEvent>>> =
    std::sync::Mutex::new(None);

/// Drop the display-change sender so its forwarding thread can exit at shutdown.
pub fn clear_display_change_sender() {
    *DISPLAY_CHANGE_SENDER
        .lock()
        .unwrap_or_else(recover_poisoned_mutex) = None;
}

/// Register a sender for display change events.
///
/// This allows the hotkey window to forward WM_DISPLAYCHANGE messages
/// to the window event channel. [`spawn_display_watcher`] does the same without
/// depending on hotkeys.
pub fn set_display_change_sender(sender: mpsc::Sender<WindowEvent>) -> Result<(), Win32Error> {
    *DISPLAY_CHANGE_SENDER
        .lock()
        .unwrap_or_else(recover_poisoned_mutex) = Some(sender);
    Ok(())
}

/// Hidden top-level window: display and setting changes are broadcast to
/// top-level windows, but not to message-only windows.
fn create_hidden_window(
    class_name: &str,
    window_proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
) -> Result<ThreadWindow, Win32Error> {
    ThreadWindow::create(
        class_name,
        Some(window_proc),
        WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        WS_POPUP,
        0,
        0,
    )
}

/// Run `inner` as a window procedure, falling back to `DefWindowProcW` on panic.
fn guarded_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    inner: fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
) -> LRESULT {
    match std::panic::catch_unwind(|| inner(hwnd, msg, wparam, lparam)) {
        Ok(lresult) => lresult,
        Err(e) => {
            tracing::error!("Panic in window procedure: {:?}", e);
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}

/// Hotkey window and its registrations, owned by the hotkey thread.
///
/// `UnregisterHotKey` only works on the thread that registered the keys, so
/// dropping this on that thread is what frees them for the next registration.
struct HotkeyWindow {
    window: ThreadWindow,
    registered_ids: Vec<HotkeyId>,
}

impl Drop for HotkeyWindow {
    fn drop(&mut self) {
        for id in &self.registered_ids {
            let _ = unsafe { UnregisterHotKey(Some(self.window.hwnd()), *id) };
        }
        let _ = HOTKEY_SENDER.try_with(|sender| sender.borrow_mut().take());
        tracing::debug!("Unregistered {} hotkeys", self.registered_ids.len());
    }
}

/// Handle for the hotkey message window and thread.
///
/// Dropping this handle unregisters all hotkeys (on the thread that registered
/// them) and stops the message loop.
pub struct HotkeyHandle {
    _thread: MessageLoopThread,
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
    let requested = hotkeys.len();
    let (tx, rx) = mpsc::channel();
    let (thread, registered_ids) = spawn_message_loop_thread(
        "openniri-hotkeys",
        move || {
            let window = create_hidden_window("OpenNiriHotkeyClass", hotkey_window_proc)?;
            let mut state = HotkeyWindow {
                window,
                registered_ids: Vec::new(),
            };
            for hotkey in &hotkeys {
                let result = unsafe {
                    RegisterHotKey(
                        Some(state.window.hwnd()),
                        hotkey.id,
                        hotkey.modifiers.to_win32(),
                        hotkey.vk,
                    )
                };
                if result.is_ok() {
                    state.registered_ids.push(hotkey.id);
                    tracing::debug!("Registered hotkey {} (vk=0x{:X})", hotkey.id, hotkey.vk);
                } else {
                    tracing::warn!(
                        "Failed to register hotkey {} (vk=0x{:X}) - may be in use",
                        hotkey.id,
                        hotkey.vk
                    );
                }
            }
            // Keep every hotkey that registered. Rejecting the whole set when one
            // combo was taken (Win+L can never be registered) left users with no
            // shortcuts at all, including the emergency panic key.
            if !hotkeys.is_empty() && state.registered_ids.is_empty() {
                return Err(Win32Error::HotkeyRegistrationFailed(format!(
                    "None of the {} hotkeys could be registered",
                    hotkeys.len()
                )));
            }
            HOTKEY_SENDER.with(|sender| *sender.borrow_mut() = Some(tx));
            let registered_ids = state.registered_ids.clone();
            Ok((state, registered_ids))
        },
        |_| {},
    )?;

    if registered_ids.len() != requested {
        tracing::warn!(
            "Registered {}/{} hotkeys; the others are already taken by Windows or another application",
            registered_ids.len(),
            requested
        );
    } else if requested > 0 {
        tracing::info!("Registered {}/{} hotkeys", registered_ids.len(), requested);
    }

    Ok((
        HotkeyHandle {
            _thread: thread,
            registered_ids,
        },
        rx,
    ))
}

/// Window procedure for the hotkey message window.
unsafe extern "system" fn hotkey_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    guarded_window_proc(hwnd, msg, wparam, lparam, hotkey_window_proc_inner)
}

fn hotkey_window_proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            let hotkey_id = wparam.0 as HotkeyId;
            tracing::debug!("Hotkey {} pressed", hotkey_id);
            let _ = HOTKEY_SENDER.try_with(|sender| {
                if let Some(sender) = sender.borrow().as_ref() {
                    let _ = sender.send(HotkeyEvent { id: hotkey_id });
                }
            });
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            tracing::info!("Display configuration changed (WM_DISPLAYCHANGE)");
            let sender = DISPLAY_CHANGE_SENDER
                .lock()
                .unwrap_or_else(recover_poisoned_mutex);
            if let Some(sender) = sender.as_ref() {
                let _ = sender.send(WindowEvent::DisplayChange);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

// ============================================================================
// Display watcher
// ============================================================================

/// Display watcher window, owned by its thread.
struct DisplayWatcher {
    _window: ThreadWindow,
}

impl Drop for DisplayWatcher {
    fn drop(&mut self) {
        let _ = DISPLAY_WATCHER_SENDER.try_with(|sender| sender.borrow_mut().take());
    }
}

/// Watch for monitor and work-area changes on a dedicated message-loop thread.
///
/// A hidden top-level window forwards `WM_DISPLAYCHANGE` (monitors added,
/// removed, rearranged or resolution changed) and
/// `WM_SETTINGCHANGE(SPI_SETWORKAREA)` (taskbar moved or resized) to `sender`
/// as [`WindowEvent::DisplayChange`]. Unlike [`set_display_change_sender`], this
/// works without registered hotkeys.
pub fn spawn_display_watcher(
    sender: mpsc::Sender<WindowEvent>,
) -> Result<MessageLoopThread, Win32Error> {
    spawn_message_loop_thread(
        "openniri-display-watcher",
        move || {
            let window = create_hidden_window("OpenNiriDisplayWatcherClass", display_window_proc)?;
            DISPLAY_WATCHER_SENDER.with(|slot| *slot.borrow_mut() = Some(sender));
            Ok((DisplayWatcher { _window: window }, ()))
        },
        |_| {},
    )
    .map(|(thread, ())| thread)
}

/// Whether a broadcast message means the monitor layout or a work area changed.
fn is_display_change_message(msg: u32, wparam: WPARAM) -> bool {
    msg == WM_DISPLAYCHANGE || (msg == WM_SETTINGCHANGE && wparam.0 == SPI_SETWORKAREA.0 as usize)
}

unsafe extern "system" fn display_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    guarded_window_proc(hwnd, msg, wparam, lparam, display_window_proc_inner)
}

fn display_window_proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if !is_display_change_message(msg, wparam) {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    tracing::info!(
        "Display configuration or work area changed (message 0x{:04X})",
        msg
    );
    let _ = DISPLAY_WATCHER_SENDER.try_with(|sender| {
        if let Some(sender) = sender.borrow().as_ref() {
            let _ = sender.send(WindowEvent::DisplayChange);
        }
    });
    LRESULT(0)
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
    fn test_hotkey_window_proc_forwards_hotkey_and_display_change_events() {
        let _guard = GLOBAL_SENDER_TEST_LOCK
            .lock()
            .unwrap_or_else(recover_poisoned_mutex);
        let timeout = std::time::Duration::from_millis(100);

        let (hotkey_tx, hotkey_rx) = mpsc::channel::<HotkeyEvent>();
        HOTKEY_SENDER.with(|sender| *sender.borrow_mut() = Some(hotkey_tx));
        let (display_tx, display_rx) = mpsc::channel::<WindowEvent>();
        set_display_change_sender(display_tx).unwrap();

        let lresult =
            hotkey_window_proc_inner(HWND(std::ptr::null_mut()), WM_HOTKEY, WPARAM(77), LPARAM(0));
        assert_eq!(lresult.0, 0);
        let event = hotkey_rx
            .recv_timeout(timeout)
            .expect("hotkey event should be forwarded");
        assert_eq!(event.id, 77);

        let lresult = hotkey_window_proc_inner(
            HWND(std::ptr::null_mut()),
            WM_DISPLAYCHANGE,
            WPARAM(0),
            LPARAM(0),
        );
        assert_eq!(lresult.0, 0);
        let event = display_rx
            .recv_timeout(timeout)
            .expect("display change event should be forwarded");
        assert!(matches!(event, WindowEvent::DisplayChange));

        HOTKEY_SENDER.with(|sender| sender.borrow_mut().take());
        clear_display_change_sender();
    }

    #[test]
    fn test_hotkey_reregistration_keeps_display_change_sender() {
        let _guard = GLOBAL_SENDER_TEST_LOCK
            .lock()
            .unwrap_or_else(recover_poisoned_mutex);
        let (display_tx, _display_rx) = mpsc::channel::<WindowEvent>();
        set_display_change_sender(display_tx).unwrap();

        // An empty set registers nothing but runs the full window lifecycle.
        let (handle, _rx) = register_hotkeys(Vec::new()).expect("hotkey thread");
        assert_eq!(handle.registered_count(), 0);
        drop(handle);
        assert!(
            DISPLAY_CHANGE_SENDER
                .lock()
                .unwrap_or_else(recover_poisoned_mutex)
                .is_some(),
            "hotkey re-registration on reload must not disable display-change detection"
        );
        clear_display_change_sender();
    }

    #[test]
    fn test_display_watcher_forwards_display_and_work_area_changes() {
        let (tx, rx) = mpsc::channel::<WindowEvent>();
        DISPLAY_WATCHER_SENDER.with(|sender| *sender.borrow_mut() = Some(tx));
        let hwnd = HWND(std::ptr::null_mut());
        display_window_proc_inner(hwnd, WM_DISPLAYCHANGE, WPARAM(32), LPARAM(0));
        display_window_proc_inner(
            hwnd,
            WM_SETTINGCHANGE,
            WPARAM(SPI_SETWORKAREA.0 as usize),
            LPARAM(0),
        );
        assert_eq!(rx.try_iter().count(), 2);

        // Other setting changes (wallpaper, colours, ...) are not display changes.
        assert!(!is_display_change_message(WM_SETTINGCHANGE, WPARAM(0x0014)));
        DISPLAY_WATCHER_SENDER.with(|sender| sender.borrow_mut().take());
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
