//! Dedicated message-loop threads for thread-affine Win32 resources.

use crate::error::Win32Error;
use std::sync::mpsc;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW, TranslateMessage, MSG,
    PM_NOREMOVE, WM_APP,
};

/// Thread message that asks a message-loop thread to run its wake handler.
const WM_OPENNIRI_WAKE: u32 = WM_APP + 0x51;
/// Thread message that asks a message-loop thread to exit.
const WM_OPENNIRI_QUIT: u32 = WM_APP + 0x52;

/// A dedicated thread that owns thread-affine Win32 resources and pumps messages.
///
/// WinEvent hooks (`WINEVENT_OUTOFCONTEXT`), low-level mouse hooks and tray icons
/// deliver their callbacks through the message queue of the thread that created
/// them. Created on a thread that never calls `GetMessageW` (such as the tokio
/// main thread), every callback waits: the mouse lags, window events arrive late
/// or not at all, and Windows silently removes low-level hooks that time out.
///
/// The resources are created and dropped on this thread. Dropping the handle
/// stops the loop and waits briefly for the thread to exit.
pub struct MessageLoopThread {
    name: String,
    thread_id: u32,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MessageLoopThread {
    /// Ask the thread to run its wake handler. Returns false if the post failed.
    pub fn wake(&self) -> bool {
        unsafe { PostThreadMessageW(self.thread_id, WM_OPENNIRI_WAKE, WPARAM(0), LPARAM(0)) }
            .is_ok()
    }

    /// Stop the message loop and wait up to `timeout` for the thread to exit.
    ///
    /// Returns false if the thread did not exit in time; it is then left detached.
    pub fn shutdown(&mut self, timeout: std::time::Duration) -> bool {
        let Some(thread) = self.thread.take() else {
            return true;
        };
        let post_quit = || {
            let _ = unsafe {
                PostThreadMessageW(self.thread_id, WM_OPENNIRI_QUIT, WPARAM(0), LPARAM(0))
            };
        };
        post_quit();
        let deadline = std::time::Instant::now() + timeout;
        let mut next_repost = std::time::Instant::now() + std::time::Duration::from_millis(100);
        while !thread.is_finished() {
            let now = std::time::Instant::now();
            // Modal loops (an open tray menu) discard thread messages, so repeat the request.
            if now >= next_repost {
                post_quit();
                next_repost = now + std::time::Duration::from_millis(100);
            }
            if now >= deadline {
                tracing::warn!(
                    "{} thread did not exit within {:?}; detaching it",
                    self.name,
                    timeout
                );
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let _ = thread.join();
        true
    }
}

impl Drop for MessageLoopThread {
    fn drop(&mut self) {
        self.shutdown(std::time::Duration::from_secs(2));
    }
}

/// Spawn a thread that runs `init`, then pumps messages until shut down.
///
/// `init` runs on the new thread and returns thread-local state `S` (kept and
/// dropped on that thread, so it need not be `Send`) plus a payload `T` handed
/// back to the caller. `on_wake` runs on the thread after each
/// [`MessageLoopThread::wake`].
pub fn spawn_message_loop_thread<S, T, I, W>(
    name: &str,
    init: I,
    mut on_wake: W,
) -> Result<(MessageLoopThread, T), Win32Error>
where
    S: 'static,
    T: Send + 'static,
    I: FnOnce() -> Result<(S, T), Win32Error> + Send + 'static,
    W: FnMut(&mut S) + Send + 'static,
{
    let (init_tx, init_rx) = mpsc::channel::<Result<(u32, T), Win32Error>>();
    let thread = std::thread::Builder::new()
        .name(name.to_string())
        .spawn(move || {
            let thread_id = unsafe { GetCurrentThreadId() };
            let mut msg = MSG::default();
            // Create this thread's message queue before reporting the thread id,
            // so messages posted by other threads cannot be lost.
            let _ = unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE) };

            let mut state = match init() {
                Ok((state, payload)) => {
                    if init_tx.send(Ok((thread_id, payload))).is_err() {
                        return;
                    }
                    state
                }
                Err(e) => {
                    let _ = init_tx.send(Err(e));
                    return;
                }
            };

            loop {
                let result = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
                if result <= 0 {
                    break;
                }
                if msg.hwnd.is_invalid() {
                    match msg.message {
                        WM_OPENNIRI_QUIT => break,
                        WM_OPENNIRI_WAKE => {
                            on_wake(&mut state);
                            continue;
                        }
                        _ => {}
                    }
                }
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            drop(state);
        })
        .map_err(|e| {
            Win32Error::HookInstallFailed(format!("Failed to spawn {} thread: {}", name, e))
        })?;

    match init_rx.recv() {
        Ok(Ok((thread_id, payload))) => Ok((
            MessageLoopThread {
                name: name.to_string(),
                thread_id,
                thread: Some(thread),
            },
            payload,
        )),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => {
            let _ = thread.join();
            Err(Win32Error::HookInstallFailed(format!(
                "{} thread exited during initialization",
                name
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_loop_thread_runs_wake_handler_and_drops_state_on_its_thread() {
        struct State {
            wakes: u32,
            events: mpsc::Sender<String>,
        }
        impl Drop for State {
            fn drop(&mut self) {
                let name = std::thread::current().name().unwrap_or("").to_string();
                let _ = self.events.send(format!("dropped on {}", name));
            }
        }

        let (events_tx, events_rx) = mpsc::channel::<String>();
        let (mut thread, payload) = spawn_message_loop_thread(
            "openniri-test-loop",
            move || {
                Ok((
                    State {
                        wakes: 0,
                        events: events_tx,
                    },
                    7u32,
                ))
            },
            |state: &mut State| {
                state.wakes += 1;
                let _ = state.events.send(format!("wake {}", state.wakes));
            },
        )
        .expect("message loop thread should start");
        assert_eq!(payload, 7);

        let timeout = std::time::Duration::from_secs(2);
        assert!(thread.wake());
        assert_eq!(events_rx.recv_timeout(timeout).unwrap(), "wake 1");
        assert!(thread.wake());
        assert_eq!(events_rx.recv_timeout(timeout).unwrap(), "wake 2");

        assert!(thread.shutdown(timeout));
        assert_eq!(
            events_rx.recv_timeout(timeout).unwrap(),
            "dropped on openniri-test-loop"
        );
    }

    #[test]
    fn test_message_loop_thread_reports_init_failure() {
        let result = spawn_message_loop_thread::<(), (), _, _>(
            "openniri-test-loop-fail",
            || Err(Win32Error::HookInstallFailed("boom".to_string())),
            |_| {},
        );
        assert!(matches!(result, Err(Win32Error::HookInstallFailed(m)) if m == "boom"));
    }
}
