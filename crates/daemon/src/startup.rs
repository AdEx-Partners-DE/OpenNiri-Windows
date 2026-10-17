//! Command-line arguments, logging initialisation and the startup banner.

use crate::config::Config;
use anyhow::Result;
use clap::Parser;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

/// Command-line arguments for the daemon binary.
#[derive(Parser, Debug, Clone)]
#[command(name = "openniri", about = "OpenNiri tiling window manager daemon")]
pub struct Args {
    /// Disable global hotkey registration
    #[arg(long)]
    pub no_hotkeys: bool,
    /// Use MoveOffScreen instead of DWM cloaking
    #[arg(long)]
    pub no_cloak: bool,
    /// Safe mode: combines --no-hotkeys and --no-cloak
    #[arg(long)]
    pub safe_mode: bool,
}

impl Args {
    /// Returns true if hotkeys should be skipped (either --no-hotkeys or --safe-mode).
    pub fn skip_hotkeys(&self) -> bool {
        self.no_hotkeys || self.safe_mode
    }

    /// Returns true if cloaking should be disabled (either --no-cloak or --safe-mode).
    pub fn skip_cloak(&self) -> bool {
        self.no_cloak || self.safe_mode
    }
}

/// Initialize logging with the configured log level.
pub(crate) fn init_logging(config: &Config) -> Result<()> {
    let log_level = match config.behavior.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO, // default fallback for invalid values
    };
    let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

/// Startup banner info for display after initialization.
pub struct StartupInfo {
    pub version: String,
    pub monitor_names: Vec<String>,
    pub window_count: usize,
    pub hotkeys_registered: usize,
    pub hotkeys_requested: usize,
    pub config_path: Option<String>,
    pub config_warnings: Vec<String>,
    pub log_path: String,
    pub safe_mode: bool,
    pub no_hotkeys: bool,
    pub no_cloak: bool,
}

/// Print a startup banner to stderr so users see immediate feedback.
/// Format the startup banner into a string (testable without capturing stderr).
pub fn format_startup_banner(info: &StartupInfo) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    writeln!(out).unwrap();
    writeln!(out, "OpenNiri Windows v{}", info.version).unwrap();
    if info.monitor_names.is_empty() {
        writeln!(out, "  Monitors: 0 (fallback mode)").unwrap();
    } else {
        writeln!(
            out,
            "  Monitors: {} ({})",
            info.monitor_names.len(),
            info.monitor_names.join(", ")
        )
        .unwrap();
    }
    writeln!(out, "  Windows:  {} managed", info.window_count).unwrap();
    if info.hotkeys_registered < info.hotkeys_requested {
        writeln!(
            out,
            "  Hotkeys:  {}/{} registered ({} failed)",
            info.hotkeys_registered,
            info.hotkeys_requested,
            info.hotkeys_requested - info.hotkeys_registered
        )
        .unwrap();
    } else {
        writeln!(out, "  Hotkeys:  {} registered", info.hotkeys_registered).unwrap();
    }
    if let Some(ref path) = info.config_path {
        writeln!(out, "  Config:   {}", path).unwrap();
    } else {
        writeln!(out, "  Config:   (default — no config file found)").unwrap();
    }
    for w in &info.config_warnings {
        writeln!(out, "  Warning:  {}", w).unwrap();
    }
    writeln!(out, "  Logs:     {}", info.log_path).unwrap();
    if info.safe_mode {
        writeln!(
            out,
            "  Mode:     SAFE MODE (hotkeys disabled, cloaking disabled)"
        )
        .unwrap();
    } else if info.no_hotkeys {
        writeln!(out, "  Mode:     hotkeys disabled").unwrap();
    } else if info.no_cloak {
        writeln!(out, "  Mode:     cloaking disabled").unwrap();
    } else {
        writeln!(out, "  Status:   Active").unwrap();
    }
    writeln!(out).unwrap();
    out
}

/// Print the startup banner to stderr.
pub fn print_startup_banner(info: &StartupInfo) {
    eprint!("{}", format_startup_banner(info));
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Startup, shutdown and singleton
    // ========================================================================

    #[test]
    fn test_args_flags_and_safe_mode() {
        for (argv, skip_hotkeys, skip_cloak) in [
            (&["openniri"][..], false, false),
            (&["openniri", "--no-hotkeys"][..], true, false),
            (&["openniri", "--no-cloak"][..], false, true),
            (&["openniri", "--safe-mode"][..], true, true),
        ] {
            let args = Args::try_parse_from(argv).unwrap();
            assert_eq!(args.skip_hotkeys(), skip_hotkeys, "{:?}", argv);
            assert_eq!(args.skip_cloak(), skip_cloak, "{:?}", argv);
        }
    }

    #[test]
    fn test_startup_banner_reports_failed_hotkeys_and_config_warnings() {
        let mut info = StartupInfo {
            version: "0.1.0".to_string(),
            monitor_names: vec!["DISPLAY1".to_string(), "DISPLAY2".to_string()],
            window_count: 14,
            hotkeys_registered: 7,
            hotkeys_requested: 10,
            config_path: None,
            config_warnings: vec!["layout.gap: Negative gap (-5) clamped to 0".to_string()],
            log_path: "openniri-daemon.log".to_string(),
            safe_mode: false,
            no_hotkeys: false,
            no_cloak: false,
        };
        let banner = format_startup_banner(&info);
        assert!(banner.contains("7/10 registered (3 failed)"));
        assert!(banner.contains("Warning:  layout.gap"));

        info.hotkeys_registered = 10;
        info.config_warnings.clear();
        let banner = format_startup_banner(&info);
        assert!(!banner.contains("failed"));
        assert!(!banner.contains("Warning:"));
    }
}
