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

    /// Whether to highlight the active window border (Windows 11+).
    #[serde(default = "default_true")]
    pub active_border: bool,

    /// Active window border color as hex RGB (e.g., "4285F4").
    #[serde(default = "default_active_border_color")]
    pub active_border_color: String,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            use_cloaking: true,
            use_deferred_positioning: true,
            active_border: true,
            active_border_color: default_active_border_color(),
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

fn default_active_border_color() -> String {
    "4285F4".to_string()
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
    ///
    /// Note: Runtime code uses `CompiledWindowRule::matches()` for efficiency.
    /// This method is retained for tests and direct use.
    #[allow(dead_code)]
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
/// - panic_revert (emergency visibility restore + shutdown)
/// - toggle_pause (pause/resume tiling)
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

        // Default vim-style navigation with Alt (Win+letter combos are mostly
        // reserved by Windows 11, and Win+L can never be registered)
        bindings.insert("Alt+H".to_string(), "focus_left".to_string());
        bindings.insert("Alt+L".to_string(), "focus_right".to_string());
        bindings.insert("Alt+J".to_string(), "focus_down".to_string());
        bindings.insert("Alt+K".to_string(), "focus_up".to_string());

        // Move columns with Alt+Shift
        bindings.insert("Alt+Shift+H".to_string(), "move_column_left".to_string());
        bindings.insert("Alt+Shift+L".to_string(), "move_column_right".to_string());

        // Resize with Alt+Ctrl
        bindings.insert("Alt+Ctrl+H".to_string(), "resize_shrink".to_string());
        bindings.insert("Alt+Ctrl+L".to_string(), "resize_grow".to_string());

        // Monitor navigation with Win+Alt
        bindings.insert("Win+Alt+H".to_string(), "focus_monitor_left".to_string());
        bindings.insert("Win+Alt+L".to_string(), "focus_monitor_right".to_string());

        // Move to monitor with Win+Alt+Shift
        bindings.insert(
            "Win+Alt+Shift+H".to_string(),
            "move_to_monitor_left".to_string(),
        );
        bindings.insert(
            "Win+Alt+Shift+L".to_string(),
            "move_to_monitor_right".to_string(),
        );

        // Utility
        bindings.insert("Alt+Shift+R".to_string(), "refresh".to_string());
        // Emergency escape hatch: revert visibility state and stop daemon.
        bindings.insert("Win+Ctrl+Escape".to_string(), "panic_revert".to_string());

        // Close focused window
        bindings.insert("Alt+Shift+Q".to_string(), "close_window".to_string());
        // Toggle floating
        bindings.insert("Alt+F".to_string(), "toggle_floating".to_string());
        // Toggle fullscreen
        bindings.insert("Alt+Shift+F".to_string(), "toggle_fullscreen".to_string());
        // Column width presets
        bindings.insert("Alt+1".to_string(), "width_third".to_string());
        bindings.insert("Alt+2".to_string(), "width_half".to_string());
        bindings.insert("Alt+3".to_string(), "width_two_thirds".to_string());
        bindings.insert("Alt+0".to_string(), "equalize_widths".to_string());

        Self { bindings }
    }
}

/// Gesture bindings for touchpad support.
///
/// Maps touchpad gestures to commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GestureConfig {
    /// Whether gesture support is enabled (experimental, off by default).
    ///
    /// Swipes are detected by accumulating mouse-wheel deltas, so ordinary
    /// scrolling can trigger them.
    #[serde(default)]
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
    #[serde(default = "default_true")]
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
            enabled: true,
            duration_ms: default_hint_duration(),
            opacity: default_hint_opacity(),
        }
    }
}

/// A warning generated during config validation.
#[derive(Debug, Clone)]
pub struct ConfigWarning {
    pub field: String,
    pub message: String,
}

/// A window rule with pre-compiled regex patterns for efficient matching.
#[derive(Debug, Clone)]
pub struct CompiledWindowRule {
    /// Pre-compiled regex for class name matching.
    pub class_regex: Option<regex::Regex>,
    /// Pre-compiled regex for title matching.
    pub title_regex: Option<regex::Regex>,
    /// Executable name to match (case-insensitive string comparison).
    pub match_executable: Option<String>,
    /// Action to take when the rule matches.
    pub action: WindowAction,
    /// Fixed width for floating windows (optional).
    pub width: Option<i32>,
    /// Fixed height for floating windows (optional).
    pub height: Option<i32>,
}

impl CompiledWindowRule {
    /// Check if this compiled rule matches a window.
    pub fn matches(&self, class_name: &str, title: &str, executable: &str) -> bool {
        let has_any_criteria = self.class_regex.is_some()
            || self.title_regex.is_some()
            || self.match_executable.is_some();

        if !has_any_criteria {
            return false;
        }

        if let Some(ref re) = self.class_regex {
            if !re.is_match(class_name) {
                return false;
            }
        }

        if let Some(ref re) = self.title_regex {
            if !re.is_match(title) {
                return false;
            }
        }

        if let Some(ref exe) = self.match_executable {
            if !executable.eq_ignore_ascii_case(exe) {
                return false;
            }
        }

        true
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
        "panic_revert" | "panic-revert" => Some(IpcCommand::PanicRevert),
        "toggle_pause" | "toggle-pause" => Some(IpcCommand::TogglePause),
        "close_window" => Some(IpcCommand::CloseWindow),
        "toggle_floating" => Some(IpcCommand::ToggleFloating),
        "toggle_fullscreen" => Some(IpcCommand::ToggleFullscreen),
        "width_third" => Some(IpcCommand::SetColumnWidth { fraction: 0.333 }),
        "width_half" => Some(IpcCommand::SetColumnWidth { fraction: 0.5 }),
        "width_two_thirds" => Some(IpcCommand::SetColumnWidth { fraction: 0.667 }),
        "equalize_widths" => Some(IpcCommand::EqualizeColumnWidths),
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

    /// Validate configuration values, clamping out-of-range fields and returning warnings.
    pub fn validate(&mut self) -> Vec<ConfigWarning> {
        let mut warnings = Vec::new();

        // gap must be >= 0
        if self.layout.gap < 0 {
            warnings.push(ConfigWarning {
                field: "layout.gap".to_string(),
                message: format!("Negative gap ({}) clamped to 0", self.layout.gap),
            });
            self.layout.gap = 0;
        }

        // outer_gap must be >= 0
        if self.layout.outer_gap < 0 {
            warnings.push(ConfigWarning {
                field: "layout.outer_gap".to_string(),
                message: format!(
                    "Negative outer_gap ({}) clamped to 0",
                    self.layout.outer_gap
                ),
            });
            self.layout.outer_gap = 0;
        }

        // min_column_width must be >= 0
        if self.layout.min_column_width < 0 {
            warnings.push(ConfigWarning {
                field: "layout.min_column_width".to_string(),
                message: format!(
                    "Negative min_column_width ({}) clamped to 0",
                    self.layout.min_column_width
                ),
            });
            self.layout.min_column_width = 0;
        }

        // max_column_width must be >= 0
        if self.layout.max_column_width < 0 {
            warnings.push(ConfigWarning {
                field: "layout.max_column_width".to_string(),
                message: format!(
                    "Negative max_column_width ({}) clamped to 0",
                    self.layout.max_column_width
                ),
            });
            self.layout.max_column_width = 0;
        }

        // default_column_width must be >= 0
        if self.layout.default_column_width < 0 {
            warnings.push(ConfigWarning {
                field: "layout.default_column_width".to_string(),
                message: format!(
                    "Negative default_column_width ({}) clamped to 0",
                    self.layout.default_column_width
                ),
            });
            self.layout.default_column_width = 0;
        }

        // min_column_width must be <= max_column_width
        if self.layout.min_column_width > self.layout.max_column_width {
            warnings.push(ConfigWarning {
                field: "layout.min_column_width / layout.max_column_width".to_string(),
                message: format!(
                    "min_column_width ({}) > max_column_width ({}), swapping",
                    self.layout.min_column_width, self.layout.max_column_width
                ),
            });
            std::mem::swap(
                &mut self.layout.min_column_width,
                &mut self.layout.max_column_width,
            );
        }

        // default_column_width must be in [min, max]
        if self.layout.default_column_width < self.layout.min_column_width
            || self.layout.default_column_width > self.layout.max_column_width
        {
            let clamped = self
                .layout
                .default_column_width
                .clamp(self.layout.min_column_width, self.layout.max_column_width);
            warnings.push(ConfigWarning {
                field: "layout.default_column_width".to_string(),
                message: format!(
                    "default_column_width ({}) outside [{}, {}], clamped to {}",
                    self.layout.default_column_width,
                    self.layout.min_column_width,
                    self.layout.max_column_width,
                    clamped,
                ),
            });
            self.layout.default_column_width = clamped;
        }

        // focus_follows_mouse_delay_ms must be >= 50 when enabled
        if self.behavior.focus_follows_mouse && self.behavior.focus_follows_mouse_delay_ms < 50 {
            warnings.push(ConfigWarning {
                field: "behavior.focus_follows_mouse_delay_ms".to_string(),
                message: format!(
                    "focus_follows_mouse_delay_ms ({}) below minimum 50, clamped to 50",
                    self.behavior.focus_follows_mouse_delay_ms
                ),
            });
            self.behavior.focus_follows_mouse_delay_ms = 50;
        }

        // snap_hints.duration_ms must be >= 50 when enabled
        if self.snap_hints.enabled && self.snap_hints.duration_ms < 50 {
            warnings.push(ConfigWarning {
                field: "snap_hints.duration_ms".to_string(),
                message: format!(
                    "snap_hints.duration_ms ({}) below minimum 50, clamped to 50",
                    self.snap_hints.duration_ms
                ),
            });
            self.snap_hints.duration_ms = 50;
        }

        // active_border_color must be exactly 6 hex characters
        {
            let color = &self.appearance.active_border_color;
            let is_valid = color.len() == 6 && color.chars().all(|c| c.is_ascii_hexdigit());
            if !is_valid {
                warnings.push(ConfigWarning {
                    field: "appearance.active_border_color".to_string(),
                    message: format!(
                        "Invalid hex color '{}' (must be 6 hex chars, e.g. \"4285F4\"), reset to default",
                        color
                    ),
                });
                self.appearance.active_border_color = default_active_border_color();
            }
        }

        // behavior.log_level must be one of trace/debug/info/warn/error
        {
            let normalized = self.behavior.log_level.to_lowercase();
            let valid = matches!(
                normalized.as_str(),
                "trace" | "debug" | "info" | "warn" | "error"
            );
            if !valid {
                warnings.push(ConfigWarning {
                    field: "behavior.log_level".to_string(),
                    message: format!(
                        "Invalid log_level '{}' (must be trace/debug/info/warn/error), reset to default",
                        self.behavior.log_level
                    ),
                });
                self.behavior.log_level = default_log_level();
            }
        }

        warnings
    }

    /// Compile window rules into pre-compiled regex patterns for efficient matching.
    ///
    /// Invalid regex patterns are logged as warnings and their rules are skipped.
    pub fn compile_window_rules(&self) -> Vec<CompiledWindowRule> {
        let mut compiled = Vec::new();

        for rule in &self.window_rules {
            let class_regex = match &rule.match_class {
                Some(pattern) => match regex::RegexBuilder::new(pattern)
                    .size_limit(1_000_000)
                    .build()
                {
                    Ok(re) => Some(re),
                    Err(e) => {
                        tracing::warn!(
                            "Invalid regex in window rule match_class '{}': {}. Skipping rule.",
                            pattern,
                            e
                        );
                        continue;
                    }
                },
                None => None,
            };

            let title_regex = match &rule.match_title {
                Some(pattern) => match regex::RegexBuilder::new(pattern)
                    .size_limit(1_000_000)
                    .build()
                {
                    Ok(re) => Some(re),
                    Err(e) => {
                        tracing::warn!(
                            "Invalid regex in window rule match_title '{}': {}. Skipping rule.",
                            pattern,
                            e
                        );
                        continue;
                    }
                },
                None => None,
            };

            compiled.push(CompiledWindowRule {
                class_regex,
                title_regex,
                match_executable: rule.match_executable.clone(),
                action: rule.action,
                width: rule.width,
                height: rule.height,
            });
        }

        compiled
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

    // 1. Windows standard: %APPDATA%/openniri/config/config.toml
    if let Some(proj_dirs) = ProjectDirs::from("", "", "openniri") {
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

    fn validated(edit: impl FnOnce(&mut Config)) -> (Config, Vec<ConfigWarning>) {
        let mut config = Config::default();
        edit(&mut config);
        let warnings = config.validate();
        (config, warnings)
    }

    fn has_warning(warnings: &[ConfigWarning], field: &str) -> bool {
        warnings.iter().any(|w| w.field == field)
    }

    fn rule(class: Option<&str>, title: Option<&str>, exe: Option<&str>) -> WindowRule {
        WindowRule {
            match_class: class.map(str::to_string),
            match_title: title.map(str::to_string),
            match_executable: exe.map(str::to_string),
            action: WindowAction::Float,
            width: None,
            height: None,
        }
    }

    #[test]
    fn test_default_config_safety_relevant_values() {
        let mut config = Config::default();
        assert!(config.appearance.use_cloaking);
        assert!(
            !config.gestures.enabled,
            "gestures are experimental and must be opt-in"
        );
        assert!(!config.behavior.focus_follows_mouse);

        let bindings = &config.hotkeys.bindings;
        assert_eq!(
            bindings.get("Alt+H").map(String::as_str),
            Some("focus_left")
        );
        assert_eq!(
            bindings.get("Alt+L").map(String::as_str),
            Some("focus_right")
        );
        assert_eq!(
            bindings.get("Win+Ctrl+Escape").map(String::as_str),
            Some("panic_revert"),
            "the panic key must stay bound by default"
        );
        for (key, command) in bindings {
            assert!(
                parse_command(command).is_some(),
                "default binding {key} maps to unknown command {command}"
            );
            assert!(
                !key.starts_with("Win+") || key.contains("Alt") || key.contains("Ctrl"),
                "bare Win+key bindings are reserved by Windows: {key}"
            );
        }

        let warnings = config.validate();
        assert!(
            warnings.is_empty(),
            "default config must validate cleanly: {warnings:?}"
        );
    }

    #[test]
    fn test_full_config_parse() {
        let toml_str = r#"
            [layout]
            gap = 4
            outer_gap = 6
            default_column_width = 1000
            min_column_width = 300
            max_column_width = 2000
            centering_mode = "just_in_view"

            [appearance]
            use_cloaking = false
            use_deferred_positioning = false
            active_border = false
            active_border_color = "FF0000"

            [behavior]
            focus_new_windows = false
            track_focus_changes = false
            log_level = "debug"
            focus_follows_mouse = true
            focus_follows_mouse_delay_ms = 200

            [hotkeys]
            "Win+Alt+A" = "focus_left"

            [gestures]
            enabled = true
            swipe_left = "scroll_left"

            [snap_hints]
            enabled = false
            duration_ms = 300
            opacity = 200

            [[window_rules]]
            match_class = "Notepad"
            action = "float"
            width = 800

            [[window_rules]]
            match_title = ".*dialog.*"
            action = "ignore"
            height = 600
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();

        assert_eq!(config.layout.gap, 4);
        assert_eq!(config.layout.outer_gap, 6);
        assert_eq!(config.layout.default_column_width, 1000);
        assert_eq!(config.layout.min_column_width, 300);
        assert_eq!(config.layout.max_column_width, 2000);
        assert_eq!(
            CenteringMode::from(config.layout.centering_mode),
            CenteringMode::JustInView
        );
        assert!(!config.appearance.use_cloaking);
        assert!(!config.appearance.use_deferred_positioning);
        assert!(!config.appearance.active_border);
        assert_eq!(config.appearance.active_border_color, "FF0000");
        assert!(!config.behavior.focus_new_windows);
        assert!(!config.behavior.track_focus_changes);
        assert_eq!(config.behavior.log_level, "debug");
        assert!(config.behavior.focus_follows_mouse);
        assert_eq!(config.behavior.focus_follows_mouse_delay_ms, 200);
        assert_eq!(
            config.hotkeys.bindings.len(),
            1,
            "user hotkeys replace the defaults"
        );
        assert_eq!(
            config.hotkeys.bindings.get("Win+Alt+A").map(String::as_str),
            Some("focus_left")
        );
        assert!(config.gestures.enabled);
        assert_eq!(config.gestures.swipe_left, "scroll_left");
        assert_eq!(config.gestures.swipe_right, "focus_right");
        assert!(!config.snap_hints.enabled);
        assert_eq!(config.snap_hints.duration_ms, 300);
        assert_eq!(config.snap_hints.opacity, 200);

        assert_eq!(config.window_rules.len(), 2);
        assert_eq!(
            config.window_rules[0].match_class.as_deref(),
            Some("Notepad")
        );
        assert_eq!(config.window_rules[0].action, WindowAction::Float);
        assert_eq!(config.window_rules[0].width, Some(800));
        assert_eq!(config.window_rules[0].height, None);
        assert_eq!(
            config.window_rules[1].match_title.as_deref(),
            Some(".*dialog.*")
        );
        assert_eq!(config.window_rules[1].action, WindowAction::Ignore);
        assert_eq!(config.window_rules[1].width, None);
        assert_eq!(config.window_rules[1].height, Some(600));
    }

    #[test]
    fn test_partial_config_uses_defaults_and_ignores_unknown_keys() {
        let toml_str = r#"
            totally_unknown_section = "hello"

            [layout]
            gap = 20
            nonexistent_field = true

            [[window_rules]]
            match_executable = "spotify.exe"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let default = Config::default();

        assert_eq!(config.layout.gap, 20);
        assert_eq!(config.layout.outer_gap, default.layout.outer_gap);
        assert_eq!(
            config.layout.default_column_width,
            default.layout.default_column_width
        );
        assert!(config.appearance.use_cloaking);
        assert!(config.behavior.focus_new_windows);
        assert!(!config.gestures.enabled);
        assert_eq!(config.hotkeys.bindings, default.hotkeys.bindings);
        assert_eq!(config.window_rules[0].action, WindowAction::Tile);

        let empty: Config = toml::from_str("").unwrap();
        assert_eq!(empty.layout.gap, default.layout.gap);
        assert_eq!(empty.hotkeys.bindings, default.hotkeys.bindings);
        assert!(empty.window_rules.is_empty());
    }

    #[test]
    fn test_invalid_toml_returns_error() {
        let cases = [
            ("missing bracket", "[layout\ngap = 10\n"),
            ("wrong type", "[layout]\ngap = \"not_a_number\"\n"),
            (
                "unknown enum value",
                "[layout]\ncentering_mode = \"left\"\n",
            ),
        ];
        for (name, toml_str) in cases {
            assert!(
                toml::from_str::<Config>(toml_str).is_err(),
                "{name} should fail to parse"
            );
        }
    }

    #[test]
    fn test_parse_command() {
        use openniri_ipc::IpcCommand;

        let cases = [
            ("focus_left", Some(IpcCommand::FocusLeft)),
            ("FOCUS_RIGHT", Some(IpcCommand::FocusRight)),
            ("focus_up", Some(IpcCommand::FocusUp)),
            ("focus_down", Some(IpcCommand::FocusDown)),
            ("move_column_left", Some(IpcCommand::MoveColumnLeft)),
            ("move_column_right", Some(IpcCommand::MoveColumnRight)),
            ("focus_monitor_left", Some(IpcCommand::FocusMonitorLeft)),
            ("focus_monitor_right", Some(IpcCommand::FocusMonitorRight)),
            (
                "move_to_monitor_left",
                Some(IpcCommand::MoveWindowToMonitorLeft),
            ),
            (
                "move_to_monitor_right",
                Some(IpcCommand::MoveWindowToMonitorRight),
            ),
            ("resize_grow", Some(IpcCommand::Resize { delta: 50 })),
            ("resize_shrink", Some(IpcCommand::Resize { delta: -50 })),
            ("scroll_left", Some(IpcCommand::Scroll { delta: -100.0 })),
            ("scroll_right", Some(IpcCommand::Scroll { delta: 100.0 })),
            ("refresh", Some(IpcCommand::Refresh)),
            ("reload", Some(IpcCommand::Reload)),
            ("panic_revert", Some(IpcCommand::PanicRevert)),
            ("PANIC-REVERT", Some(IpcCommand::PanicRevert)),
            ("toggle_pause", Some(IpcCommand::TogglePause)),
            ("toggle-pause", Some(IpcCommand::TogglePause)),
            ("close_window", Some(IpcCommand::CloseWindow)),
            ("toggle_floating", Some(IpcCommand::ToggleFloating)),
            ("toggle_fullscreen", Some(IpcCommand::ToggleFullscreen)),
            (
                "width_half",
                Some(IpcCommand::SetColumnWidth { fraction: 0.5 }),
            ),
            ("equalize_widths", Some(IpcCommand::EqualizeColumnWidths)),
            ("unknown_command", None),
            ("", None),
            ("focus left", None),
        ];
        for (input, expected) in cases {
            assert_eq!(parse_command(input), expected, "parse_command({input:?})");
        }
    }

    // =========================================================================
    // Validation: bad values are clamped or reset, each with a warning
    // =========================================================================

    #[test]
    fn test_validate_negative_gaps_clamped() {
        let (config, warnings) = validated(|c| {
            c.layout.gap = -5;
            c.layout.outer_gap = -10;
        });
        assert_eq!(config.layout.gap, 0);
        assert_eq!(config.layout.outer_gap, 0);
        assert!(has_warning(&warnings, "layout.gap"));
        assert!(has_warning(&warnings, "layout.outer_gap"));

        // Zero gaps are valid.
        let (_, warnings) = validated(|c| {
            c.layout.gap = 0;
            c.layout.outer_gap = 0;
        });
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn test_validate_negative_column_widths_clamped() {
        let (config, warnings) = validated(|c| {
            c.layout.min_column_width = -200;
            c.layout.max_column_width = 900;
        });
        assert_eq!(config.layout.min_column_width, 0);
        assert!(has_warning(&warnings, "layout.min_column_width"));

        let (config, warnings) = validated(|c| {
            c.layout.min_column_width = 0;
            c.layout.max_column_width = -900;
            c.layout.default_column_width = 0;
        });
        assert_eq!(config.layout.max_column_width, 0);
        assert!(has_warning(&warnings, "layout.max_column_width"));

        // A negative default is clamped to 0, then pulled into [min, max].
        let (config, warnings) = validated(|c| {
            c.layout.min_column_width = 300;
            c.layout.max_column_width = 1000;
            c.layout.default_column_width = -50;
        });
        assert_eq!(config.layout.default_column_width, 300);
        assert!(has_warning(&warnings, "layout.default_column_width"));
    }

    #[test]
    fn test_validate_min_gt_max_swapped() {
        let (config, warnings) = validated(|c| {
            c.layout.min_column_width = 2000;
            c.layout.max_column_width = 500;
        });
        assert_eq!(config.layout.min_column_width, 500);
        assert_eq!(config.layout.max_column_width, 2000);
        assert!(warnings
            .iter()
            .any(|w| w.field.contains("min_column_width")));
    }

    #[test]
    fn test_validate_default_width_outside_range_clamped() {
        let (config, warnings) = validated(|c| {
            c.layout.min_column_width = 600;
            c.layout.max_column_width = 1000;
            c.layout.default_column_width = 1500;
        });
        assert_eq!(config.layout.default_column_width, 1000);
        assert!(has_warning(&warnings, "layout.default_column_width"));
    }

    #[test]
    fn test_validate_minimum_delays_clamped_when_enabled() {
        let (config, warnings) = validated(|c| {
            c.behavior.focus_follows_mouse = true;
            c.behavior.focus_follows_mouse_delay_ms = 10;
            c.snap_hints.enabled = true;
            c.snap_hints.duration_ms = 20;
        });
        assert_eq!(config.behavior.focus_follows_mouse_delay_ms, 50);
        assert_eq!(config.snap_hints.duration_ms, 50);
        assert!(has_warning(
            &warnings,
            "behavior.focus_follows_mouse_delay_ms"
        ));
        assert!(has_warning(&warnings, "snap_hints.duration_ms"));
    }

    #[test]
    fn test_validate_log_level() {
        let (config, warnings) = validated(|c| c.behavior.log_level = "verbose".to_string());
        assert_eq!(config.behavior.log_level, "info");
        assert!(has_warning(&warnings, "behavior.log_level"));

        let (config, warnings) = validated(|c| c.behavior.log_level = "DEBUG".to_string());
        assert_eq!(config.behavior.log_level, "DEBUG");
        assert!(!has_warning(&warnings, "behavior.log_level"));
    }

    #[test]
    fn test_validate_hex_color() {
        for valid in ["ff0000", "FF0000", "4285f4"] {
            let (config, warnings) =
                validated(|c| c.appearance.active_border_color = valid.to_string());
            assert_eq!(config.appearance.active_border_color, valid);
            assert!(!has_warning(&warnings, "appearance.active_border_color"));
        }
        for invalid in ["ZZZZZZ", "FFF", "#4285F4", ""] {
            let (config, warnings) =
                validated(|c| c.appearance.active_border_color = invalid.to_string());
            assert_eq!(
                config.appearance.active_border_color,
                default_active_border_color(),
                "{invalid:?} should reset to the default"
            );
            assert!(has_warning(&warnings, "appearance.active_border_color"));
        }
    }

    // =========================================================================
    // Compiled window rules
    // =========================================================================

    #[test]
    fn test_compiled_window_rule_matches() {
        let config = Config {
            window_rules: vec![
                rule(Some("Chrome.*"), Some(".*YouTube.*"), None),
                rule(None, None, Some("notepad.exe")),
                rule(None, Some(r"^\[DEBUG\]"), None),
                rule(None, None, None),
            ],
            ..Default::default()
        };
        let compiled = config.compile_window_rules();
        assert_eq!(compiled.len(), 4);

        // All specified criteria must match.
        assert!(compiled[0].matches("Chrome_WidgetWin_1", "YouTube - Chrome", "chrome.exe"));
        assert!(!compiled[0].matches("Firefox", "YouTube - Firefox", "firefox.exe"));
        assert!(!compiled[0].matches("Chrome_WidgetWin_1", "Google Chrome", "chrome.exe"));

        // The executable match is case-insensitive.
        assert!(compiled[1].matches("AnyClass", "Any Title", "NOTEPAD.EXE"));
        assert!(!compiled[1].matches("AnyClass", "Any Title", "wordpad.exe"));

        // Titles are regexes, case-sensitive by default.
        assert!(compiled[2].matches("AnyClass", "[DEBUG] started", "app.exe"));
        assert!(!compiled[2].matches("AnyClass", "DEBUG started", "app.exe"));

        // A rule without criteria matches nothing.
        assert!(!compiled[3].matches("AnyClass", "Any Title", "any.exe"));
    }

    #[test]
    fn test_compiled_window_rule_invalid_regex_skipped() {
        let config = Config {
            window_rules: vec![
                rule(Some("[invalid(regex"), None, None),
                rule(None, Some("(unclosed"), None),
                rule(Some("ValidClass"), None, None),
            ],
            ..Default::default()
        };
        let compiled = config.compile_window_rules();
        assert_eq!(compiled.len(), 1);
        assert!(compiled[0].matches("ValidClass", "Any Title", "any.exe"));
    }

    #[test]
    fn test_compiled_window_rule_oversized_regex_skipped() {
        // Valid under the regex crate's default 10 MB limit, but over the
        // 1 MB limit that compile_window_rules enforces.
        let oversized = r"\w{100}";
        assert!(
            regex::Regex::new(oversized).is_ok(),
            "pattern must compile under the default limit"
        );
        let config = Config {
            window_rules: vec![
                rule(None, Some(oversized), None),
                rule(Some("ValidClass"), None, None),
            ],
            ..Default::default()
        };
        let compiled = config.compile_window_rules();
        assert_eq!(
            compiled.len(),
            1,
            "a rule over the 1 MB compile limit is skipped"
        );
        assert!(compiled[0].matches("ValidClass", "", ""));
    }
}
