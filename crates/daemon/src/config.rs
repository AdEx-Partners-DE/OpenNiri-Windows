//! Configuration management for OpenNiri daemon.
//!
//! Configuration is loaded from TOML files in the following locations (in order):
//! 1. `%APPDATA%/openniri/config.toml` (Windows standard)
//! 2. `~/.config/openniri/config.toml` (Unix-style, for WSL compatibility)
//! 3. `./config.toml` (current directory, for development)

use anyhow::{Context, Result};
use directories::ProjectDirs;
use openniri_core_layout::CenteringMode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Main configuration structure for OpenNiri.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Layout configuration.
    pub layout: LayoutConfig,
    /// Appearance configuration.
    pub appearance: AppearanceConfig,
    /// Behavior configuration.
    pub behavior: BehaviorConfig,
    /// Hotkey bindings.
    pub hotkeys: HotkeyConfig,
    /// Window rules for per-window behavior.
    #[serde(default)]
    pub window_rules: Vec<WindowRule>,
    /// Gesture bindings for touchpad support.
    #[serde(default)]
    pub gestures: GestureConfig,
    /// Snap hint configuration.
    #[serde(default)]
    pub snap_hints: SnapHintConfig,
}

/// Layout-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutConfig {
    /// Gap between columns in pixels.
    #[serde(default = "default_gap")]
    pub gap: i32,

    /// Gap at the edges of the viewport in pixels.
    #[serde(default = "default_outer_gap")]
    pub outer_gap: i32,

    /// Default width for new columns in pixels.
    #[serde(default = "default_column_width")]
    pub default_column_width: i32,

    /// Minimum column width in pixels.
    #[serde(default = "default_min_column_width")]
    pub min_column_width: i32,

    /// Maximum column width in pixels.
    #[serde(default = "default_max_column_width")]
    pub max_column_width: i32,

    /// Centering mode for focus navigation.
    #[serde(default)]
    pub centering_mode: CenteringModeConfig,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            gap: default_gap(),
            outer_gap: default_outer_gap(),
            default_column_width: default_column_width(),
            min_column_width: default_min_column_width(),
            max_column_width: default_max_column_width(),
            centering_mode: CenteringModeConfig::default(),
        }
    }
}

/// Centering mode configuration (wrapper for serialization).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CenteringModeConfig {
    /// Center the focused column in the viewport.
    #[default]
    Center,
    /// Only scroll if the focused column would be outside the viewport.
    JustInView,
}

impl From<CenteringModeConfig> for CenteringMode {
    fn from(config: CenteringModeConfig) -> Self {
        match config {
            CenteringModeConfig::Center => CenteringMode::Center,
            CenteringModeConfig::JustInView => CenteringMode::JustInView,
        }
    }
}

/// Appearance-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    /// Whether to use DWM cloaking for off-screen windows.
    #[serde(default = "default_true")]
    pub use_cloaking: bool,

    /// Whether to use batched window positioning (DeferWindowPos).
    #[serde(default = "default_true")]
    pub use_deferred_positioning: bool,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            use_cloaking: true,
            use_deferred_positioning: true,
        }
    }
}

/// Behavior-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    /// Whether to focus new windows automatically.
    #[serde(default = "default_true")]
    pub focus_new_windows: bool,

    /// Whether to track window focus changes from Windows.
    #[serde(default = "default_true")]
    pub track_focus_changes: bool,

    /// Log level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Whether focus follows the mouse cursor.
    /// When enabled, windows receive focus when the mouse enters them.
    #[serde(default = "default_false")]
    pub focus_follows_mouse: bool,

    /// Delay in milliseconds before focus changes on mouse enter.
    /// Only applies when focus_follows_mouse is true.
    #[serde(default = "default_focus_delay")]
    pub focus_follows_mouse_delay_ms: u32,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            focus_new_windows: true,
            track_focus_changes: true,
            log_level: default_log_level(),
            focus_follows_mouse: false,
            focus_follows_mouse_delay_ms: default_focus_delay(),
        }
    }
}

// Default value functions for serde
fn default_gap() -> i32 {
    10
}

fn default_outer_gap() -> i32 {
    10
}

fn default_column_width() -> i32 {
    800
}

fn default_min_column_width() -> i32 {
    400
}

fn default_max_column_width() -> i32 {
    1600
}

fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_focus_delay() -> u32 {
    100
}

// ============================================================================
// Window Rules
// ============================================================================

/// A rule for per-window behavior.
///
/// Window rules are evaluated in order; the first matching rule wins.
///
/// # Example Config
///
/// ```toml
/// [[window_rules]]
/// match_class = "Chrome_WidgetWin_1"
/// match_title = ".*DevTools.*"
/// action = "float"
///
/// [[window_rules]]
/// match_executable = "spotify.exe"
/// action = "float"
///
/// [[window_rules]]
/// match_class = "#32770"  # Windows dialogs
/// action = "ignore"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowRule {
    /// Regex pattern to match window class name.
    #[serde(default)]
    pub match_class: Option<String>,

    /// Regex pattern to match window title.
    #[serde(default)]
    pub match_title: Option<String>,

    /// Executable name to match (e.g., "notepad.exe").
    #[serde(default)]
    pub match_executable: Option<String>,

    /// Action to take when the rule matches.
    #[serde(default)]
    pub action: WindowAction,

    /// Fixed width for floating windows (optional).
    #[serde(default)]
    pub width: Option<i32>,

    /// Fixed height for floating windows (optional).
    #[serde(default)]
    pub height: Option<i32>,
}

/// Action to take for a matching window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowAction {
    /// Tile the window normally (default behavior).
    #[default]
    Tile,
    /// Float the window outside the tiling layout.
    Float,
    /// Ignore the window (don't manage it at all).
    Ignore,
}

impl WindowRule {
    /// Check if this rule matches a window with the given properties.
    ///
    /// All specified match criteria must match for the rule to apply.
    /// If no match criteria are specified, the rule matches nothing.
    pub fn matches(&self, class_name: &str, title: &str, executable: &str) -> bool {
        let has_any_criteria = self.match_class.is_some()
            || self.match_title.is_some()
            || self.match_executable.is_some();

        if !has_any_criteria {
            return false;
        }

        // Check class name if specified
        if let Some(ref pattern) = self.match_class {
            if let Ok(re) = regex::Regex::new(pattern) {
                if !re.is_match(class_name) {
                    return false;
                }
            } else {
                tracing::warn!("Invalid regex in window rule match_class: {}", pattern);
                return false;
            }
        }

        // Check title if specified
        if let Some(ref pattern) = self.match_title {
            if let Ok(re) = regex::Regex::new(pattern) {
                if !re.is_match(title) {
                    return false;
                }
            } else {
                tracing::warn!("Invalid regex in window rule match_title: {}", pattern);
                return false;
            }
        }

        // Check executable if specified (case-insensitive)
        if let Some(ref exe) = self.match_executable {
            if !executable.eq_ignore_ascii_case(exe) {
                return false;
            }
        }

        true
    }
}

/// Hotkey bindings configuration.
///
/// Each key is a hotkey string (e.g., "Win+H") and each value is a command
/// (e.g., "focus_left"). Supported commands:
/// - focus_left, focus_right, focus_up, focus_down
/// - move_column_left, move_column_right
/// - focus_monitor_left, focus_monitor_right
/// - move_to_monitor_left, move_to_monitor_right
/// - resize_grow, resize_shrink (by 50px)
/// - scroll_left, scroll_right (by 100px)
/// - refresh, reload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    /// Map of hotkey string to command name.
    #[serde(flatten)]
    pub bindings: HashMap<String, String>,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        let mut bindings = HashMap::new();

        // Default vim-style navigation with Win key
        bindings.insert("Win+H".to_string(), "focus_left".to_string());
        bindings.insert("Win+L".to_string(), "focus_right".to_string());
        bindings.insert("Win+J".to_string(), "focus_down".to_string());
        bindings.insert("Win+K".to_string(), "focus_up".to_string());

        // Move columns with Win+Shift
        bindings.insert("Win+Shift+H".to_string(), "move_column_left".to_string());
        bindings.insert("Win+Shift+L".to_string(), "move_column_right".to_string());

        // Resize with Win+Ctrl
        bindings.insert("Win+Ctrl+H".to_string(), "resize_shrink".to_string());
        bindings.insert("Win+Ctrl+L".to_string(), "resize_grow".to_string());

        // Monitor navigation with Win+Alt
        bindings.insert("Win+Alt+H".to_string(), "focus_monitor_left".to_string());
        bindings.insert("Win+Alt+L".to_string(), "focus_monitor_right".to_string());

        // Move to monitor with Win+Alt+Shift
        bindings.insert("Win+Alt+Shift+H".to_string(), "move_to_monitor_left".to_string());
        bindings.insert("Win+Alt+Shift+L".to_string(), "move_to_monitor_right".to_string());

        // Utility
        bindings.insert("Win+R".to_string(), "refresh".to_string());

        Self { bindings }
    }
}

/// Gesture bindings for touchpad support.
///
/// Maps touchpad gestures to commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GestureConfig {
    /// Whether gesture support is enabled.
    #[serde(default = "default_false")]
    pub enabled: bool,

    /// Command for three-finger swipe left.
    #[serde(default = "default_swipe_left")]
    pub swipe_left: String,

    /// Command for three-finger swipe right.
    #[serde(default = "default_swipe_right")]
    pub swipe_right: String,

    /// Command for three-finger swipe up.
    #[serde(default = "default_swipe_up")]
    pub swipe_up: String,

    /// Command for three-finger swipe down.
    #[serde(default = "default_swipe_down")]
    pub swipe_down: String,
}

fn default_false() -> bool {
    false
}

fn default_swipe_left() -> String {
    "focus_left".to_string()
}

fn default_swipe_right() -> String {
    "focus_right".to_string()
}

fn default_swipe_up() -> String {
    "focus_up".to_string()
}

fn default_swipe_down() -> String {
    "focus_down".to_string()
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            swipe_left: default_swipe_left(),
            swipe_right: default_swipe_right(),
            swipe_up: default_swipe_up(),
            swipe_down: default_swipe_down(),
        }
    }
}

/// Configuration for visual snap hints.
///
/// Snap hints provide visual feedback during resize operations,
/// showing column boundaries and snap targets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SnapHintConfig {
    /// Whether snap hints are enabled.
    #[serde(default = "default_false")]
    pub enabled: bool,

    /// Duration to show hints in milliseconds.
    #[serde(default = "default_hint_duration")]
    pub duration_ms: u32,

    /// Opacity of the hint overlay (0-255).
    #[serde(default = "default_hint_opacity")]
    pub opacity: u8,
}

fn default_hint_duration() -> u32 {
    200
}

fn default_hint_opacity() -> u8 {
    128
}

impl Default for SnapHintConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            duration_ms: default_hint_duration(),
            opacity: default_hint_opacity(),
        }
    }
}

/// Parse a command string into an IpcCommand.
///
/// Returns None if the command is not recognized.
pub fn parse_command(cmd: &str) -> Option<openniri_ipc::IpcCommand> {
    use openniri_ipc::IpcCommand;

    match cmd.to_lowercase().as_str() {
        "focus_left" => Some(IpcCommand::FocusLeft),
        "focus_right" => Some(IpcCommand::FocusRight),
        "focus_up" => Some(IpcCommand::FocusUp),
        "focus_down" => Some(IpcCommand::FocusDown),
        "move_column_left" => Some(IpcCommand::MoveColumnLeft),
        "move_column_right" => Some(IpcCommand::MoveColumnRight),
        "focus_monitor_left" => Some(IpcCommand::FocusMonitorLeft),
        "focus_monitor_right" => Some(IpcCommand::FocusMonitorRight),
        "move_to_monitor_left" => Some(IpcCommand::MoveWindowToMonitorLeft),
        "move_to_monitor_right" => Some(IpcCommand::MoveWindowToMonitorRight),
        "resize_grow" => Some(IpcCommand::Resize { delta: 50 }),
        "resize_shrink" => Some(IpcCommand::Resize { delta: -50 }),
        "scroll_left" => Some(IpcCommand::Scroll { delta: -100.0 }),
        "scroll_right" => Some(IpcCommand::Scroll { delta: 100.0 }),
        "refresh" => Some(IpcCommand::Refresh),
        "reload" => Some(IpcCommand::Reload),
        _ => None,
    }
}

impl Config {
    /// Load configuration from standard locations.
    ///
    /// Tries the following locations in order:
    /// 1. `%APPDATA%/openniri/config.toml`
    /// 2. `~/.config/openniri/config.toml`
    /// 3. `./config.toml`
    ///
    /// Returns default config if no file is found.
    pub fn load() -> Result<Self> {
        let paths = config_paths();

        for path in &paths {
            if path.exists() {
                tracing::info!("Loading config from: {}", path.display());
                return Self::load_from_path(path);
            }
        }

        tracing::info!("No config file found, using defaults");
        Ok(Self::default())
    }

    /// Load configuration from a specific path.
    pub fn load_from_path(path: &PathBuf) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok(config)
    }
}

/// Get all possible config file paths in priority order.
pub fn config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. Windows standard: %APPDATA%/openniri/config.toml
    if let Some(proj_dirs) = ProjectDirs::from("com", "openniri", "openniri") {
        paths.push(proj_dirs.config_dir().join("config.toml"));
    }

    // 2. Unix-style: ~/.config/openniri/config.toml
    if let Some(home) = dirs_home() {
        paths.push(home.join(".config").join("openniri").join("config.toml"));
    }

    // 3. Current directory: ./config.toml
    paths.push(PathBuf::from("config.toml"));

    paths
}

/// Get the user's home directory.
fn dirs_home() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.layout.gap, 10);
        assert_eq!(config.layout.outer_gap, 10);
        assert_eq!(config.layout.default_column_width, 800);
        assert_eq!(config.layout.centering_mode, CenteringModeConfig::Center);
        assert!(config.appearance.use_cloaking);
        assert!(config.behavior.focus_new_windows);
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.layout.gap, config.layout.gap);
        assert_eq!(parsed.layout.centering_mode, config.layout.centering_mode);
    }

    #[test]
    fn test_config_partial_parse() {
        // Config with only some fields should use defaults for the rest
        let toml_str = r#"
            [layout]
            gap = 20
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.layout.gap, 20);
        assert_eq!(config.layout.outer_gap, 10); // default
        assert_eq!(config.layout.default_column_width, 800); // default
    }

    #[test]
    fn test_centering_mode_conversion() {
        let config_center = CenteringModeConfig::Center;
        let config_just_in_view = CenteringModeConfig::JustInView;

        let mode_center: CenteringMode = config_center.into();
        let mode_just_in_view: CenteringMode = config_just_in_view.into();

        assert_eq!(mode_center, CenteringMode::Center);
        assert_eq!(mode_just_in_view, CenteringMode::JustInView);
    }

    #[test]
    fn test_config_paths_not_empty() {
        let paths = config_paths();
        assert!(!paths.is_empty());
    }

    #[test]
    fn test_hotkey_config_default() {
        let config = HotkeyConfig::default();
        assert!(!config.bindings.is_empty());
        assert_eq!(config.bindings.get("Win+H"), Some(&"focus_left".to_string()));
        assert_eq!(config.bindings.get("Win+L"), Some(&"focus_right".to_string()));
        assert_eq!(config.bindings.get("Win+Shift+H"), Some(&"move_column_left".to_string()));
    }

    #[test]
    fn test_parse_command() {
        use openniri_ipc::IpcCommand;

        assert_eq!(parse_command("focus_left"), Some(IpcCommand::FocusLeft));
        assert_eq!(parse_command("FOCUS_RIGHT"), Some(IpcCommand::FocusRight));
        assert_eq!(parse_command("move_column_left"), Some(IpcCommand::MoveColumnLeft));
        assert_eq!(parse_command("focus_monitor_left"), Some(IpcCommand::FocusMonitorLeft));
        assert_eq!(parse_command("resize_grow"), Some(IpcCommand::Resize { delta: 50 }));
        assert_eq!(parse_command("resize_shrink"), Some(IpcCommand::Resize { delta: -50 }));
        assert_eq!(parse_command("refresh"), Some(IpcCommand::Refresh));
        assert_eq!(parse_command("unknown_command"), None);
    }

    #[test]
    fn test_hotkey_config_serialization() {
        let toml_str = r#"
            [hotkeys]
            "Win+A" = "focus_left"
            "Ctrl+Alt+B" = "focus_right"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.hotkeys.bindings.get("Win+A"), Some(&"focus_left".to_string()));
        assert_eq!(config.hotkeys.bindings.get("Ctrl+Alt+B"), Some(&"focus_right".to_string()));
    }

    #[test]
    fn test_column_width_bounds_defaults() {
        let config = Config::default();
        // Verify default bounds are sensible
        assert_eq!(config.layout.min_column_width, 400);
        assert_eq!(config.layout.max_column_width, 1600);
        assert!(config.layout.min_column_width < config.layout.max_column_width);
        assert!(config.layout.default_column_width >= config.layout.min_column_width);
        assert!(config.layout.default_column_width <= config.layout.max_column_width);
    }

    #[test]
    fn test_column_width_bounds_custom() {
        let toml_str = r#"
            [layout]
            min_column_width = 300
            max_column_width = 2000
            default_column_width = 1000
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.layout.min_column_width, 300);
        assert_eq!(config.layout.max_column_width, 2000);
        assert_eq!(config.layout.default_column_width, 1000);
    }

    #[test]
    fn test_width_clamping_logic() {
        let config = Config::default();
        // Simulate the clamping logic used in daemon
        let too_small = 200i32;
        let too_large = 2000i32;
        let just_right = 800i32;

        let clamped_small = too_small.clamp(
            config.layout.min_column_width,
            config.layout.max_column_width,
        );
        let clamped_large = too_large.clamp(
            config.layout.min_column_width,
            config.layout.max_column_width,
        );
        let clamped_right = just_right.clamp(
            config.layout.min_column_width,
            config.layout.max_column_width,
        );

        assert_eq!(clamped_small, 400); // Clamped to min
        assert_eq!(clamped_large, 1600); // Clamped to max
        assert_eq!(clamped_right, 800); // Unchanged
    }

    #[test]
    fn test_window_rule_matches_class() {
        let rule = WindowRule {
            match_class: Some("Notepad".to_string()),
            match_title: None,
            match_executable: None,
            action: WindowAction::Float,
            width: None,
            height: None,
        };

        assert!(rule.matches("Notepad", "Untitled - Notepad", "notepad.exe"));
        assert!(!rule.matches("Chrome_WidgetWin_1", "Google Chrome", "chrome.exe"));
    }

    #[test]
    fn test_window_rule_matches_title_regex() {
        let rule = WindowRule {
            match_class: None,
            match_title: Some(".*DevTools.*".to_string()),
            match_executable: None,
            action: WindowAction::Float,
            width: Some(800),
            height: Some(600),
        };

        assert!(rule.matches("Chrome_WidgetWin_1", "DevTools - localhost:3000", "chrome.exe"));
        assert!(rule.matches("SomeClass", "Firefox DevTools", "firefox.exe"));
        assert!(!rule.matches("Chrome_WidgetWin_1", "Google Chrome", "chrome.exe"));
    }

    #[test]
    fn test_window_rule_matches_executable() {
        let rule = WindowRule {
            match_class: None,
            match_title: None,
            match_executable: Some("spotify.exe".to_string()),
            action: WindowAction::Float,
            width: None,
            height: None,
        };

        assert!(rule.matches("SpotifyClass", "Spotify - Song Title", "spotify.exe"));
        assert!(rule.matches("SpotifyClass", "Spotify - Song Title", "SPOTIFY.EXE")); // Case insensitive
        assert!(!rule.matches("SpotifyClass", "Spotify - Song Title", "chrome.exe"));
    }

    #[test]
    fn test_window_rule_matches_combined() {
        let rule = WindowRule {
            match_class: Some("Chrome.*".to_string()),
            match_title: Some(".*YouTube.*".to_string()),
            match_executable: None,
            action: WindowAction::Tile,
            width: None,
            height: None,
        };

        // Both patterns must match
        assert!(rule.matches("Chrome_WidgetWin_1", "YouTube - Google Chrome", "chrome.exe"));
        assert!(!rule.matches("Firefox", "YouTube - Mozilla Firefox", "firefox.exe")); // Class doesn't match
        assert!(!rule.matches("Chrome_WidgetWin_1", "Google Chrome", "chrome.exe")); // Title doesn't match
    }

    #[test]
    fn test_window_rule_no_criteria_matches_nothing() {
        let rule = WindowRule {
            match_class: None,
            match_title: None,
            match_executable: None,
            action: WindowAction::Ignore,
            width: None,
            height: None,
        };

        assert!(!rule.matches("AnyClass", "Any Title", "any.exe"));
    }

    #[test]
    fn test_window_rule_config_parse() {
        let toml_str = r#"
            [[window_rules]]
            match_class = "Notepad"
            action = "float"
            width = 800
            height = 600

            [[window_rules]]
            match_executable = "spotify.exe"
            action = "float"

            [[window_rules]]
            match_title = ".*dialog.*"
            action = "ignore"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.window_rules.len(), 3);

        assert_eq!(config.window_rules[0].match_class, Some("Notepad".to_string()));
        assert_eq!(config.window_rules[0].action, WindowAction::Float);
        assert_eq!(config.window_rules[0].width, Some(800));
        assert_eq!(config.window_rules[0].height, Some(600));

        assert_eq!(config.window_rules[1].match_executable, Some("spotify.exe".to_string()));
        assert_eq!(config.window_rules[1].action, WindowAction::Float);

        assert_eq!(config.window_rules[2].match_title, Some(".*dialog.*".to_string()));
        assert_eq!(config.window_rules[2].action, WindowAction::Ignore);
    }

    #[test]
    fn test_window_action_default() {
        let action = WindowAction::default();
        assert_eq!(action, WindowAction::Tile);
    }

    #[test]
    fn test_snap_hint_config_default() {
        let config = SnapHintConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.duration_ms, 200);
        assert_eq!(config.opacity, 128);
    }

    #[test]
    fn test_snap_hint_config_serialization() {
        let toml_str = r#"
            [snap_hints]
            enabled = true
            duration_ms = 300
            opacity = 200
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.snap_hints.enabled);
        assert_eq!(config.snap_hints.duration_ms, 300);
        assert_eq!(config.snap_hints.opacity, 200);
    }

    #[test]
    fn test_focus_follows_mouse_default() {
        let config = Config::default();
        assert!(!config.behavior.focus_follows_mouse);
        assert_eq!(config.behavior.focus_follows_mouse_delay_ms, 100);
    }

    #[test]
    fn test_focus_follows_mouse_serialization() {
        let toml_str = r#"
            [behavior]
            focus_follows_mouse = true
            focus_follows_mouse_delay_ms = 200
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.behavior.focus_follows_mouse);
        assert_eq!(config.behavior.focus_follows_mouse_delay_ms, 200);
    }
}
