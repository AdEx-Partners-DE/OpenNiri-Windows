//! System tray icon management for OpenNiri daemon.
//!
//! Provides a system tray icon with a context menu for common operations:
//! - Refresh windows
//! - Reload configuration
//! - Exit daemon

use std::sync::mpsc;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};
use thiserror::Error;
use tracing::{debug, info};

/// Menu item IDs for tray context menu.
mod menu_ids {
    pub const REFRESH: &str = "refresh";
    pub const RELOAD: &str = "reload";
    pub const EXIT: &str = "exit";
    pub const TOGGLE_PAUSE: &str = "toggle_pause";
    pub const OPEN_CONFIG: &str = "open_config";
    pub const VIEW_LOGS: &str = "view_logs";
}

/// Events emitted by the tray icon.
#[derive(Debug, Clone)]
pub enum TrayEvent {
    /// User clicked "Refresh Windows" menu item.
    Refresh,
    /// User clicked "Reload Config" menu item.
    Reload,
    /// User clicked "Exit" menu item.
    Exit,
    /// User clicked "Pause/Resume Tiling" menu item.
    TogglePause,
    /// User clicked "Open Config" menu item.
    OpenConfig,
    /// User clicked "View Logs" menu item.
    ViewLogs,
}

/// Manages the system tray icon and context menu.
pub struct TrayManager {
    _tray: TrayIcon,
}

impl TrayManager {
    /// Create a new tray manager with icon and context menu.
    ///
    /// The provided sender will receive tray events when menu items are clicked.
    /// The sender should be a std::sync::mpsc::Sender that can be passed to the
    /// event thread.
    pub fn new(event_sender: mpsc::Sender<TrayEvent>) -> Result<Self, TrayError> {
        // Create context menu
        let menu = Menu::new();

        // Title item (disabled)
        let title = MenuItem::new("OpenNiri Windows", false, None);
        menu.append(&title).map_err(|e| TrayError::Menu(e.to_string()))?;

        // Separator
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| TrayError::Menu(e.to_string()))?;

        // Refresh Windows
        let refresh = MenuItem::with_id(menu_ids::REFRESH, "Refresh Windows", true, None);
        menu.append(&refresh).map_err(|e| TrayError::Menu(e.to_string()))?;

        // Reload Config
        let reload = MenuItem::with_id(menu_ids::RELOAD, "Reload Config", true, None);
        menu.append(&reload).map_err(|e| TrayError::Menu(e.to_string()))?;

        // Toggle Pause
        let toggle_pause = MenuItem::with_id(menu_ids::TOGGLE_PAUSE, "Pause Tiling", true, None);
        menu.append(&toggle_pause).map_err(|e| TrayError::Menu(e.to_string()))?;

        // Open Config
        let open_config = MenuItem::with_id(menu_ids::OPEN_CONFIG, "Open Config", true, None);
        menu.append(&open_config).map_err(|e| TrayError::Menu(e.to_string()))?;

        // View Logs
        let view_logs = MenuItem::with_id(menu_ids::VIEW_LOGS, "View Logs", true, None);
        menu.append(&view_logs).map_err(|e| TrayError::Menu(e.to_string()))?;

        // Separator
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| TrayError::Menu(e.to_string()))?;

        // Exit
        let exit = MenuItem::with_id(menu_ids::EXIT, "Exit", true, None);
        menu.append(&exit).map_err(|e| TrayError::Menu(e.to_string()))?;

        // Create the tray icon with a simple embedded icon
        let icon = create_default_icon()?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("OpenNiri Windows - Tiling Window Manager")
            .with_icon(icon)
            .build()
            .map_err(|e| TrayError::Build(e.to_string()))?;

        info!("System tray icon created");

        // Spawn thread to handle menu events and forward them
        std::thread::spawn(move || {
            let menu_channel = MenuEvent::receiver();
            while let Ok(event) = menu_channel.recv() {
                let tray_event = match event.id.0.as_str() {
                    menu_ids::REFRESH => TrayEvent::Refresh,
                    menu_ids::RELOAD => TrayEvent::Reload,
                    menu_ids::EXIT => TrayEvent::Exit,
                    menu_ids::TOGGLE_PAUSE => TrayEvent::TogglePause,
                    menu_ids::OPEN_CONFIG => TrayEvent::OpenConfig,
                    menu_ids::VIEW_LOGS => TrayEvent::ViewLogs,
                    id => {
                        debug!("Unknown menu item clicked: {}", id);
                        continue;
                    }
                };

                if event_sender.send(tray_event).is_err() {
                    // Receiver dropped, exit thread
                    break;
                }
            }
        });

        Ok(Self {
            _tray: tray,
        })
    }
}

/// Create a default icon for the tray.
///
/// Uses a simple blue square as a placeholder icon.
fn create_default_icon() -> Result<tray_icon::Icon, TrayError> {
    // Create a simple 32x32 RGBA icon (blue square with rounded appearance)
    const SIZE: usize = 32;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];

    // Colors: OpenNiri blue theme
    let primary_r = 66u8;
    let primary_g = 133u8;
    let primary_b = 244u8;
    let accent_r = 52u8;
    let accent_g = 168u8;
    let accent_b = 83u8;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let idx = (y * SIZE + x) * 4;

            // Calculate distance from center
            let cx = SIZE as f32 / 2.0;
            let cy = SIZE as f32 / 2.0;
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            // Create a rounded square with gradient
            let max_dist = SIZE as f32 / 2.0 - 2.0;

            if dist < max_dist {
                // Inside the icon - create a tiling pattern
                let tile_size = 8;
                let tx = x / tile_size;
                let ty = y / tile_size;

                // Checkerboard pattern to represent tiling
                if (tx + ty) % 2 == 0 {
                    rgba[idx] = primary_r;
                    rgba[idx + 1] = primary_g;
                    rgba[idx + 2] = primary_b;
                } else {
                    rgba[idx] = accent_r;
                    rgba[idx + 1] = accent_g;
                    rgba[idx + 2] = accent_b;
                }
                rgba[idx + 3] = 255; // Fully opaque
            } else if dist < max_dist + 2.0 {
                // Anti-aliased edge
                let alpha = ((max_dist + 2.0 - dist) / 2.0 * 255.0) as u8;
                rgba[idx] = primary_r;
                rgba[idx + 1] = primary_g;
                rgba[idx + 2] = primary_b;
                rgba[idx + 3] = alpha;
            }
            // else: transparent (default 0)
        }
    }

    tray_icon::Icon::from_rgba(rgba, SIZE as u32, SIZE as u32)
        .map_err(|e| TrayError::Icon(e.to_string()))
}

/// Errors that can occur during tray operations.
#[derive(Debug, Error)]
pub enum TrayError {
    #[error("Failed to create menu: {0}")]
    Menu(String),

    #[error("Failed to build tray icon: {0}")]
    Build(String),

    #[error("Failed to create icon: {0}")]
    Icon(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_default_icon() {
        let icon = create_default_icon();
        assert!(icon.is_ok(), "Should create default icon successfully");
    }
}
