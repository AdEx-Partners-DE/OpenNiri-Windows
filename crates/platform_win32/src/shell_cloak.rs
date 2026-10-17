//! Cloaking other processes' windows through the Windows shell.
//!
//! `DwmSetWindowAttribute(DWMWA_CLOAK)` only works for windows of the calling
//! process; for every other window DWM returns `E_ACCESSDENIED`. The shell
//! (explorer.exe) can cloak any top-level window through the undocumented
//! `IApplicationView::SetCloak`, which is how virtual desktops hide windows.
//! The interface layout follows GlazeWM (GPL-3.0,
//! `packages/wm-platform/src/platform_impl/windows/com.rs`).
//!
//! COM objects from the shell live in a single-threaded apartment, so one
//! dedicated thread owns them; other threads send it requests.

use std::sync::{mpsc, OnceLock};
use std::time::Duration;

use windows::core::{interface, IUnknown, IUnknown_Vtbl, Interface, GUID, HRESULT};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IServiceProvider, CLSCTX_ALL,
    COINIT_APARTMENTTHREADED,
};

use crate::{spawn_message_loop_thread, MessageLoopThread, Win32Error, WindowId};

/// CLSID of the immersive shell, which implements `IServiceProvider`.
const CLSID_IMMERSIVE_SHELL: GUID = GUID::from_u128(0xC2F03A33_21F5_47FA_B4BB_156362A2F239);

/// `SetCloak` arguments used by the shell for virtual-desktop hiding.
const CLOAK_TYPE: u32 = 1;
const CLOAK_FLAG_HIDDEN: i32 = 2;
const CLOAK_FLAG_VISIBLE: i32 = 0;

/// How long a caller waits for the shell thread before falling back.
const REQUEST_TIMEOUT: Duration = Duration::from_millis(500);

/// HRESULTs that mean the shell went away (for example Explorer restarted).
const RPC_E_DISCONNECTED: i32 = 0x8001_0108_u32 as i32;
const RPC_S_SERVER_UNAVAILABLE: i32 = 0x8007_06BA_u32 as i32;
const RPC_E_SERVER_DIED: i32 = 0x8001_0007_u32 as i32;
const RPC_E_SERVER_DIED_DNE: i32 = 0x8001_0012_u32 as i32;

/// Undocumented shell interface; placeholder methods keep the vtable layout.
#[interface("1841c6d7-4f9d-42c0-af41-8747538f10e5")]
unsafe trait IApplicationViewCollection: IUnknown {
    fn get_views(&self) -> HRESULT;
    fn get_views_by_z_order(&self) -> HRESULT;
    fn get_views_by_app_user_model_id(&self) -> HRESULT;
    fn get_view_for_hwnd(&self, window: isize, view: *mut Option<IApplicationView>) -> HRESULT;
}

/// Undocumented shell view interface. It derives from `IInspectable`; the first
/// three placeholders are the `IInspectable` methods.
#[interface("372E1D3B-38D3-42E4-A15B-8AB2B178F513")]
unsafe trait IApplicationView: IUnknown {
    fn get_iids(&self) -> HRESULT;
    fn get_runtime_class_name(&self) -> HRESULT;
    fn get_trust_level(&self) -> HRESULT;
    fn set_focus(&self) -> HRESULT;
    fn switch_to(&self) -> HRESULT;
    fn try_invoke_back(&self) -> HRESULT;
    fn get_thumbnail_window(&self) -> HRESULT;
    fn get_monitor(&self) -> HRESULT;
    fn get_visibility(&self) -> HRESULT;
    fn set_cloak(&self, cloak_type: u32, flag: i32) -> HRESULT;
}

struct Request {
    hwnd: isize,
    cloak: bool,
    reply: mpsc::Sender<Result<(), String>>,
}

/// COM state owned by the shell thread.
struct ShellState {
    collection: Option<IApplicationViewCollection>,
}

impl ShellState {
    fn connect() -> Option<IApplicationViewCollection> {
        unsafe {
            let provider: IServiceProvider =
                CoCreateInstance(&CLSID_IMMERSIVE_SHELL, None, CLSCTX_ALL).ok()?;
            provider.QueryService(&IApplicationViewCollection::IID).ok()
        }
    }

    fn set_cloak_once(&mut self, hwnd: isize, cloak: bool) -> windows::core::Result<()> {
        if self.collection.is_none() {
            self.collection = Self::connect();
        }
        let collection = self
            .collection
            .as_ref()
            .ok_or_else(|| windows::core::Error::from(E_FAIL))?;
        let mut view: Option<IApplicationView> = None;
        unsafe { collection.get_view_for_hwnd(hwnd, &mut view) }.ok()?;
        let view = view.ok_or_else(|| windows::core::Error::from(E_FAIL))?;
        let flag = if cloak {
            CLOAK_FLAG_HIDDEN
        } else {
            CLOAK_FLAG_VISIBLE
        };
        unsafe { view.set_cloak(CLOAK_TYPE, flag) }.ok()
    }

    fn set_cloak(&mut self, hwnd: isize, cloak: bool) -> Result<(), String> {
        match self.set_cloak_once(hwnd, cloak) {
            Ok(()) => Ok(()),
            Err(e) if is_disconnected(e.code().0) || self.collection.is_none() => {
                // Explorer restarted or was not ready: reconnect once.
                self.collection = Self::connect();
                self.set_cloak_once(hwnd, cloak)
                    .map_err(|e| format!("{} (HRESULT 0x{:08X})", e, e.code().0 as u32))
            }
            Err(e) => Err(format!("{} (HRESULT 0x{:08X})", e, e.code().0 as u32)),
        }
    }
}

impl Drop for ShellState {
    fn drop(&mut self) {
        self.collection = None;
        unsafe { CoUninitialize() };
    }
}

fn is_disconnected(code: i32) -> bool {
    matches!(
        code,
        RPC_E_DISCONNECTED | RPC_S_SERVER_UNAVAILABLE | RPC_E_SERVER_DIED | RPC_E_SERVER_DIED_DNE
    )
}

struct ShellCloakService {
    thread: MessageLoopThread,
    requests: mpsc::Sender<Request>,
}

fn service() -> Option<&'static ShellCloakService> {
    static SERVICE: OnceLock<Option<ShellCloakService>> = OnceLock::new();
    SERVICE
        .get_or_init(|| {
            let (requests, request_rx) = mpsc::channel::<Request>();
            let started = spawn_message_loop_thread(
                "openniri-shell-cloak",
                || {
                    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                        .ok()
                        .map_err(|e| {
                            Win32Error::CloakFailed(format!("CoInitializeEx failed: {}", e))
                        })?;
                    let state = ShellState {
                        collection: ShellState::connect(),
                    };
                    let connected = state.collection.is_some();
                    Ok((state, connected))
                },
                move |state: &mut ShellState| {
                    while let Ok(request) = request_rx.try_recv() {
                        let result = state.set_cloak(request.hwnd, request.cloak);
                        let _ = request.reply.send(result);
                    }
                },
            );
            match started {
                Ok((thread, connected)) => {
                    if connected {
                        tracing::info!("Shell cloaking available (IApplicationView::SetCloak)");
                    } else {
                        tracing::warn!(
                            "Shell cloaking interface not available yet; will retry on use"
                        );
                    }
                    Some(ShellCloakService { thread, requests })
                }
                Err(e) => {
                    tracing::warn!("Shell cloaking unavailable: {}", e);
                    None
                }
            }
        })
        .as_ref()
}

/// Whether the shell cloaking thread is running.
pub(crate) fn available() -> bool {
    service().is_some()
}

/// Cloak (`true`) or uncloak (`false`) a window through the shell.
pub(crate) fn set_cloak(window_id: WindowId, cloak: bool) -> Result<(), String> {
    let service = service().ok_or_else(|| "shell cloaking unavailable".to_string())?;
    let (reply, reply_rx) = mpsc::channel();
    service
        .requests
        .send(Request {
            hwnd: window_id as isize,
            cloak,
            reply,
        })
        .map_err(|_| "shell cloaking thread stopped".to_string())?;
    if !service.thread.wake() {
        return Err("failed to wake shell cloaking thread".to_string());
    }
    reply_rx
        .recv_timeout(REQUEST_TIMEOUT)
        .map_err(|_| "shell cloaking request timed out".to_string())?
}
