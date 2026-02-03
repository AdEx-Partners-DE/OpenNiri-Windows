//! OpenNiri Daemon
//!
//! Main daemon process for the OpenNiri window manager.
//!
//! Responsibilities:
//! - Maintain workspace state
//! - Process window events from the platform layer
//! - Handle IPC commands from the CLI
//! - Trigger layout recalculations
//! - Apply window placements

use anyhow::Result;
use openniri_core_layout::{Rect, Workspace};
use openniri_platform_win32::PlatformConfig;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// Application state.
struct AppState {
    /// The scrollable workspace.
    workspace: Workspace,
    /// Platform configuration.
    platform_config: PlatformConfig,
    /// Current monitor viewport.
    viewport: Rect,
}

impl AppState {
    fn new() -> Self {
        Self {
            workspace: Workspace::with_gaps(10, 10),
            platform_config: PlatformConfig::default(),
            // TODO: Get actual primary monitor dimensions
            viewport: Rect::new(0, 0, 1920, 1080),
        }
    }

    /// Recalculate layout and apply placements.
    fn apply_layout(&self) -> Result<()> {
        let placements = self.workspace.compute_placements(self.viewport);
        openniri_platform_win32::apply_placements(&placements, &self.platform_config)?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("OpenNiri daemon starting...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    let state = AppState::new();

    info!("Workspace initialized with {} columns", state.workspace.column_count());
    info!("Viewport: {}x{}", state.viewport.width, state.viewport.height);

    // TODO: Implement main event loop
    // 1. Enumerate existing windows and add to workspace
    // 2. Install WinEvent hooks
    // 3. Start IPC server (named pipe)
    // 4. Process events in a loop:
    //    - Window events from hooks
    //    - Commands from IPC
    //    - Timer events for animations

    info!("OpenNiri daemon is not yet fully implemented.");
    info!("Core layout engine is ready. Platform integration pending.");

    // For now, just demonstrate the layout engine
    let mut demo_workspace = Workspace::with_gaps(10, 10);
    demo_workspace.insert_window(1, Some(600));
    demo_workspace.insert_window(2, Some(800));
    demo_workspace.insert_window(3, Some(600));

    info!("Demo workspace has {} columns", demo_workspace.column_count());
    info!("Total strip width: {} pixels", demo_workspace.total_width());

    let placements = demo_workspace.compute_placements(state.viewport);
    for p in &placements {
        info!(
            "Window {} at ({}, {}) size {}x{} - {:?}",
            p.window_id, p.rect.x, p.rect.y, p.rect.width, p.rect.height, p.visibility
        );
    }

    Ok(())
}
