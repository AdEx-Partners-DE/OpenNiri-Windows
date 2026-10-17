//! OpenNiri Core Layout Engine
//!
//! Platform-agnostic scrollable tiling layout engine inspired by Niri.
//!
//! This crate implements the "infinite horizontal strip" paradigm where:
//! - Windows are arranged in columns on an infinite horizontal strip
//! - The monitor acts as a viewport/camera sliding over this strip
//! - New windows append without resizing existing ones

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// Minimum width for columns in pixels.
const MIN_COLUMN_WIDTH: i32 = 100;

/// Default gap between columns in pixels.
pub const DEFAULT_GAP: i32 = 10;
/// Default gap at viewport edges in pixels.
pub const DEFAULT_OUTER_GAP: i32 = 10;
/// Default width for new columns in pixels.
pub const DEFAULT_COLUMN_WIDTH: i32 = 800;

/// Unique identifier for a window.
/// On Windows, this will typically be the HWND cast to u64.
pub type WindowId = u64;

/// Errors that can occur during layout operations.
#[derive(Debug, Error)]
pub enum LayoutError {
    #[error("Column index {0} is out of bounds (max: {1})")]
    ColumnOutOfBounds(usize, usize),

    #[error("Window {0} not found in workspace")]
    WindowNotFound(WindowId),

    #[error("Window {0} already exists in workspace")]
    DuplicateWindow(WindowId),

    #[error("Window index {0} is out of bounds in column {1} (max: {2})")]
    WindowIndexOutOfBounds(usize, usize, usize),
}

/// A rectangle in screen coordinates (pixels).
///
/// Note: Fields are intentionally public for convenient read access.
/// Use `Rect::new()` to construct with dimension validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    /// Create a new rectangle.
    /// Width and height are clamped to >= 0 to prevent invalid dimensions.
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width: width.max(0),
            height: height.max(0),
        }
    }

    /// Check if this rectangle intersects with another.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    /// Get the right edge x-coordinate.
    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    /// Get the bottom edge y-coordinate.
    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }
}

/// Visibility state for layout computation.
/// Determines whether a window should be rendered or cloaked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    /// Window is within the viewport and should be rendered.
    Visible,
    /// Window is off-screen to the left of the viewport.
    OffScreenLeft,
    /// Window is off-screen to the right of the viewport.
    OffScreenRight,
}

// ============================================================================
// Animation Support
// ============================================================================

/// Easing function types for animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Easing {
    /// Linear interpolation (constant speed).
    Linear,
    /// Smooth deceleration (starts fast, ends slow).
    #[default]
    EaseOut,
    /// Smooth acceleration (starts slow, ends fast).
    EaseIn,
    /// Smooth acceleration and deceleration.
    EaseInOut,
}

impl Easing {
    /// Apply the easing function to a progress value (0.0 to 1.0).
    /// Returns the eased progress value (0.0 to 1.0).
    pub fn apply(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseOut => 1.0 - (1.0 - t).powi(3), // Cubic ease out
            Easing::EaseIn => t.powi(3),                // Cubic ease in
            Easing::EaseInOut => {
                // Cubic ease in-out
                if t < 0.5 {
                    4.0 * t.powi(3)
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
        }
    }
}

/// Duration of scroll animations in milliseconds.
pub const DEFAULT_ANIMATION_DURATION_MS: u64 = 200;

/// Animation state for smooth scrolling.
#[derive(Debug, Clone)]
pub struct ScrollAnimation {
    /// Starting scroll offset.
    pub start_offset: f64,
    /// Target scroll offset.
    pub target_offset: f64,
    /// Animation duration in milliseconds.
    pub duration_ms: u64,
    /// Elapsed time in milliseconds.
    pub elapsed_ms: u64,
    /// Easing function to use.
    pub easing: Easing,
}

impl ScrollAnimation {
    /// Create a new scroll animation.
    pub fn new(start: f64, target: f64, duration_ms: u64, easing: Easing) -> Self {
        Self {
            start_offset: start,
            target_offset: target,
            duration_ms,
            elapsed_ms: 0,
            easing,
        }
    }

    /// Create a new animation with default duration and easing.
    pub fn with_defaults(start: f64, target: f64) -> Self {
        Self::new(
            start,
            target,
            DEFAULT_ANIMATION_DURATION_MS,
            Easing::default(),
        )
    }

    /// Check if the animation is complete.
    pub fn is_complete(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }

    /// Get the current progress (0.0 to 1.0).
    pub fn progress(&self) -> f64 {
        if self.duration_ms == 0 {
            return 1.0;
        }
        (self.elapsed_ms as f64 / self.duration_ms as f64).clamp(0.0, 1.0)
    }

    /// Get the current scroll offset based on animation progress.
    pub fn current_offset(&self) -> f64 {
        let eased_progress = self.easing.apply(self.progress());
        self.start_offset + (self.target_offset - self.start_offset) * eased_progress
    }

    /// Advance the animation by the given delta time in milliseconds.
    /// Returns true if the animation is still running, false if complete.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        !self.is_complete()
    }

    /// Get the final target offset.
    pub fn target(&self) -> f64 {
        self.target_offset
    }
}

/// Computed placement for a window.
/// Contains the target rectangle and visibility state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowPlacement {
    /// The window identifier.
    pub window_id: WindowId,
    /// The target rectangle in screen coordinates.
    pub rect: Rect,
    /// Whether the window is visible or off-screen.
    pub visibility: Visibility,
    /// The column index this window belongs to.
    pub column_index: usize,
}

/// A column in the infinite strip.
/// A column contains one or more vertically stacked windows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Column {
    /// Width of the column in pixels.
    width: i32,
    /// Windows in this column (vertically stacked).
    windows: Vec<WindowId>,
}

impl Column {
    /// Create a new column with a single window.
    /// Width is clamped to MIN_COLUMN_WIDTH (100px) minimum.
    pub fn new(window_id: WindowId, width: i32) -> Self {
        Self {
            width: width.max(MIN_COLUMN_WIDTH),
            windows: vec![window_id],
        }
    }

    /// Create an empty column with specified width.
    /// Width is clamped to MIN_COLUMN_WIDTH (100px) minimum.
    pub fn empty(width: i32) -> Self {
        Self {
            width: width.max(MIN_COLUMN_WIDTH),
            windows: Vec::new(),
        }
    }

    /// Check if the column is empty.
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Get the number of windows in this column.
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    /// Add a window to this column (at the bottom of the stack).
    pub fn add_window(&mut self, window_id: WindowId) {
        self.windows.push(window_id);
    }

    /// Remove a window from this column.
    /// Returns the index of the removed window if found, None otherwise.
    pub fn remove_window(&mut self, window_id: WindowId) -> Option<usize> {
        if let Some(pos) = self.windows.iter().position(|&w| w == window_id) {
            self.windows.remove(pos);
            Some(pos)
        } else {
            None
        }
    }

    /// Get the width of this column.
    pub fn width(&self) -> i32 {
        self.width
    }

    /// Set the width of this column.
    /// Width is clamped to MIN_COLUMN_WIDTH (100px) minimum.
    pub fn set_width(&mut self, width: i32) {
        self.width = width.max(MIN_COLUMN_WIDTH);
    }

    /// Get a slice of windows in this column.
    pub fn windows(&self) -> &[WindowId] {
        &self.windows
    }

    /// Check if this column contains a specific window.
    pub fn contains(&self, window_id: WindowId) -> bool {
        self.windows.contains(&window_id)
    }

    /// Get a window by index.
    pub fn get(&self, index: usize) -> Option<WindowId> {
        self.windows.get(index).copied()
    }
}

/// Focus centering mode.
/// Determines how the viewport adjusts when focus changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CenteringMode {
    /// Center the focused column in the viewport.
    #[default]
    Center,
    /// Only scroll if the focused column would be outside the viewport.
    JustInView,
}

/// A floating window that is not part of the tiling layout.
///
/// Floating windows are positioned at absolute coordinates and always
/// remain visible (not scrolled with the workspace).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloatingWindow {
    /// The window identifier.
    pub id: WindowId,
    /// The position and size of the floating window.
    pub rect: Rect,
}

/// The scrollable workspace.
/// This is the core data structure representing the infinite horizontal strip.
///
/// # Invariants
///
/// The following invariants are maintained by all methods:
///
/// 1. **No duplicate windows:** Each `WindowId` appears at most once.
/// 2. **Valid focus:** If `columns` is empty, `focused_window()` returns `None`.
///    Otherwise, `focused_column < columns.len()` and
///    `focused_window_in_column < columns[focused_column].len()`.
/// 3. **Valid column widths:** All column widths are >= `MIN_COLUMN_WIDTH` (100px).
/// 4. **Valid scroll range:** `0.0 <= scroll_offset <= max_scroll` where
///    `max_scroll = (total_width() - viewport_width).max(0)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Columns in the workspace, ordered left to right.
    columns: Vec<Column>,
    /// Index of the currently focused column.
    focused_column: usize,
    /// Index of the focused window within the focused column.
    focused_window_in_column: usize,
    /// Current scroll offset (x position of viewport's left edge on the strip).
    scroll_offset: f64,
    /// Gap between columns in pixels (always >= 0).
    gap: i32,
    /// Gap at the edges of the viewport (always >= 0).
    outer_gap: i32,
    /// Default width for new columns (always >= MIN_COLUMN_WIDTH).
    default_column_width: i32,
    /// Centering mode for focus changes.
    centering_mode: CenteringMode,
    /// Active scroll animation, if any.
    #[serde(skip)]
    active_animation: Option<ScrollAnimation>,
    /// Floating windows outside the tiling layout.
    #[serde(default)]
    floating_windows: Vec<FloatingWindow>,
    /// Window ID in fullscreen mode, if any.
    #[serde(default)]
    fullscreen_window: Option<WindowId>,
    /// Windows that are currently minimized (excluded from layout).
    #[serde(default)]
    minimized_windows: HashSet<WindowId>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            focused_column: 0,
            focused_window_in_column: 0,
            scroll_offset: 0.0,
            gap: DEFAULT_GAP,
            outer_gap: DEFAULT_OUTER_GAP,
            default_column_width: DEFAULT_COLUMN_WIDTH,
            centering_mode: CenteringMode::default(),
            active_animation: None,
            floating_windows: Vec::new(),
            fullscreen_window: None,
            minimized_windows: HashSet::new(),
        }
    }
}

impl Workspace {
    /// Create a new empty workspace with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a workspace with custom gap settings.
    /// Gap values are clamped to >= 0.
    pub fn with_gaps(gap: i32, outer_gap: i32) -> Self {
        Self {
            gap: gap.max(0),
            outer_gap: outer_gap.max(0),
            ..Default::default()
        }
    }

    /// Check if the workspace is empty.
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// Get the number of columns.
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Check if a window ID already exists in the workspace (tiled or floating).
    pub fn contains_window(&self, window_id: WindowId) -> bool {
        self.columns.iter().any(|c| c.windows.contains(&window_id))
            || self.floating_windows.iter().any(|f| f.id == window_id)
    }

    /// Check if a window is floating.
    pub fn is_floating(&self, window_id: WindowId) -> bool {
        self.floating_windows.iter().any(|f| f.id == window_id)
    }

    /// Get the number of floating windows.
    pub fn floating_count(&self) -> usize {
        self.floating_windows.len()
    }

    /// Add a floating window to the workspace.
    ///
    /// # Errors
    ///
    /// Returns `LayoutError::DuplicateWindow` if the window ID already exists.
    pub fn add_floating(&mut self, window_id: WindowId, rect: Rect) -> Result<(), LayoutError> {
        if self.contains_window(window_id) {
            return Err(LayoutError::DuplicateWindow(window_id));
        }

        self.floating_windows.push(FloatingWindow {
            id: window_id,
            rect,
        });
        Ok(())
    }

    /// Remove a floating window from the workspace.
    ///
    /// Returns true if the window was found and removed, false otherwise.
    pub fn remove_floating(&mut self, window_id: WindowId) -> bool {
        if let Some(pos) = self.floating_windows.iter().position(|f| f.id == window_id) {
            self.floating_windows.remove(pos);
            if self.fullscreen_window == Some(window_id) {
                self.fullscreen_window = None;
            }
            true
        } else {
            false
        }
    }

    /// Update the position/size of a floating window.
    pub fn update_floating(&mut self, window_id: WindowId, rect: Rect) -> bool {
        if let Some(floating) = self.floating_windows.iter_mut().find(|f| f.id == window_id) {
            floating.rect = rect;
            true
        } else {
            false
        }
    }

    /// Get all floating windows.
    pub fn floating_windows(&self) -> &[FloatingWindow] {
        &self.floating_windows
    }

    /// Get the total width of the strip (sum of all column widths + gaps).
    ///
    /// Note: Negative gaps are treated as zero for calculation purposes.
    pub fn total_width(&self) -> i32 {
        if self.columns.is_empty() {
            return 0;
        }

        // Defensively clamp gaps to >= 0 in case fields were set directly
        let gap = self.gap.max(0);
        let outer_gap = self.outer_gap.max(0);

        let column_widths: i32 = self
            .columns
            .iter()
            .map(|c| c.width)
            .fold(0i32, |acc, w| acc.saturating_add(w));
        let gaps = gap.saturating_mul(self.columns.len().saturating_sub(1) as i32);
        let outer_gaps = outer_gap.saturating_mul(2);

        column_widths
            .saturating_add(gaps)
            .saturating_add(outer_gaps)
    }

    /// Insert a new window as a new column to the right of the focused column.
    /// Column width is clamped to MIN_COLUMN_WIDTH (100px) minimum.
    ///
    /// # Errors
    ///
    /// Returns `LayoutError::DuplicateWindow` if the window ID already exists.
    pub fn insert_window(
        &mut self,
        window_id: WindowId,
        width: Option<i32>,
    ) -> Result<(), LayoutError> {
        if self.contains_window(window_id) {
            return Err(LayoutError::DuplicateWindow(window_id));
        }

        let column_width = width
            .unwrap_or(self.default_column_width)
            .max(MIN_COLUMN_WIDTH);
        let new_column = Column::new(window_id, column_width);

        if self.columns.is_empty() {
            self.columns.push(new_column);
            self.focused_column = 0;
        } else {
            // Insert to the right of the focused column
            let insert_pos = self.focused_column + 1;
            self.columns.insert(insert_pos, new_column);
            self.focused_column = insert_pos;
        }
        self.focused_window_in_column = 0;

        debug_assert!(
            self.focused_column < self.columns.len(),
            "Invariant violation: focused_column out of bounds after insert"
        );

        Ok(())
    }

    /// Insert a window without changing the current focus.
    ///
    /// Same as `insert_window`, but preserves `focused_column` and
    /// `focused_window_in_column` so that the user's keyboard position
    /// is not stolen by the new window. Used when `focus_new_windows=false`.
    ///
    /// # Errors
    ///
    /// Returns `LayoutError::DuplicateWindow` if the window ID already exists.
    pub fn insert_window_no_focus(
        &mut self,
        window_id: WindowId,
        width: Option<i32>,
    ) -> Result<(), LayoutError> {
        let saved_col = self.focused_column;
        let saved_win = self.focused_window_in_column;

        self.insert_window(window_id, width)?;

        // insert_window inserts at saved_col + 1 (or 0 if was empty).
        // If the workspace was empty, there's nothing to restore.
        if saved_col < self.columns.len() && self.columns.len() > 1 {
            // The new column was inserted at saved_col + 1, which shifted
            // nothing before it, so saved_col is still valid.
            self.focused_column = saved_col;
            self.focused_window_in_column = saved_win;
        }

        Ok(())
    }

    /// Insert a window into an existing column (stacking).
    ///
    /// # Errors
    ///
    /// Returns `LayoutError::ColumnOutOfBounds` if the column index is invalid.
    /// Returns `LayoutError::DuplicateWindow` if the window ID already exists.
    pub fn insert_window_in_column(
        &mut self,
        window_id: WindowId,
        column_index: usize,
    ) -> Result<(), LayoutError> {
        if self.contains_window(window_id) {
            return Err(LayoutError::DuplicateWindow(window_id));
        }

        if column_index >= self.columns.len() {
            return Err(LayoutError::ColumnOutOfBounds(
                column_index,
                self.columns.len().saturating_sub(1),
            ));
        }

        self.columns[column_index].add_window(window_id);
        Ok(())
    }

    /// Remove a window from the workspace.
    /// If removing the last window from a column, the column is removed.
    /// If removing the last column, the workspace becomes empty.
    ///
    /// # Focus Policy
    ///
    /// When removing a window from a stacked column:
    /// - If removed window was before the focused window, focus index decrements to stay on same window
    /// - If removed window was the focused window, focus moves to next window (or previous if at end)
    /// - If removed window was after the focused window, focus index stays the same
    pub fn remove_window(&mut self, window_id: WindowId) -> Result<(), LayoutError> {
        for (col_idx, column) in self.columns.iter_mut().enumerate() {
            if let Some(removed_idx) = column.remove_window(window_id) {
                // Also clear from minimized set
                self.minimized_windows.remove(&window_id);
                if self.fullscreen_window == Some(window_id) {
                    self.fullscreen_window = None;
                }

                // If column is now empty, remove it
                if column.is_empty() {
                    self.columns.remove(col_idx);
                    if self.columns.is_empty() {
                        // Workspace is now empty - reset all state
                        self.focused_column = 0;
                        self.focused_window_in_column = 0;
                        self.scroll_offset = 0.0;
                    } else if self.focused_column >= self.columns.len() {
                        self.focused_column = self.columns.len() - 1;
                    } else if self.focused_column > col_idx {
                        self.focused_column -= 1;
                    }
                } else {
                    // Adjust focused window in column if this is the focused column
                    if col_idx == self.focused_column {
                        let col_len = self.columns[self.focused_column].len();
                        if removed_idx < self.focused_window_in_column {
                            // Removed window was before focused - decrement to stay on same window
                            self.focused_window_in_column -= 1;
                        } else if removed_idx == self.focused_window_in_column {
                            // Removed the focused window - move to next (or previous if at end)
                            if self.focused_window_in_column >= col_len {
                                self.focused_window_in_column = col_len.saturating_sub(1);
                            }
                            // If focus index is still valid, it now points to the "next" window
                            // (which slid into this position), which is the expected behavior
                        }
                        // If removed_idx > focused_window_in_column, no adjustment needed
                    }
                }

                self.clamp_focus_indices();

                debug_assert!(
                    self.columns.is_empty() || self.focused_column < self.columns.len(),
                    "Invariant violation: focused_column out of bounds after remove"
                );
                debug_assert!(
                    self.columns.is_empty()
                        || self.focused_window_in_column < self.columns[self.focused_column].len(),
                    "Invariant violation: focused_window_in_column out of bounds after remove"
                );

                return Ok(());
            }
        }
        Err(LayoutError::WindowNotFound(window_id))
    }

    /// Move focus to the column on the left, skipping columns where all
    /// windows are minimized. Within the target column, adjusts the
    /// focused window index to a non-minimized window.
    pub fn focus_left(&mut self) {
        let start = self.focused_column;
        while self.focused_column > 0 {
            self.focused_column -= 1;
            // Clamp focused window in column
            let col_len = self.columns[self.focused_column].len();
            if self.focused_window_in_column >= col_len {
                self.focused_window_in_column = col_len.saturating_sub(1);
            }
            // Skip columns where every window is minimized
            if self.has_visible_window_in_column(self.focused_column) {
                self.adjust_focus_to_visible_in_column();
                break;
            }
        }
        // If we didn't find a visible column, stay at original
        if !self.has_visible_window_in_column(self.focused_column) {
            self.focused_column = start;
            if let Some(col) = self.columns.get(self.focused_column) {
                if self.focused_window_in_column >= col.len() {
                    self.focused_window_in_column = col.len().saturating_sub(1);
                }
            }
        }

        self.clamp_focus_indices();
        if self.has_visible_window_in_column(self.focused_column) {
            self.adjust_focus_to_visible_in_column();
        }

        debug_assert!(
            self.columns.is_empty()
                || (self.focused_column < self.columns.len()
                    && self.focused_window_in_column < self.columns[self.focused_column].len()),
            "Invariant violation: focus indices out of bounds after focus_left"
        );
    }

    /// Move focus to the column on the right, skipping columns where all
    /// windows are minimized. Within the target column, adjusts the
    /// focused window index to a non-minimized window.
    pub fn focus_right(&mut self) {
        let start = self.focused_column;
        while self.focused_column + 1 < self.columns.len() {
            self.focused_column += 1;
            // Clamp focused window in column
            let col_len = self.columns[self.focused_column].len();
            if self.focused_window_in_column >= col_len {
                self.focused_window_in_column = col_len.saturating_sub(1);
            }
            // Skip columns where every window is minimized
            if self.has_visible_window_in_column(self.focused_column) {
                self.adjust_focus_to_visible_in_column();
                break;
            }
        }
        // If we didn't find a visible column, stay at original
        if !self.has_visible_window_in_column(self.focused_column) {
            self.focused_column = start;
            if let Some(col) = self.columns.get(self.focused_column) {
                if self.focused_window_in_column >= col.len() {
                    self.focused_window_in_column = col.len().saturating_sub(1);
                }
            }
        }

        self.clamp_focus_indices();
        if self.has_visible_window_in_column(self.focused_column) {
            self.adjust_focus_to_visible_in_column();
        }

        debug_assert!(
            self.columns.is_empty()
                || (self.focused_column < self.columns.len()
                    && self.focused_window_in_column < self.columns[self.focused_column].len()),
            "Invariant violation: focus indices out of bounds after focus_right"
        );
    }

    /// Move focus to the window above in the current column, skipping
    /// minimized windows.
    pub fn focus_up(&mut self) {
        if let Some(column) = self.columns.get(self.focused_column) {
            let mut target = self.focused_window_in_column;
            while target > 0 {
                target -= 1;
                if !self.minimized_windows.contains(&column.windows[target]) {
                    self.focused_window_in_column = target;
                    return;
                }
            }
        }
    }

    /// Move focus to the window below in the current column, skipping
    /// minimized windows.
    pub fn focus_down(&mut self) {
        if let Some(column) = self.columns.get(self.focused_column) {
            let mut target = self.focused_window_in_column;
            while target + 1 < column.len() {
                target += 1;
                if !self.minimized_windows.contains(&column.windows[target]) {
                    self.focused_window_in_column = target;
                    return;
                }
            }
        }
    }

    /// Clamp focus indices to valid column/window bounds.
    fn clamp_focus_indices(&mut self) {
        if self.columns.is_empty() {
            self.focused_column = 0;
            self.focused_window_in_column = 0;
            return;
        }

        if self.focused_column >= self.columns.len() {
            self.focused_column = self.columns.len() - 1;
        }

        let col_len = self.columns[self.focused_column].len();
        if col_len == 0 {
            self.focused_window_in_column = 0;
            return;
        }

        if self.focused_window_in_column >= col_len {
            self.focused_window_in_column = col_len - 1;
        }
    }

    /// Check if a column has at least one non-minimized window.
    fn has_visible_window_in_column(&self, col_idx: usize) -> bool {
        self.columns.get(col_idx).is_some_and(|col| {
            col.windows
                .iter()
                .any(|w| !self.minimized_windows.contains(w))
        })
    }

    /// If the current `focused_window_in_column` points to a minimized window,
    /// shift it to the nearest non-minimized window in the same column.
    /// Searches downward first, then upward.
    fn adjust_focus_to_visible_in_column(&mut self) {
        let col = match self.columns.get(self.focused_column) {
            Some(c) => c,
            None => return,
        };
        let cur = self.focused_window_in_column;
        // Already pointing at a visible window — nothing to do
        if cur < col.len() && !self.minimized_windows.contains(&col.windows[cur]) {
            return;
        }
        // Search downward from current index
        for i in cur..col.len() {
            if !self.minimized_windows.contains(&col.windows[i]) {
                self.focused_window_in_column = i;
                return;
            }
        }
        // Search upward from current index
        for i in (0..cur).rev() {
            if !self.minimized_windows.contains(&col.windows[i]) {
                self.focused_window_in_column = i;
                return;
            }
        }
        // All minimized — leave index as is (has_visible_window_in_column
        // should have prevented us from landing here).
    }

    /// Get the currently focused window ID.
    pub fn focused_window(&self) -> Option<WindowId> {
        self.columns
            .get(self.focused_column)
            .and_then(|col| col.windows.get(self.focused_window_in_column))
            .copied()
    }

    /// Get the focused window ID, but only if it is not minimized.
    /// Falls back to the nearest non-minimized window in the focused column.
    /// Returns `None` if the workspace is empty or every window is minimized.
    pub fn focused_visible_window(&self) -> Option<WindowId> {
        let col = self.columns.get(self.focused_column)?;
        let cur = self.focused_window_in_column;

        // Check the exact focused index first
        if let Some(&wid) = col.windows.get(cur) {
            if !self.minimized_windows.contains(&wid) {
                return Some(wid);
            }
        }

        // Search downward then upward for a visible window
        for i in cur..col.len() {
            if !self.minimized_windows.contains(&col.windows[i]) {
                return Some(col.windows[i]);
            }
        }
        for i in (0..cur).rev() {
            if !self.minimized_windows.contains(&col.windows[i]) {
                return Some(col.windows[i]);
            }
        }
        None
    }

    /// Get the index of the currently focused column.
    pub fn focused_column_index(&self) -> usize {
        self.focused_column
    }

    /// Get the index of the focused window within the focused column.
    pub fn focused_window_index_in_column(&self) -> usize {
        self.focused_window_in_column
    }

    /// Get the current scroll offset.
    pub fn scroll_offset(&self) -> f64 {
        self.scroll_offset
    }

    /// Get a slice of all columns.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Get a column by index (safe access).
    pub fn column(&self, index: usize) -> Option<&Column> {
        self.columns.get(index)
    }

    /// Find a window's location in the workspace.
    /// Returns (column_index, window_index_in_column) if found.
    pub fn find_window_location(&self, window_id: WindowId) -> Option<(usize, usize)> {
        for (col_idx, column) in self.columns.iter().enumerate() {
            if let Some(win_idx) = column.windows.iter().position(|&w| w == window_id) {
                return Some((col_idx, win_idx));
            }
        }
        None
    }

    /// Get total window count across all columns.
    pub fn window_count(&self) -> usize {
        self.columns.iter().map(|c| c.len()).sum()
    }

    /// Get all window IDs in this workspace (both tiled and floating).
    ///
    /// Useful for migrating windows when monitors are disconnected.
    pub fn all_window_ids(&self) -> Vec<WindowId> {
        let mut ids: Vec<WindowId> = self
            .columns
            .iter()
            .flat_map(|c| c.windows().iter().copied())
            .collect();
        ids.extend(self.floating_windows.iter().map(|f| f.id));
        ids
    }

    /// Get the gap between columns in pixels.
    pub fn gap(&self) -> i32 {
        self.gap
    }

    /// Set the gap between columns in pixels.
    /// Value is clamped to >= 0.
    pub fn set_gap(&mut self, gap: i32) {
        self.gap = gap.max(0);
    }

    /// Get the gap at viewport edges in pixels.
    pub fn outer_gap(&self) -> i32 {
        self.outer_gap
    }

    /// Set the gap at viewport edges in pixels.
    /// Value is clamped to >= 0.
    pub fn set_outer_gap(&mut self, outer_gap: i32) {
        self.outer_gap = outer_gap.max(0);
    }

    /// Get the default width for new columns.
    pub fn default_column_width(&self) -> i32 {
        self.default_column_width
    }

    /// Set the default width for new columns.
    /// Value is clamped to >= MIN_COLUMN_WIDTH (100px).
    pub fn set_default_column_width(&mut self, width: i32) {
        self.default_column_width = width.max(MIN_COLUMN_WIDTH);
    }

    /// Get the centering mode for focus changes.
    pub fn centering_mode(&self) -> CenteringMode {
        self.centering_mode
    }

    /// Set the centering mode for focus changes.
    pub fn set_centering_mode(&mut self, mode: CenteringMode) {
        self.centering_mode = mode;
    }

    /// Set focus to a specific column and window index with validation.
    ///
    /// # Errors
    ///
    /// Returns `LayoutError::ColumnOutOfBounds` if the column index is invalid.
    /// Returns `LayoutError::WindowIndexOutOfBounds` if the window index is invalid.
    pub fn set_focus(&mut self, column: usize, window_in_column: usize) -> Result<(), LayoutError> {
        if column >= self.columns.len() {
            return Err(LayoutError::ColumnOutOfBounds(
                column,
                self.columns.len().saturating_sub(1),
            ));
        }

        let col_len = self.columns[column].len();
        if window_in_column >= col_len {
            return Err(LayoutError::WindowIndexOutOfBounds(
                window_in_column,
                column,
                col_len.saturating_sub(1),
            ));
        }

        self.focused_column = column;
        self.focused_window_in_column = window_in_column;
        Ok(())
    }

    /// Focus a window by its ID.
    ///
    /// # Errors
    ///
    /// Returns `LayoutError::WindowNotFound` if the window is not in the workspace.
    pub fn focus_window(&mut self, window_id: WindowId) -> Result<(), LayoutError> {
        for (col_idx, column) in self.columns.iter().enumerate() {
            if let Some(win_idx) = column.windows.iter().position(|&w| w == window_id) {
                self.focused_column = col_idx;
                self.focused_window_in_column = win_idx;
                return Ok(());
            }
        }
        Err(LayoutError::WindowNotFound(window_id))
    }

    /// Calculate the x-coordinate of a column's left edge on the strip.
    ///
    /// Note: Negative gaps are treated as zero for calculation purposes.
    fn column_x(&self, column_index: usize) -> i32 {
        // Defensively clamp gaps to >= 0
        let gap = self.gap.max(0);
        let outer_gap = self.outer_gap.max(0);

        let mut x = outer_gap;
        for (i, col) in self.columns.iter().enumerate() {
            if i == column_index {
                return x;
            }
            x = x.saturating_add(col.width).saturating_add(gap);
        }
        x
    }

    /// Get the x-coordinate and width of the focused column.
    fn focused_column_bounds(&self) -> Option<(i32, i32)> {
        self.columns.get(self.focused_column).map(|col| {
            let x = self.column_x(self.focused_column);
            (x, col.width)
        })
    }

    /// Ensure the focused column is visible in the viewport.
    /// Adjusts scroll_offset according to the centering mode.
    ///
    /// Note: Negative gaps are treated as zero for calculation purposes.
    pub fn ensure_focused_visible(&mut self, viewport_width: i32) {
        if self.columns.is_empty() {
            return;
        }

        let Some((col_x, col_width)) = self.focused_column_bounds() else {
            return;
        };

        // Defensively clamp outer_gap to >= 0
        let outer_gap = self.outer_gap.max(0);

        match self.centering_mode {
            CenteringMode::Center => {
                // Center the focused column in the viewport
                let col_center = col_x.saturating_add(col_width / 2);
                self.scroll_offset = (col_center.saturating_sub(viewport_width / 2)) as f64;
            }
            CenteringMode::JustInView => {
                // Only scroll if the focused column is outside the viewport
                // Use rounding instead of truncation for consistent behavior
                let viewport_left = self.scroll_offset.round() as i32;
                let viewport_right = viewport_left.saturating_add(viewport_width);
                let col_right = col_x.saturating_add(col_width);

                if col_x < viewport_left {
                    // Column is to the left of viewport, scroll left
                    self.scroll_offset = col_x.saturating_sub(outer_gap) as f64;
                } else if col_right > viewport_right {
                    // Column is to the right of viewport, scroll right
                    self.scroll_offset = col_right
                        .saturating_add(outer_gap)
                        .saturating_sub(viewport_width)
                        as f64;
                }
            }
        }

        // Clamp scroll offset to valid range
        let max_scroll = (self.total_width() - viewport_width).max(0);
        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll as f64);
    }

    /// Compute placements for all windows given a viewport.
    ///
    /// Returns a list of WindowPlacement structs indicating where each window
    /// should be positioned and whether it's visible or off-screen.
    ///
    /// Note: Negative gaps are treated as zero for calculation purposes.
    pub fn compute_placements(&self, viewport: Rect) -> Vec<WindowPlacement> {
        // Use rounding instead of truncation to prevent sub-pixel jitter
        let viewport_left = self.scroll_offset.round() as i32;

        // Fullscreen mode: one window covers the entire viewport, others are off-screen
        if let Some(fs_wid) = self.fullscreen_window {
            return self.compute_fullscreen_placements(fs_wid, viewport, viewport_left);
        }

        self.compute_non_fullscreen_placements(viewport, viewport_left)
    }

    /// Compute non-fullscreen placements for a specific viewport-left offset.
    /// Used by both static and animated placement paths.
    fn compute_non_fullscreen_placements(
        &self,
        viewport: Rect,
        viewport_left: i32,
    ) -> Vec<WindowPlacement> {
        let mut placements = Vec::new();

        // Defensively clamp gaps to >= 0 in case fields were set directly
        let gap = self.gap.max(0);
        let outer_gap = self.outer_gap.max(0);

        let viewport_right = viewport_left.saturating_add(viewport.width);

        let mut current_x = outer_gap;

        for (col_idx, column) in self.columns.iter().enumerate() {
            // Calculate column position in strip coordinates
            let col_strip_x = current_x;
            let col_strip_right = col_strip_x.saturating_add(column.width);

            // Transform to screen coordinates (relative to viewport)
            let col_screen_x = col_strip_x
                .saturating_sub(viewport_left)
                .saturating_add(viewport.x);

            // Determine visibility
            let visibility = if col_strip_right <= viewport_left {
                Visibility::OffScreenLeft
            } else if col_strip_x >= viewport_right {
                Visibility::OffScreenRight
            } else {
                Visibility::Visible
            };

            // Filter out minimized windows for height calculation
            let visible_windows: Vec<WindowId> = column
                .windows()
                .iter()
                .copied()
                .filter(|w| !self.minimized_windows.contains(w))
                .collect();

            // Calculate window heights (equal split for stacked windows)
            // Clamp usable_height to >= 0 to handle tight viewports
            // Use saturating arithmetic to prevent overflow
            let usable_height = viewport
                .height
                .saturating_sub(outer_gap.saturating_mul(2))
                .max(0);
            let window_count = visible_windows.len() as i32;
            let window_gaps = if window_count > 1 {
                gap.saturating_mul(window_count - 1)
            } else {
                0
            };
            // Clamp window_height to >= 0 to prevent negative dimensions
            let window_height = if window_count > 0 {
                ((usable_height - window_gaps).max(0)) / window_count
            } else {
                0
            };

            let mut current_y = viewport.y + outer_gap;

            for (win_idx, &window_id) in visible_windows.iter().enumerate() {
                // Adjust height for last visible window to handle rounding
                // Clamp to >= 0 to prevent negative dimensions
                let height = if win_idx == visible_windows.len() - 1 {
                    (viewport.y + viewport.height - outer_gap - current_y).max(0)
                } else {
                    window_height
                };

                placements.push(WindowPlacement {
                    window_id,
                    rect: Rect::new(col_screen_x, current_y, column.width, height),
                    visibility,
                    column_index: col_idx,
                });

                current_y = current_y.saturating_add(height).saturating_add(gap);
            }

            current_x = current_x.saturating_add(column.width).saturating_add(gap);
        }

        // Add floating windows (always visible, at their absolute positions)
        for floating in &self.floating_windows {
            placements.push(WindowPlacement {
                window_id: floating.id,
                rect: floating.rect,
                visibility: Visibility::Visible,
                column_index: usize::MAX, // Sentinel for floating windows
            });
        }

        placements
    }

    /// Resize the focused column by a delta amount.
    pub fn resize_focused_column(&mut self, delta: i32) {
        if let Some(column) = self.columns.get_mut(self.focused_column) {
            let new_width = column.width.saturating_add(delta).max(MIN_COLUMN_WIDTH);
            column.width = new_width;
        }
    }

    /// Move the focused column left (swap with the column to its left).
    pub fn move_column_left(&mut self) {
        if self.focused_column > 0 {
            self.columns
                .swap(self.focused_column, self.focused_column - 1);
            self.focused_column -= 1;
        }
    }

    /// Move the focused column right (swap with the column to its right).
    pub fn move_column_right(&mut self) {
        if self.focused_column + 1 < self.columns.len() {
            self.columns
                .swap(self.focused_column, self.focused_column + 1);
            self.focused_column += 1;
        }
    }

    /// Scroll the viewport by a pixel delta.
    ///
    /// Special float values (NaN, Infinity) are treated as zero for safety.
    pub fn scroll_by(&mut self, delta: f64, viewport_width: i32) {
        // Treat NaN and Infinity as zero for safety
        let safe_delta = if delta.is_finite() { delta } else { 0.0 };
        self.scroll_offset += safe_delta;
        let max_scroll = (self.total_width() - viewport_width).max(0);
        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll as f64);
    }

    // ========================================================================
    // Animation Methods
    // ========================================================================

    /// Check if a scroll animation is currently active.
    pub fn is_animating(&self) -> bool {
        self.active_animation.is_some()
    }

    /// Get the current effective scroll offset.
    /// Returns the animated offset if an animation is active, otherwise the base offset.
    pub fn effective_scroll_offset(&self) -> f64 {
        match &self.active_animation {
            Some(anim) => anim.current_offset(),
            None => self.scroll_offset,
        }
    }

    /// Start an animated scroll to a target offset.
    /// If an animation is already active, it will be cancelled and a new one started.
    pub fn start_scroll_animation(
        &mut self,
        target: f64,
        viewport_width: i32,
        duration_ms: Option<u64>,
        easing: Option<Easing>,
    ) {
        // Clamp target to valid range
        let max_scroll = (self.total_width() - viewport_width).max(0);
        let clamped_target = target.clamp(0.0, max_scroll as f64);

        // Use current effective position as start (handles interrupting animations)
        let start = self.effective_scroll_offset();

        // If already at target, no animation needed
        if (start - clamped_target).abs() < 0.5 {
            self.scroll_offset = clamped_target;
            self.active_animation = None;
            return;
        }

        let duration = duration_ms.unwrap_or(DEFAULT_ANIMATION_DURATION_MS);
        let ease = easing.unwrap_or_default();

        self.active_animation = Some(ScrollAnimation::new(start, clamped_target, duration, ease));
    }

    /// Advance the active animation by the given delta time in milliseconds.
    /// Returns true if an animation is still active, false if complete or no animation.
    pub fn tick_animation(&mut self, delta_ms: u64) -> bool {
        let Some(anim) = &mut self.active_animation else {
            return false;
        };

        let still_running = anim.tick(delta_ms);

        if !still_running {
            // Animation complete - finalize scroll offset and clear animation
            self.scroll_offset = anim.target();
            self.active_animation = None;
            false
        } else {
            true
        }
    }

    /// Stop the current animation and snap to the target position.
    pub fn stop_animation(&mut self) {
        if let Some(anim) = self.active_animation.take() {
            self.scroll_offset = anim.target();
        }
    }

    /// Cancel the current animation and stay at the current position.
    pub fn cancel_animation(&mut self) {
        if let Some(anim) = self.active_animation.take() {
            self.scroll_offset = anim.current_offset();
        }
    }

    /// Ensure the focused column is visible with animation.
    /// Like `ensure_focused_visible` but animates the scroll instead of jumping.
    pub fn ensure_focused_visible_animated(&mut self, viewport_width: i32) {
        if self.columns.is_empty() {
            return;
        }

        let Some((col_x, col_width)) = self.focused_column_bounds() else {
            return;
        };

        // Defensively clamp outer_gap to >= 0
        let outer_gap = self.outer_gap.max(0);

        let target_offset = match self.centering_mode {
            CenteringMode::Center => {
                // Center the focused column in the viewport
                let col_center = col_x.saturating_add(col_width / 2);
                (col_center.saturating_sub(viewport_width / 2)) as f64
            }
            CenteringMode::JustInView => {
                // Only scroll if the focused column is outside the viewport
                let current = self.effective_scroll_offset();
                let viewport_left = current.round() as i32;
                let viewport_right = viewport_left.saturating_add(viewport_width);
                let col_right = col_x.saturating_add(col_width);

                if col_x < viewport_left {
                    // Column is to the left of viewport, scroll left
                    col_x.saturating_sub(outer_gap) as f64
                } else if col_right > viewport_right {
                    // Column is to the right of viewport, scroll right
                    col_right
                        .saturating_add(outer_gap)
                        .saturating_sub(viewport_width) as f64
                } else {
                    // Already in view, no scroll needed
                    return;
                }
            }
        };

        self.start_scroll_animation(target_offset, viewport_width, None, None);
    }

    /// Compute placements for all windows, using animated scroll offset if active.
    ///
    /// This is similar to `compute_placements` but uses `effective_scroll_offset()`
    /// to support smooth scrolling animations.
    pub fn compute_placements_animated(&self, viewport: Rect) -> Vec<WindowPlacement> {
        // Use animated scroll offset
        let viewport_left = self.effective_scroll_offset().round() as i32;

        // Fullscreen mode: one window covers the entire viewport, others are off-screen
        if let Some(fs_wid) = self.fullscreen_window {
            return self.compute_fullscreen_placements(fs_wid, viewport, viewport_left);
        }

        self.compute_non_fullscreen_placements(viewport, viewport_left)
    }

    /// Compute placements when a window is fullscreen.
    /// The fullscreen window gets the full viewport; all others are marked off-screen.
    fn compute_fullscreen_placements(
        &self,
        fs_wid: WindowId,
        viewport: Rect,
        viewport_left: i32,
    ) -> Vec<WindowPlacement> {
        // Stale or minimized fullscreen target: fall back to normal placements.
        if !self.contains_window(fs_wid) || self.minimized_windows.contains(&fs_wid) {
            return self.compute_non_fullscreen_placements(viewport, viewport_left);
        }

        let mut placements = Vec::new();

        for (col_idx, column) in self.columns.iter().enumerate() {
            for &window_id in column.windows() {
                if self.minimized_windows.contains(&window_id) {
                    continue;
                }
                if window_id == fs_wid {
                    placements.push(WindowPlacement {
                        window_id,
                        rect: viewport,
                        visibility: Visibility::Visible,
                        column_index: col_idx,
                    });
                } else {
                    placements.push(WindowPlacement {
                        window_id,
                        rect: Rect::new(0, 0, 0, 0),
                        visibility: Visibility::OffScreenLeft,
                        column_index: col_idx,
                    });
                }
            }
        }

        // Floating windows are also hidden during fullscreen
        for floating in &self.floating_windows {
            if floating.id == fs_wid {
                placements.push(WindowPlacement {
                    window_id: floating.id,
                    rect: viewport,
                    visibility: Visibility::Visible,
                    column_index: usize::MAX,
                });
            } else {
                placements.push(WindowPlacement {
                    window_id: floating.id,
                    rect: Rect::new(0, 0, 0, 0),
                    visibility: Visibility::OffScreenLeft,
                    column_index: usize::MAX,
                });
            }
        }

        placements
    }

    // ========================================================================
    // Minimize Methods
    // ========================================================================

    /// Mark a window as minimized. The window stays in its column but is
    /// excluded from layout placement calculations.
    ///
    /// If the minimized window is the current fullscreen window, fullscreen
    /// mode is exited so that other windows become visible again.
    ///
    /// Returns `true` if the window was managed (tiled) and is now marked minimized.
    /// Returns `false` if the window is not in this workspace or is floating.
    pub fn mark_minimized(&mut self, window_id: WindowId) -> bool {
        // Only mark tiled windows as minimized (not floating)
        if self.find_window_location(window_id).is_some() {
            // If the fullscreen window is being minimized, exit fullscreen
            if self.fullscreen_window == Some(window_id) {
                self.fullscreen_window = None;
            }
            self.minimized_windows.insert(window_id)
        } else {
            false
        }
    }

    /// Mark a window as restored (no longer minimized).
    ///
    /// Returns `true` if the window was previously marked minimized.
    pub fn mark_restored(&mut self, window_id: WindowId) -> bool {
        self.minimized_windows.remove(&window_id)
    }

    /// Check if a window is currently minimized.
    pub fn is_minimized(&self, window_id: WindowId) -> bool {
        self.minimized_windows.contains(&window_id)
    }

    /// Get the number of currently minimized windows.
    pub fn minimized_count(&self) -> usize {
        self.minimized_windows.len()
    }

    // ========================================================================
    // Fullscreen Methods
    // ========================================================================

    /// Check if a window is currently fullscreen.
    pub fn is_fullscreen(&self) -> bool {
        self.fullscreen_window.is_some()
    }

    /// Get the fullscreen window ID, if any.
    pub fn fullscreen_window_id(&self) -> Option<WindowId> {
        self.fullscreen_window
    }

    /// Clear fullscreen mode when it currently targets `window_id`.
    ///
    /// Returns `true` if fullscreen was cleared.
    pub fn clear_fullscreen_if_window(&mut self, window_id: WindowId) -> bool {
        if self.fullscreen_window == Some(window_id) {
            self.fullscreen_window = None;
            true
        } else {
            false
        }
    }

    /// Toggle fullscreen mode for the focused window.
    /// Returns true if entering fullscreen, false if exiting.
    pub fn toggle_fullscreen(&mut self) -> bool {
        if let Some(fs_wid) = self.fullscreen_window {
            // If fullscreen points at a removed/minimized window, clear stale state
            // and treat this invocation as a fresh toggle attempt.
            if !self.contains_window(fs_wid) || self.minimized_windows.contains(&fs_wid) {
                self.fullscreen_window = None;
            } else {
                self.fullscreen_window = None;
                return false;
            }
        }

        if let Some(wid) = self.focused_visible_window() {
            self.fullscreen_window = Some(wid);
            true
        } else {
            self.fullscreen_window = None;
            false
        }
    }

    // ========================================================================
    // Toggle Floating
    // ========================================================================

    /// Toggle floating state for the focused window.
    /// If the focused window is tiled, move it to floating with a centered rect.
    /// If the focused window is floating, this is a no-op (floating windows are not focused via column focus).
    /// Returns the window ID that was toggled, if any.
    pub fn toggle_floating(&mut self, viewport: Rect) -> Option<WindowId> {
        let wid = self.focused_window()?;

        // Defensive guard: if the focused window is currently fullscreen, clear
        // fullscreen before moving it to floating state.
        self.clear_fullscreen_if_window(wid);

        // Check if the focused window is tiled (it always is since focus tracks tiled columns)
        // Remove from columns
        let _ = self.remove_window(wid);

        // Center a floating window of 800x600 or clamped to viewport
        let float_w = 800.min(viewport.width - 40);
        let float_h = 600.min(viewport.height - 40);
        let float_x = viewport.x + (viewport.width - float_w) / 2;
        let float_y = viewport.y + (viewport.height - float_h) / 2;
        let rect = Rect::new(float_x, float_y, float_w, float_h);

        let _ = self.add_floating(wid, rect);
        Some(wid)
    }

    /// Move a floating window back to the tiling layout.
    /// Returns true if the window was unfloated.
    pub fn unfloat_window(&mut self, window_id: WindowId) -> bool {
        if self.remove_floating(window_id) {
            // Insert as a new column
            let _ = self.insert_window(window_id, None);
            true
        } else {
            false
        }
    }

    // ========================================================================
    // Column Width Presets
    // ========================================================================

    /// Set the focused column's width as a fraction of the viewport width.
    /// Fraction should be between 0.1 and 1.0.
    pub fn set_focused_column_width_fraction(&mut self, fraction: f64, viewport_width: i32) {
        let fraction = fraction.clamp(0.1, 1.0);
        let outer_gap = self.outer_gap.max(0);
        let usable_width = viewport_width.saturating_sub(outer_gap * 2);
        let new_width = (usable_width as f64 * fraction).round() as i32;

        if let Some(column) = self.columns.get_mut(self.focused_column) {
            column.set_width(new_width);
        }
    }

    /// Equalize all column widths to share the viewport equally.
    pub fn equalize_column_widths(&mut self, viewport_width: i32) {
        if self.columns.is_empty() {
            return;
        }

        let outer_gap = self.outer_gap.max(0);
        let gap = self.gap.max(0);
        let n = self.columns.len() as i32;
        let total_gaps = gap * (n - 1) + outer_gap * 2;
        let per_column = ((viewport_width - total_gaps).max(MIN_COLUMN_WIDTH * n)) / n;

        for col in &mut self.columns {
            col.set_width(per_column);
        }
    }

    /// Set scroll offset directly (bypasses clamping).
    pub fn set_scroll_offset(&mut self, offset: f64) {
        self.scroll_offset = offset;
    }
}

// Test-only helper methods for direct state manipulation
#[cfg(test)]
impl Workspace {
    /// Set focus state directly without validation (test helper).
    pub fn test_set_focus_unchecked(&mut self, column: usize, win: usize) {
        self.focused_column = column;
        self.focused_window_in_column = win;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Workspace with one single-window column per width; window ids are 1..=n.
    fn ws_with_columns(gap: i32, outer_gap: i32, widths: &[i32]) -> Workspace {
        let mut ws = Workspace::with_gaps(gap, outer_gap);
        for (i, &width) in widths.iter().enumerate() {
            ws.insert_window(i as WindowId + 1, Some(width)).unwrap();
        }
        ws
    }

    /// Workspace with a single 400px column stacking `ids` top to bottom.
    /// Focus stays on the first (top) window.
    fn ws_with_stack(gap: i32, outer_gap: i32, ids: &[WindowId]) -> Workspace {
        let mut ws = Workspace::with_gaps(gap, outer_gap);
        ws.insert_window(ids[0], Some(400)).unwrap();
        for &id in &ids[1..] {
            ws.insert_window_in_column(id, 0).unwrap();
        }
        ws
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    // ========================================================================
    // Rect / Column
    // ========================================================================

    #[test]
    fn rect_clamps_negative_size_and_reports_edges() {
        let r = Rect::new(10, 20, -100, -50);
        assert_eq!((r.x, r.y, r.width, r.height), (10, 20, 0, 0));

        let r = Rect::new(-50, -30, 100, 80);
        assert_eq!((r.right(), r.bottom()), (50, 50));
    }

    #[test]
    fn rect_intersects_is_symmetric() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(50, 50, 100, 100);
        let c = Rect::new(200, 200, 50, 50);
        assert!(a.intersects(&b) && b.intersects(&a));
        assert!(!a.intersects(&c) && !c.intersects(&a));
    }

    #[test]
    fn column_width_is_clamped_to_minimum() {
        for (width, expected) in [
            (500, 500),
            (300, 300),
            (50, MIN_COLUMN_WIDTH),
            (-10, MIN_COLUMN_WIDTH),
        ] {
            assert_eq!(Column::new(1, width).width(), expected, "new({width})");
            assert_eq!(Column::empty(width).width(), expected, "empty({width})");
        }

        let empty = Column::empty(300);
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert!(empty.windows().is_empty());
    }

    #[test]
    fn column_membership_and_removal_index() {
        let mut col = Column::new(1, 400);
        col.add_window(2);
        col.add_window(3);
        assert_eq!(col.windows(), &[1, 2, 3]);
        assert!(col.contains(2) && !col.contains(999));
        assert_eq!((col.get(2), col.get(10)), (Some(3), None));

        // remove_window returns the index the window occupied.
        assert_eq!(col.remove_window(2), Some(1));
        assert_eq!(col.remove_window(1), Some(0));
        assert_eq!(col.remove_window(999), None);
        assert_eq!(col.windows(), &[3]);
    }

    // ========================================================================
    // Settings, insert and queries
    // ========================================================================

    #[test]
    fn gap_and_default_width_settings_are_clamped() {
        let ws = Workspace::with_gaps(-100, -50);
        assert_eq!((ws.gap(), ws.outer_gap()), (0, 0));

        let mut ws = Workspace::new();
        ws.set_gap(20);
        ws.set_outer_gap(15);
        assert_eq!((ws.gap(), ws.outer_gap()), (20, 15));
        ws.set_gap(-50);
        ws.set_outer_gap(-100);
        assert_eq!((ws.gap(), ws.outer_gap()), (0, 0));

        ws.set_default_column_width(500);
        assert_eq!(ws.default_column_width(), 500);
        ws.set_default_column_width(50);
        assert_eq!(ws.default_column_width(), MIN_COLUMN_WIDTH);
    }

    #[test]
    fn insert_window_appends_column_and_takes_focus() {
        let mut ws = Workspace::with_gaps(10, 10);
        assert!(ws.is_empty());
        assert_eq!(ws.total_width(), 0);

        ws.insert_window(1, Some(400)).unwrap();
        assert_eq!(ws.focused_window(), Some(1));

        ws.insert_window(2, Some(600)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();
        assert_eq!(ws.column_count(), 3);
        assert_eq!(ws.focused_column_index(), 2);
        assert_eq!(ws.focused_window(), Some(3));
        // outer + 400 + gap + 600 + gap + 400 + outer
        assert_eq!(ws.total_width(), 1440);
    }

    #[test]
    fn insert_window_width_defaults_and_clamps() {
        let mut ws = ws_with_columns(10, 10, &[0, -50]);
        assert_eq!(ws.columns()[0].width(), MIN_COLUMN_WIDTH);
        assert_eq!(ws.columns()[1].width(), MIN_COLUMN_WIDTH);

        ws.set_default_column_width(500);
        ws.insert_window(3, None).unwrap();
        assert_eq!(ws.columns()[2].width(), 500);
    }

    #[test]
    fn insert_window_no_focus_preserves_focus() {
        let mut ws = Workspace::new();
        // Into an empty workspace the first window must still take focus.
        ws.insert_window_no_focus(1, Some(400)).unwrap();
        assert_eq!(ws.focused_window(), Some(1));

        ws.insert_window(2, Some(400)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();
        ws.insert_window_no_focus(4, Some(400)).unwrap();
        assert_eq!(ws.focused_window(), Some(3));
        assert_eq!(ws.window_count(), 4);
        assert_eq!(ws.find_window_location(4), Some((3, 0)));
    }

    #[test]
    fn duplicate_window_ids_are_rejected_everywhere() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();
        ws.add_floating(2, Rect::new(100, 100, 400, 300)).unwrap();

        // Both a tiled (1) and a floating (2) id must be rejected by every insert path.
        for id in [1, 2] {
            assert!(
                matches!(ws.insert_window(id, Some(400)), Err(LayoutError::DuplicateWindow(d)) if d == id)
            );
            assert!(
                matches!(ws.insert_window_no_focus(id, Some(400)), Err(LayoutError::DuplicateWindow(d)) if d == id)
            );
            assert!(
                matches!(ws.insert_window_in_column(id, 0), Err(LayoutError::DuplicateWindow(d)) if d == id)
            );
            assert!(
                matches!(ws.add_floating(id, Rect::new(0, 0, 10, 10)), Err(LayoutError::DuplicateWindow(d)) if d == id)
            );
        }
        assert_eq!(ws.column_count(), 1);
        assert_eq!(ws.columns()[0].len(), 1);
        assert_eq!(ws.floating_count(), 1);
    }

    #[test]
    fn window_queries_reflect_layout() {
        let mut ws = Workspace::new();
        assert!(ws.column(0).is_none());
        assert_eq!(ws.window_count(), 0);

        ws.insert_window(1, Some(400)).unwrap(); // col 0
        ws.insert_window(2, Some(500)).unwrap(); // col 1
        ws.insert_window_in_column(3, 0).unwrap(); // col 0, index 1
        ws.insert_window_in_column(4, 1).unwrap(); // col 1, index 1
        ws.add_floating(5, Rect::new(0, 0, 100, 100)).unwrap();

        assert_eq!(ws.find_window_location(1), Some((0, 0)));
        assert_eq!(ws.find_window_location(2), Some((1, 0)));
        assert_eq!(ws.find_window_location(3), Some((0, 1)));
        assert_eq!(ws.find_window_location(4), Some((1, 1)));
        assert_eq!(ws.find_window_location(5), None); // floating is not in a column
        assert_eq!(ws.find_window_location(999), None);

        assert_eq!(ws.column(1).map(Column::width), Some(500));
        assert!(ws.column(2).is_none());

        // window_count is tiled-only; all_window_ids includes floating.
        assert_eq!(ws.window_count(), 4);
        let mut ids = ws.all_window_ids();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2, 3, 4, 5]);

        ws.remove_window(2).unwrap();
        assert_eq!(ws.window_count(), 3);
    }

    // ========================================================================
    // Remove
    // ========================================================================

    #[test]
    fn remove_window_drops_empty_column_and_allows_reinsert() {
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.remove_window(2).unwrap();
        assert_eq!(ws.column_count(), 2);
        assert!(!ws.contains_window(2));
        assert!(ws.contains_window(1) && ws.contains_window(3));

        ws.insert_window(2, Some(500)).unwrap();
        assert!(ws.contains_window(2));
        assert_eq!(ws.focused_window(), Some(2));
    }

    #[test]
    fn remove_last_window_resets_workspace_state() {
        let mut ws = ws_with_columns(10, 10, &[400, 400]);
        ws.set_scroll_offset(200.0);
        ws.remove_window(1).unwrap();
        ws.remove_window(2).unwrap();

        assert!(ws.is_empty());
        assert_eq!(ws.focused_window(), None);
        assert_eq!(ws.focused_column_index(), 0);
        assert_eq!(ws.focused_window_index_in_column(), 0);
        assert_eq!(ws.scroll_offset(), 0.0);
    }

    #[test]
    fn remove_column_left_of_focus_shifts_focus_index() {
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.test_set_focus_unchecked(2, 0);
        ws.remove_window(1).unwrap();
        assert_eq!(ws.focused_column_index(), 1);
        assert_eq!(ws.focused_window(), Some(3));
    }

    #[test]
    fn remove_window_clamps_stale_window_index_after_column_removal() {
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.insert_window_in_column(4, 2).unwrap();
        ws.insert_window_in_column(5, 2).unwrap(); // col 2: [3, 4, 5]

        ws.test_set_focus_unchecked(1, 99);
        ws.remove_window(2).unwrap(); // col 1 removed, focus lands on old col 2

        assert_eq!(ws.focused_column_index(), 1);
        assert_eq!(ws.focused_window_index_in_column(), 2);
        assert_eq!(ws.focused_window(), Some(5));
    }

    #[test]
    fn remove_window_in_stack_follows_focus_policy() {
        // (focused index, removed window, expected focused window, expected index)
        let cases = [
            (1, 1, 2, 0), // removed before focus: stay on the same window
            (1, 2, 3, 1), // removed the focused window: move to next
            (2, 3, 2, 1), // removed the focused last window: move to previous
            (0, 3, 1, 0), // removed after focus: unchanged
        ];
        for (focus_idx, removed, want_window, want_idx) in cases {
            let mut ws = ws_with_stack(10, 10, &[1, 2, 3]);
            ws.test_set_focus_unchecked(0, focus_idx);
            ws.remove_window(removed).unwrap();
            let case = (focus_idx, removed);
            assert_eq!(ws.focused_window(), Some(want_window), "case {case:?}");
            assert_eq!(
                ws.focused_window_index_in_column(),
                want_idx,
                "case {case:?}"
            );
        }
    }

    #[test]
    fn unknown_window_ids_are_reported() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();

        assert!(matches!(
            ws.remove_window(999),
            Err(LayoutError::WindowNotFound(999))
        ));
        assert!(!ws.remove_floating(999));
        assert!(!ws.update_floating(999, Rect::new(0, 0, 10, 10)));
        assert!(!ws.unfloat_window(999));
        assert_eq!(ws.window_count(), 1);
    }

    // ========================================================================
    // Focus
    // ========================================================================

    #[test]
    fn focus_left_right_stop_at_edges() {
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.focus_left();
        assert_eq!(ws.focused_column_index(), 1);
        assert_eq!(ws.focused_window(), Some(2));
        ws.focus_left();
        ws.focus_left();
        assert_eq!(ws.focused_column_index(), 0);

        ws.focus_right();
        ws.focus_right();
        ws.focus_right();
        assert_eq!(ws.focused_column_index(), 2);

        let mut empty = Workspace::new();
        empty.focus_left();
        empty.focus_right();
        empty.focus_up();
        empty.focus_down();
        assert_eq!(empty.focused_window(), None);
        assert_eq!(empty.focused_column_index(), 0);
        assert_eq!(empty.focused_window_index_in_column(), 0);
    }

    #[test]
    fn focus_right_clamps_window_index_to_shorter_column() {
        let mut ws = ws_with_columns(10, 10, &[400, 400]);
        ws.insert_window_in_column(3, 0).unwrap();
        ws.insert_window_in_column(4, 0).unwrap(); // col 0: [1, 3, 4], col 1: [2]

        ws.test_set_focus_unchecked(0, 2);
        ws.focus_right();

        assert_eq!(ws.focused_column_index(), 1);
        assert_eq!(ws.focused_window_index_in_column(), 0);
        assert_eq!(ws.focused_window(), Some(2));
    }

    #[test]
    fn focus_window_by_id() {
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.focus_window(1).unwrap();
        assert_eq!(ws.focused_column_index(), 0);
        assert_eq!(ws.focused_window(), Some(1));

        ws.focus_window(2).unwrap();
        assert_eq!(ws.focused_column_index(), 1);
        assert_eq!(ws.focused_window(), Some(2));

        assert!(matches!(
            ws.focus_window(999),
            Err(LayoutError::WindowNotFound(999))
        ));
    }

    #[test]
    fn set_focus_and_column_insert_validate_indices() {
        let mut ws = ws_with_columns(10, 10, &[400, 400]);
        ws.insert_window_in_column(3, 1).unwrap();

        ws.set_focus(1, 1).unwrap();
        assert_eq!(ws.focused_column_index(), 1);
        assert_eq!(ws.focused_window_index_in_column(), 1);
        assert_eq!(ws.focused_window(), Some(3));

        assert!(matches!(
            ws.set_focus(5, 0),
            Err(LayoutError::ColumnOutOfBounds(5, 1))
        ));
        assert!(matches!(
            ws.set_focus(0, 10),
            Err(LayoutError::WindowIndexOutOfBounds(10, 0, 0))
        ));
        assert!(matches!(
            ws.insert_window_in_column(9, 5),
            Err(LayoutError::ColumnOutOfBounds(5, 1))
        ));
    }

    #[test]
    fn focus_left_right_skip_fully_minimized_columns() {
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.mark_minimized(2);

        ws.focus_left();
        assert_eq!(ws.focused_window(), Some(1));
        ws.focus_right();
        assert_eq!(ws.focused_window(), Some(3));

        // No visible column in that direction: focus stays put.
        ws.mark_minimized(1);
        ws.focus_left();
        assert_eq!(ws.focused_window(), Some(3));
    }

    #[test]
    fn focus_up_down_skip_minimized_windows() {
        let mut ws = ws_with_stack(10, 10, &[1, 2, 3]);
        assert_eq!(ws.focused_window(), Some(1));
        ws.focus_down();
        assert_eq!(ws.focused_window(), Some(2));
        ws.focus_up();
        ws.focus_up(); // top edge
        assert_eq!(ws.focused_window(), Some(1));

        ws.mark_minimized(2);
        ws.focus_down();
        assert_eq!(ws.focused_window(), Some(3));
        ws.focus_down(); // bottom edge
        assert_eq!(ws.focused_window(), Some(3));
        ws.focus_up();
        assert_eq!(ws.focused_window(), Some(1));
    }

    #[test]
    fn focus_into_partly_minimized_column_lands_on_visible_window() {
        // Left: col 0 = [1 (minimized), 2], col 1 = [3] focused.
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window_in_column(2, 0).unwrap();
        ws.insert_window(3, Some(400)).unwrap();
        ws.mark_minimized(1);
        ws.focus_left();
        assert_eq!(ws.focused_window(), Some(2));

        // Right: col 0 = [1] focused, col 1 = [2 (minimized), 3].
        let mut ws = ws_with_columns(10, 10, &[400, 400]);
        ws.insert_window_in_column(3, 1).unwrap();
        ws.mark_minimized(2);
        ws.focus_window(1).unwrap();
        ws.focus_right();
        assert_eq!(ws.focused_window(), Some(3));
    }

    #[test]
    fn focus_at_edge_moves_off_minimized_window() {
        // Left boundary: col 0 = [1 (minimized, focused), 2].
        let mut ws = ws_with_stack(10, 10, &[1, 2]);
        ws.mark_minimized(1);
        ws.focus_left();
        assert_eq!(ws.focused_column_index(), 0);
        assert_eq!(ws.focused_window(), Some(2));

        // Right boundary: col 1 = [2 (minimized, focused), 3].
        let mut ws = ws_with_columns(10, 10, &[400, 400]);
        ws.insert_window_in_column(3, 1).unwrap();
        ws.mark_minimized(2);
        ws.focus_window(2).unwrap();
        ws.focus_right();
        assert_eq!(ws.focused_column_index(), 1);
        assert_eq!(ws.focused_window(), Some(3));
    }

    #[test]
    fn focused_visible_window_falls_back_past_minimized() {
        let mut ws = ws_with_stack(10, 10, &[1, 2]);
        ws.mark_minimized(1);
        // focused_window is the raw index; focused_visible_window searches down...
        assert_eq!(ws.focused_window(), Some(1));
        assert_eq!(ws.focused_visible_window(), Some(2));

        // ...then up.
        ws.mark_restored(1);
        ws.mark_minimized(2);
        ws.test_set_focus_unchecked(0, 1);
        assert_eq!(ws.focused_visible_window(), Some(1));

        ws.mark_minimized(1);
        assert_eq!(ws.focused_visible_window(), None);
    }

    #[test]
    fn invariants_hold_after_complex_sequence() {
        let mut ws = ws_with_columns(10, 10, &[400, 300, 500]);
        ws.insert_window_in_column(4, 1).unwrap();
        ws.insert_window_in_column(5, 1).unwrap();

        ws.focus_left();
        ws.focus_down();
        ws.move_column_right();
        ws.resize_focused_column(100);
        ws.focus_right();
        ws.focus_up();
        ws.remove_window(4).unwrap();
        ws.focus_left();
        ws.move_column_left();

        assert!(ws.focused_column < ws.columns.len());
        assert!(ws.focused_window_in_column < ws.columns[ws.focused_column].len());

        let mut all_windows: Vec<WindowId> = ws
            .columns
            .iter()
            .flat_map(|c| c.windows().iter().copied())
            .collect();
        all_windows.sort_unstable();
        let len_before = all_windows.len();
        all_windows.dedup();
        assert_eq!(all_windows.len(), len_before, "Duplicate windows found");
    }

    // ========================================================================
    // Move, resize and width presets
    // ========================================================================

    #[test]
    fn move_column_swaps_with_neighbour_and_stops_at_edges() {
        let mut ws = ws_with_columns(10, 10, &[400, 300, 500]);
        ws.test_set_focus_unchecked(1, 0);

        ws.move_column_left();
        assert_eq!(ws.focused_column_index(), 0);
        assert_eq!(ws.columns()[0].get(0), Some(2));
        assert_eq!(ws.columns()[1].get(0), Some(1));
        assert_eq!(ws.columns()[0].width(), 300); // width travels with the column

        ws.move_column_left(); // left edge: no-op
        assert_eq!(ws.focused_column_index(), 0);
        assert_eq!(ws.columns()[0].get(0), Some(2));

        ws.move_column_right();
        assert_eq!(ws.focused_column_index(), 1);
        assert_eq!(ws.columns()[0].get(0), Some(1));
        assert_eq!(ws.columns()[1].get(0), Some(2));

        ws.test_set_focus_unchecked(2, 0);
        ws.move_column_right(); // right edge: no-op
        assert_eq!(ws.focused_column_index(), 2);
        assert_eq!(ws.columns()[2].get(0), Some(3));
    }

    #[test]
    fn resize_focused_column_clamps_and_saturates() {
        let mut ws = ws_with_columns(10, 10, &[400]);
        for (delta, expected) in [
            (100, 500),
            (-200, 300),
            (-500, MIN_COLUMN_WIDTH),
            (i32::MIN, MIN_COLUMN_WIDTH),
            (i32::MAX, i32::MAX),
        ] {
            ws.resize_focused_column(delta);
            assert_eq!(ws.columns()[0].width(), expected, "delta {delta}");
        }
    }

    #[test]
    fn column_width_fraction_uses_usable_width_and_clamps() {
        // usable = 1920 - 2 * outer_gap(10) = 1900; fraction clamped to [0.1, 1.0]
        for (fraction, expected) in [(0.5, 950), (0.333, 633), (2.0, 1900), (0.0, 190)] {
            let mut ws = ws_with_columns(10, 10, &[400]);
            ws.set_focused_column_width_fraction(fraction, 1920);
            assert_eq!(ws.columns()[0].width(), expected, "fraction {fraction}");
        }
    }

    #[test]
    fn equalize_column_widths_shares_viewport_with_minimum_floor() {
        let mut ws = ws_with_columns(10, 10, &[300, 600, 400]);
        // (1920 - 2 * gap - 2 * outer_gap) / 3 = 626
        ws.equalize_column_widths(1920);
        assert!(ws.columns().iter().all(|c| c.width() == 626));

        ws.equalize_column_widths(200);
        assert!(ws.columns().iter().all(|c| c.width() == MIN_COLUMN_WIDTH));
    }

    // ========================================================================
    // Scrolling and centering
    // ========================================================================

    #[test]
    fn scroll_by_clamps_and_ignores_non_finite_deltas() {
        // total_width = 1240, viewport 500 -> max scroll 740
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.scroll_by(100.0, 500);
        assert_eq!(ws.scroll_offset(), 100.0);

        for delta in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            ws.scroll_by(delta, 500);
            assert_eq!(ws.scroll_offset(), 100.0, "delta {delta}");
        }

        ws.scroll_by(2000.0, 500);
        assert_eq!(ws.scroll_offset(), 740.0);
        ws.scroll_by(-5000.0, 500);
        assert_eq!(ws.scroll_offset(), 0.0);
    }

    #[test]
    fn ensure_focused_visible_center_mode_centers_and_clamps() {
        // Columns at strip x = 10, 420, 830 (width 400); total 1240; viewport 500.
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.set_centering_mode(CenteringMode::Center);
        // (focused column, start offset, expected offset)
        for (col, start, expected) in [(1, 0.0, 370.0), (0, 500.0, 0.0), (2, 0.0, 740.0)] {
            ws.test_set_focus_unchecked(col, 0);
            ws.set_scroll_offset(start);
            ws.ensure_focused_visible(500);
            assert_eq!(ws.scroll_offset(), expected, "column {col}");
        }
    }

    #[test]
    fn ensure_focused_visible_just_in_view_scrolls_minimally() {
        // Columns at strip x = 10, 220, 430 (width 200); total 640; viewport 500.
        let mut ws = ws_with_columns(10, 10, &[200, 200, 200]);
        ws.set_centering_mode(CenteringMode::JustInView);

        ws.test_set_focus_unchecked(0, 0);
        ws.ensure_focused_visible(500);
        assert_eq!(ws.scroll_offset(), 0.0); // already in view

        ws.test_set_focus_unchecked(2, 0);
        ws.ensure_focused_visible(500);
        assert_eq!(ws.scroll_offset(), 140.0); // right edge 630 + outer 10 - 500

        ws.resize_focused_column(500); // right edge now 1130
        ws.ensure_focused_visible(500);
        assert_eq!(ws.scroll_offset(), 640.0);

        ws.test_set_focus_unchecked(0, 0);
        ws.ensure_focused_visible(500);
        assert_eq!(ws.scroll_offset(), 0.0); // left edge 10 - outer 10
    }

    // ========================================================================
    // Placements
    // ========================================================================

    #[test]
    fn compute_placements_classifies_visibility_by_scroll_offset() {
        use Visibility::{OffScreenLeft as L, OffScreenRight as R, Visible as V};
        // Columns span strip x = 10-410, 420-820, 830-1230; viewport is 500 wide.
        let base = ws_with_columns(10, 10, &[400, 400, 400]);
        let cases = [
            (0.0, [V, V, R]),
            (330.0, [V, V, R]), // column left edge == viewport right is off-screen
            (420.0, [L, V, V]),
            (820.0, [L, L, V]), // column right edge == viewport left is off-screen
        ];
        for (offset, expected) in cases {
            let mut ws = base.clone();
            ws.set_scroll_offset(offset);
            let visibility: Vec<Visibility> = ws
                .compute_placements(Rect::new(0, 0, 500, 600))
                .iter()
                .map(|p| p.visibility)
                .collect();
            assert_eq!(visibility, expected, "offset {offset}");
        }
    }

    #[test]
    fn compute_placements_rect_math_with_gaps_and_viewport_origin() {
        let mut ws = ws_with_columns(10, 20, &[400, 300]);
        ws.set_scroll_offset(100.7); // rounds to 101 (not truncated to 100)
        let viewport = Rect::new(1920, 50, 1000, 600);
        let p = ws.compute_placements(viewport);

        // col 0 strip x = outer 20; screen x = 20 - 101 + 1920.
        // y = 50 + outer; height = 600 - 2 * outer.
        assert_eq!(p[0].rect, Rect::new(1839, 70, 400, 560));
        // col 1 strip x = 20 + 400 + gap 10 = 430.
        assert_eq!(p[1].rect, Rect::new(2249, 70, 300, 560));
        assert_eq!((p[0].column_index, p[1].column_index), (0, 1));
    }

    #[test]
    fn compute_placements_splits_stack_height_with_remainder_on_last() {
        let ws = ws_with_stack(10, 10, &[1, 2, 3]);
        // usable = 101 - 2 * 10 = 81; minus 2 gaps = 61 -> 20 each, last takes remainder.
        let viewport = Rect::new(0, 0, 500, 101);
        let placements = ws.compute_placements(viewport);
        let rows: Vec<(WindowId, i32, i32)> = placements
            .iter()
            .map(|p| (p.window_id, p.rect.y, p.rect.height))
            .collect();
        assert_eq!(rows, vec![(1, 10, 20), (2, 40, 20), (3, 70, 21)]);
        assert!(placements.iter().all(|p| p.column_index == 0));

        let animated: Vec<(WindowId, i32, i32)> = ws
            .compute_placements_animated(viewport)
            .iter()
            .map(|p| (p.window_id, p.rect.y, p.rect.height))
            .collect();
        assert_eq!(animated, rows);
    }

    #[test]
    fn compute_placements_degenerate_viewports_stay_non_negative() {
        let ws = ws_with_stack(10, 50, &[1, 2, 3]);
        // Shorter than 2 * outer_gap, and zero height.
        for viewport in [Rect::new(0, 0, 500, 80), Rect::new(0, 0, 500, 0)] {
            let p = ws.compute_placements(viewport);
            assert_eq!(p.len(), 3);
            assert!(p.iter().all(|p| p.rect.height >= 0), "{viewport:?}");
        }

        // Zero-width viewport: every column is off-screen right.
        let ws = ws_with_columns(10, 10, &[400, 400]);
        let p = ws.compute_placements(Rect::new(0, 0, 0, 600));
        assert_eq!(p.len(), 2);
        assert!(p.iter().all(|p| p.visibility == Visibility::OffScreenRight));
    }

    #[test]
    fn column_wider_than_viewport_is_visible_at_full_width() {
        let ws = ws_with_columns(10, 10, &[1000]);
        let p = ws.compute_placements(Rect::new(0, 0, 500, 600));
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].visibility, Visibility::Visible);
        assert_eq!(p[0].rect.width, 1000);
    }

    #[test]
    fn total_width_saturates() {
        let mut ws = Workspace::new();
        for i in 0..1000 {
            ws.insert_window(i, Some(i32::MAX / 100)).unwrap();
        }
        assert_eq!(ws.total_width(), i32::MAX);
    }

    // ========================================================================
    // Easing and ScrollAnimation
    // ========================================================================

    #[test]
    fn easing_curves_match_cubic_formulas_and_clamp_input() {
        let ts = [0.0, 0.25, 0.5, 0.75, 1.0];
        let cases = [
            (Easing::Linear, [0.0, 0.25, 0.5, 0.75, 1.0]),
            (Easing::EaseIn, [0.0, 0.015625, 0.125, 0.421875, 1.0]),
            (Easing::EaseOut, [0.0, 0.578125, 0.875, 0.984375, 1.0]),
            (Easing::EaseInOut, [0.0, 0.0625, 0.5, 0.9375, 1.0]),
        ];
        for (easing, expected) in cases {
            for (t, want) in ts.into_iter().zip(expected) {
                let got = easing.apply(t);
                assert!(approx(got, want), "{easing:?}({t}) = {got}, want {want}");
            }
            assert!(approx(easing.apply(-0.5), 0.0), "{easing:?} below 0");
            assert!(approx(easing.apply(1.5), 1.0), "{easing:?} above 1");
        }
    }

    #[test]
    fn scroll_animation_progress_and_offset() {
        let mut forward = ScrollAnimation::new(0.0, 100.0, 100, Easing::Linear);
        let mut backward = ScrollAnimation::new(100.0, 0.0, 100, Easing::Linear);
        // (elapsed, progress, complete, forward offset)
        for (elapsed, progress, complete, offset) in [
            (0, 0.0, false, 0.0),
            (50, 0.5, false, 50.0),
            (100, 1.0, true, 100.0),
            (150, 1.0, true, 100.0), // progress clamps past duration
        ] {
            forward.elapsed_ms = elapsed;
            backward.elapsed_ms = elapsed;
            assert!(approx(forward.progress(), progress), "elapsed {elapsed}");
            assert_eq!(forward.is_complete(), complete, "elapsed {elapsed}");
            assert!(
                approx(forward.current_offset(), offset),
                "elapsed {elapsed}"
            );
            assert!(
                approx(backward.current_offset(), 100.0 - offset),
                "elapsed {elapsed}"
            );
        }
        assert!(approx(forward.target(), 100.0));

        let instant = ScrollAnimation::new(0.0, 100.0, 0, Easing::Linear);
        assert!(approx(instant.progress(), 1.0));
        assert!(instant.is_complete());

        let mut eased = ScrollAnimation::new(0.0, 100.0, 100, Easing::EaseOut);
        eased.elapsed_ms = 50;
        assert!(approx(eased.current_offset(), 87.5));
    }

    #[test]
    fn scroll_animation_tick_reports_running_and_saturates() {
        let mut anim = ScrollAnimation::new(0.0, 100.0, 100, Easing::Linear);
        for expected_elapsed in [25, 50, 75] {
            assert!(anim.tick(25));
            assert_eq!(anim.elapsed_ms, expected_elapsed);
        }
        assert!(!anim.tick(25));
        assert_eq!(anim.elapsed_ms, 100);
        assert!(!anim.tick(50));
        assert_eq!(anim.elapsed_ms, 150);

        anim.elapsed_ms = u64::MAX - 10;
        anim.tick(100);
        assert_eq!(anim.elapsed_ms, u64::MAX);
    }

    // ========================================================================
    // Workspace animation
    // ========================================================================

    #[test]
    fn workspace_animation_ticks_to_target() {
        // total_width = 1240, viewport 500 -> max scroll 740
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        assert!(!ws.is_animating());
        assert!(!ws.tick_animation(16)); // nothing to tick

        ws.start_scroll_animation(200.0, 500, Some(100), Some(Easing::Linear));
        assert!(ws.is_animating());
        assert!(approx(ws.effective_scroll_offset(), 0.0));

        assert!(ws.tick_animation(50));
        assert!(approx(ws.effective_scroll_offset(), 100.0));
        assert_eq!(ws.scroll_offset(), 0.0); // base offset commits on completion

        assert!(!ws.tick_animation(50));
        assert!(!ws.is_animating());
        assert_eq!(ws.scroll_offset(), 200.0);

        // Defaults: DEFAULT_ANIMATION_DURATION_MS.
        ws.start_scroll_animation(0.0, 500, None, None);
        assert!(ws.tick_animation(DEFAULT_ANIMATION_DURATION_MS - 1));
        assert!(!ws.tick_animation(1));
        assert_eq!(ws.scroll_offset(), 0.0);
    }

    #[test]
    fn start_scroll_animation_clamps_target() {
        // Content narrower than viewport: any target clamps to the current 0.
        let mut ws = ws_with_columns(10, 10, &[400]);
        ws.start_scroll_animation(500.0, 1000, None, None);
        assert!(!ws.is_animating());
        ws.start_scroll_animation(0.0, 1000, None, None);
        assert!(!ws.is_animating());

        // Over-range target clamps to max scroll.
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.start_scroll_animation(10_000.0, 500, None, None);
        assert!(ws.is_animating());
        ws.stop_animation();
        assert_eq!(ws.scroll_offset(), 740.0);
    }

    #[test]
    fn stop_snaps_to_target_and_cancel_keeps_current_position() {
        let start_halfway = || {
            let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
            ws.start_scroll_animation(200.0, 500, Some(100), Some(Easing::Linear));
            ws.tick_animation(50);
            ws
        };

        let mut ws = start_halfway();
        ws.stop_animation();
        assert!(!ws.is_animating());
        assert!(approx(ws.scroll_offset(), 200.0));

        let mut ws = start_halfway();
        ws.cancel_animation();
        assert!(!ws.is_animating());
        assert!(approx(ws.scroll_offset(), 100.0));
    }

    #[test]
    fn new_animation_interrupts_from_current_position() {
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.start_scroll_animation(200.0, 500, Some(100), Some(Easing::Linear));
        ws.tick_animation(50);

        ws.start_scroll_animation(300.0, 500, Some(100), Some(Easing::Linear));
        assert!(ws.is_animating());
        assert!(approx(ws.effective_scroll_offset(), 100.0));

        ws.tick_animation(150);
        assert!(approx(ws.effective_scroll_offset(), 300.0));
    }

    #[test]
    fn compute_placements_animated_uses_effective_offset() {
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        let viewport = Rect::new(0, 0, 500, 600);
        ws.start_scroll_animation(200.0, 500, Some(100), Some(Easing::Linear));
        ws.tick_animation(50); // effective offset 100, base offset still 0

        assert_eq!(ws.compute_placements(viewport)[0].rect.x, 10);
        assert_eq!(ws.compute_placements_animated(viewport)[0].rect.x, -90);
    }

    #[test]
    fn ensure_focused_visible_animated_animates_toward_target() {
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400, 400, 400]);
        ws.ensure_focused_visible(500); // last column focused -> scrolled right
        assert!(ws.scroll_offset() > 0.0);

        ws.focus_window(1).unwrap();
        ws.ensure_focused_visible_animated(500);
        assert!(ws.is_animating());
        ws.stop_animation();
        assert_eq!(ws.scroll_offset(), 0.0);

        // JustInView: an already visible column starts no animation.
        ws.set_centering_mode(CenteringMode::JustInView);
        ws.ensure_focused_visible_animated(500);
        assert!(!ws.is_animating());
    }

    // ========================================================================
    // Floating
    // ========================================================================

    #[test]
    fn floating_windows_are_tracked_outside_columns() {
        let mut ws = Workspace::new();
        ws.add_floating(1, Rect::new(100, 100, 400, 300)).unwrap();
        assert!(ws.contains_window(1) && ws.is_floating(1));
        assert_eq!((ws.floating_count(), ws.column_count()), (1, 0));

        let moved = Rect::new(200, 200, 600, 400);
        assert!(ws.update_floating(1, moved));
        assert_eq!(ws.floating_windows()[0].rect, moved);

        assert!(ws.remove_floating(1));
        assert!(!ws.contains_window(1));
        assert_eq!(ws.floating_count(), 0);
    }

    #[test]
    fn floating_placements_follow_tiled_at_absolute_rect() {
        let mut ws = ws_with_columns(10, 10, &[400, 400]);
        let rect = Rect::new(500, 500, 300, 200);
        ws.add_floating(3, rect).unwrap();
        ws.set_scroll_offset(300.0); // floating windows ignore scroll
        assert!(!ws.is_floating(1));

        let p = ws.compute_placements(Rect::new(0, 0, 1920, 1080));
        assert_eq!(p.len(), 3);
        let f = &p[2];
        assert_eq!(
            (f.window_id, f.rect, f.visibility, f.column_index),
            (3, rect, Visibility::Visible, usize::MAX)
        );
    }

    #[test]
    fn toggle_floating_moves_focused_window_and_unfloat_restores_it() {
        let viewport = Rect::new(0, 0, 1920, 1080);
        let mut ws = Workspace::new();
        assert_eq!(ws.toggle_floating(viewport), None);

        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();
        assert_eq!(ws.toggle_floating(viewport), Some(2));
        assert!(ws.is_floating(2));
        assert_eq!((ws.column_count(), ws.floating_count()), (1, 1));
        // 800x600 centred in the viewport.
        assert_eq!(ws.floating_windows()[0].rect, Rect::new(560, 240, 800, 600));

        assert!(ws.unfloat_window(2));
        assert!(!ws.is_floating(2));
        assert_eq!(ws.column_count(), 2);
        assert_eq!(ws.focused_window(), Some(2));
    }

    #[test]
    fn toggle_floating_clears_fullscreen_for_focused_window() {
        let mut ws = ws_with_columns(10, 10, &[400, 400]);
        ws.toggle_fullscreen();
        assert_eq!(ws.fullscreen_window_id(), Some(2));

        assert_eq!(ws.toggle_floating(Rect::new(0, 0, 1920, 1080)), Some(2));
        assert!(ws.is_floating(2));
        assert_eq!(ws.fullscreen_window_id(), None);
    }

    // ========================================================================
    // Fullscreen
    // ========================================================================

    #[test]
    fn toggle_fullscreen_enters_and_exits() {
        let mut ws = Workspace::new();
        assert!(!ws.toggle_fullscreen()); // nothing to fullscreen
        assert!(!ws.is_fullscreen());

        ws.insert_window(1, Some(400)).unwrap();
        assert!(ws.toggle_fullscreen());
        assert_eq!(ws.fullscreen_window_id(), Some(1));
        assert!(!ws.toggle_fullscreen());
        assert_eq!(ws.fullscreen_window_id(), None);
    }

    #[test]
    fn toggle_fullscreen_targets_visible_window_when_focus_is_minimized() {
        let mut ws = ws_with_stack(10, 10, &[1, 2]);
        ws.mark_minimized(1);
        assert_eq!(ws.focused_window(), Some(1));

        // Stale fullscreen state pointing at a minimized window.
        ws.fullscreen_window = Some(1);
        assert!(ws.toggle_fullscreen());
        assert_eq!(ws.fullscreen_window_id(), Some(2));
    }

    #[test]
    fn fullscreen_placements_cover_viewport_and_hide_others() {
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.add_floating(4, Rect::new(0, 0, 100, 100)).unwrap();
        ws.focus_left();
        ws.toggle_fullscreen();
        assert_eq!(ws.fullscreen_window_id(), Some(2));

        let viewport = Rect::new(0, 0, 1920, 1080);
        for placements in [
            ws.compute_placements(viewport),
            ws.compute_placements_animated(viewport),
        ] {
            assert_eq!(placements.len(), 4);
            for p in &placements {
                if p.window_id == 2 {
                    assert_eq!((p.rect, p.visibility), (viewport, Visibility::Visible));
                } else {
                    assert_eq!(p.visibility, Visibility::OffScreenLeft, "{p:?}");
                }
            }
        }
    }

    #[test]
    fn fullscreen_placements_fall_back_when_target_is_minimized() {
        let mut ws = ws_with_columns(10, 10, &[400, 400]);
        ws.mark_minimized(1);
        ws.fullscreen_window = Some(1); // stale/minimized fullscreen target

        let p = ws.compute_placements(Rect::new(0, 0, 1920, 1080));
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].window_id, 2);
        assert_eq!(p[0].visibility, Visibility::Visible);
    }

    #[test]
    fn removing_fullscreen_window_clears_fullscreen() {
        // Tiled.
        let mut ws = ws_with_columns(10, 10, &[400, 400]);
        ws.focus_window(1).unwrap();
        ws.toggle_fullscreen();
        ws.remove_window(1).unwrap();
        assert_eq!(ws.fullscreen_window_id(), None);
        let p = ws.compute_placements(Rect::new(0, 0, 1920, 1080));
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].visibility, Visibility::Visible);

        // Floating.
        let mut ws = Workspace::new();
        ws.add_floating(10, Rect::new(100, 100, 500, 400)).unwrap();
        ws.add_floating(11, Rect::new(100, 100, 500, 400)).unwrap();
        ws.fullscreen_window = Some(10);
        assert!(ws.remove_floating(10));
        assert!(!ws.is_fullscreen());

        // clear_fullscreen_if_window only clears the matching target.
        ws.fullscreen_window = Some(11);
        assert!(!ws.clear_fullscreen_if_window(99));
        assert!(ws.is_fullscreen());
        assert!(ws.clear_fullscreen_if_window(11));
        assert!(!ws.is_fullscreen());
    }

    #[test]
    fn minimizing_fullscreen_window_exits_fullscreen() {
        let mut ws = ws_with_columns(10, 10, &[400, 400]);
        ws.focus_window(1).unwrap();
        ws.toggle_fullscreen();

        // Minimizing another window keeps fullscreen.
        ws.mark_minimized(2);
        assert_eq!(ws.fullscreen_window_id(), Some(1));
        ws.mark_restored(2);

        // Minimizing the fullscreen window exits it; others lay out normally.
        ws.mark_minimized(1);
        assert_eq!(ws.fullscreen_window_id(), None);
        let p = ws.compute_placements(Rect::new(0, 0, 1920, 1080));
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].window_id, 2);
        assert_eq!(p[0].visibility, Visibility::Visible);

        // Restoring does not re-enter fullscreen.
        ws.mark_restored(1);
        assert!(!ws.is_fullscreen());
    }

    // ========================================================================
    // Minimize
    // ========================================================================

    #[test]
    fn mark_minimized_tracks_only_tiled_windows() {
        let mut ws = ws_with_columns(10, 10, &[400, 400]);
        ws.add_floating(3, Rect::new(0, 0, 100, 100)).unwrap();

        assert!(!ws.mark_restored(1)); // not minimized yet
        assert!(ws.mark_minimized(1));
        assert!(!ws.mark_minimized(1)); // already minimized
        assert!(!ws.mark_minimized(3)); // floating
        assert!(!ws.mark_minimized(999)); // unknown
        assert!(ws.is_minimized(1));
        assert!(!ws.is_minimized(3) && !ws.is_minimized(999));
        assert_eq!(ws.minimized_count(), 1);

        // Minimized windows stay managed.
        assert!(ws.contains_window(1));
        assert_eq!(ws.window_count(), 2);
        assert!(ws.all_window_ids().contains(&1));

        assert!(ws.mark_restored(1));
        assert!(!ws.is_minimized(1));

        // Removing a minimized window drops it from the minimized set.
        ws.mark_minimized(2);
        ws.remove_window(2).unwrap();
        assert!(!ws.is_minimized(2));
        assert_eq!(ws.minimized_count(), 0);
    }

    #[test]
    fn placements_skip_minimized_windows() {
        let mut ws = ws_with_columns(0, 0, &[400, 400]);
        let viewport = Rect::new(0, 0, 1920, 1080);
        assert_eq!(ws.compute_placements(viewport).len(), 2);

        ws.mark_minimized(1);
        for p in [
            ws.compute_placements(viewport),
            ws.compute_placements_animated(viewport),
        ] {
            assert_eq!(p.len(), 1);
            assert_eq!(p[0].window_id, 2);
        }
    }

    #[test]
    fn minimized_windows_give_up_stack_height() {
        let mut ws = ws_with_stack(0, 0, &[1, 2]);
        let viewport = Rect::new(0, 0, 400, 1000);
        let heights = |ws: &Workspace| -> Vec<(WindowId, i32)> {
            ws.compute_placements(viewport)
                .iter()
                .map(|p| (p.window_id, p.rect.height))
                .collect()
        };

        assert_eq!(heights(&ws), vec![(1, 500), (2, 500)]);
        ws.mark_minimized(1);
        assert_eq!(heights(&ws), vec![(2, 1000)]);
        ws.mark_minimized(2);
        assert!(heights(&ws).is_empty());
    }
}
