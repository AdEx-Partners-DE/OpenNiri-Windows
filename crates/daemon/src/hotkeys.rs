//! Global hotkey registration from config.

use crate::config::{self, Config};
use crate::DaemonEvent;
use openniri_ipc::IpcCommand;
use openniri_platform_win32::{parse_hotkey_string, register_hotkeys, Hotkey, HotkeyId};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Hotkey registration result containing handle and mapping.
pub(crate) struct HotkeyState {
    /// Handle to unregister hotkeys on drop.
    pub(crate) handle: Option<openniri_platform_win32::HotkeyHandle>,
    /// Mapping of hotkey IDs to commands.
    pub(crate) mapping: HashMap<HotkeyId, IpcCommand>,
    /// Number of hotkeys that were requested for registration.
    pub(crate) requested_count: usize,
    /// Number of hotkeys the OS actually registered (may be less than
    /// `requested_count` if some conflicted with other applications).
    pub(crate) registered_count: usize,
}

/// Register hotkeys from config and return state.
///
/// This function is called both at startup and on config reload.
pub(crate) fn setup_hotkeys(config: &Config, event_tx: mpsc::Sender<DaemonEvent>) -> HotkeyState {
    let config_hotkeys = &config.hotkeys.bindings;

    // Build hotkey definitions and command mapping
    let mut hotkeys = Vec::new();
    let mut mapping = HashMap::new();
    let mut key_names: HashMap<HotkeyId, &str> = HashMap::new();
    let mut next_id: HotkeyId = 1;

    for (key_str, cmd_str) in config_hotkeys {
        if let Some((modifiers, vk)) = parse_hotkey_string(key_str) {
            if let Some(cmd) = config::parse_command(cmd_str) {
                hotkeys.push(Hotkey::new(next_id, modifiers, vk));
                key_names.insert(next_id, key_str.as_str());
                mapping.insert(next_id, cmd);
                debug!(
                    "Configured hotkey {}: {} -> {:?}",
                    next_id, key_str, cmd_str
                );
                next_id += 1;
            } else {
                warn!(
                    "Unknown command in hotkey config: {} -> {}",
                    key_str, cmd_str
                );
            }
        } else {
            warn!("Invalid hotkey string in config: {}", key_str);
        }
    }

    let requested_count = hotkeys.len();

    if hotkeys.is_empty() {
        info!("No hotkeys configured");
        return HotkeyState {
            handle: None,
            mapping,
            requested_count: 0,
            registered_count: 0,
        };
    }

    match register_hotkeys(hotkeys) {
        Ok((handle, hotkey_receiver)) => {
            info!("Registered {} global hotkeys", handle.registered_count());

            // Spawn task to forward hotkey events
            match std::thread::Builder::new()
                .name("hotkey-fwd".to_string())
                .spawn(move || {
                    while let Ok(event) = hotkey_receiver.recv() {
                        if event_tx.blocking_send(DaemonEvent::Hotkey(event)).is_err() {
                            break;
                        }
                    }
                }) {
                Ok(_) => {} // Thread is detached, we don't track it
                Err(e) => {
                    warn!("Failed to spawn hotkey-fwd thread: {}", e);
                }
            }

            report_unregistered_hotkeys(&handle, &mapping, &key_names);

            let registered_count = handle.registered_count();
            HotkeyState {
                handle: Some(handle),
                mapping,
                requested_count,
                registered_count,
            }
        }
        Err(e) => {
            warn!(
                "Failed to register hotkeys: {}. Global shortcuts disabled.",
                e
            );
            HotkeyState {
                handle: None,
                mapping,
                requested_count,
                registered_count: 0,
            }
        }
    }
}

/// Log each configured hotkey that could not be registered.
fn report_unregistered_hotkeys(
    handle: &openniri_platform_win32::HotkeyHandle,
    mapping: &HashMap<HotkeyId, IpcCommand>,
    key_names: &HashMap<HotkeyId, &str>,
) {
    let mut ids: Vec<HotkeyId> = mapping.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        if handle.is_registered(id) {
            continue;
        }
        let key = key_names.get(&id).copied().unwrap_or("?");
        if matches!(mapping.get(&id), Some(IpcCommand::PanicRevert)) {
            warn!(
                "Emergency hotkey {} (panic_revert) could not be registered; use `openniri-cli panic-revert` or the tray menu instead",
                key
            );
        } else {
            warn!(
                "Hotkey {} -> {:?} could not be registered (already used by Windows or another application)",
                key,
                mapping.get(&id)
            );
        }
    }
}
