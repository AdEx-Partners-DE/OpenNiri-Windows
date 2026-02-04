//! OpenNiri Core Layout Engine
//!
//! Platform-agnostic scrollable tiling layout engine inspired by Niri.
//!
//! This crate implements the "infinite horizontal strip" paradigm where:
//! - Windows are arranged in columns on an infinite horizontal strip
//! - The monitor acts as a viewport/camera sliding over this strip
//! - New windows append without resizing existing ones

use serde::{Deserialize, Serialize};
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
            Easing::EaseIn => t.powi(3),                 // Cubic ease in
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
        Self::new(start, target, DEFAULT_ANIMATION_DURATION_MS, Easing::default())
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

    /// Check if a window ID already exists in the workspace.
    pub fn contains_window(&self, window_id: WindowId) -> bool {
        self.columns.iter().any(|c| c.windows.contains(&window_id))
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

        let column_widths: i32 = self.columns.iter()
            .map(|c| c.width)
            .fold(0i32, |acc, w| acc.saturating_add(w));
        let gaps = gap.saturating_mul(self.columns.len().saturating_sub(1) as i32);
        let outer_gaps = outer_gap.saturating_mul(2);

        column_widths.saturating_add(gaps).saturating_add(outer_gaps)
    }

    /// Insert a new window as a new column to the right of the focused column.
    /// Column width is clamped to MIN_COLUMN_WIDTH (100px) minimum.
    ///
    /// # Errors
    ///
    /// Returns `LayoutError::DuplicateWindow` if the window ID already exists.
    pub fn insert_window(&mut self, window_id: WindowId, width: Option<i32>) -> Result<(), LayoutError> {
        if self.contains_window(window_id) {
            return Err(LayoutError::DuplicateWindow(window_id));
        }

        let column_width = width.unwrap_or(self.default_column_width).max(MIN_COLUMN_WIDTH);
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

    /// Move focus to the column on the left.
    pub fn focus_left(&mut self) {
        if self.focused_column > 0 {
            self.focused_column -= 1;
            // Clamp focused window in column
            let col_len = self.columns[self.focused_column].len();
            if self.focused_window_in_column >= col_len {
                self.focused_window_in_column = col_len.saturating_sub(1);
            }
        }

        debug_assert!(
            self.columns.is_empty()
                || (self.focused_column < self.columns.len()
                    && self.focused_window_in_column < self.columns[self.focused_column].len()),
            "Invariant violation: focus indices out of bounds after focus_left"
        );
    }

    /// Move focus to the column on the right.
    pub fn focus_right(&mut self) {
        if self.focused_column + 1 < self.columns.len() {
            self.focused_column += 1;
            // Clamp focused window in column
            let col_len = self.columns[self.focused_column].len();
            if self.focused_window_in_column >= col_len {
                self.focused_window_in_column = col_len.saturating_sub(1);
            }
        }

        debug_assert!(
            self.columns.is_empty()
                || (self.focused_column < self.columns.len()
                    && self.focused_window_in_column < self.columns[self.focused_column].len()),
            "Invariant violation: focus indices out of bounds after focus_right"
        );
    }

    /// Move focus to the window above in the current column.
    pub fn focus_up(&mut self) {
        if self.focused_window_in_column > 0 {
            self.focused_window_in_column -= 1;
        }
    }

    /// Move focus to the window below in the current column.
    pub fn focus_down(&mut self) {
        if let Some(column) = self.columns.get(self.focused_column) {
            if self.focused_window_in_column + 1 < column.len() {
                self.focused_window_in_column += 1;
            }
        }
    }

    /// Get the currently focused window ID.
    pub fn focused_window(&self) -> Option<WindowId> {
        self.columns
            .get(self.focused_column)
            .and_then(|col| col.windows.get(self.focused_window_in_column))
            .copied()
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
                    self.scroll_offset =
                        col_right.saturating_add(outer_gap).saturating_sub(viewport_width) as f64;
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
        let mut placements = Vec::new();

        if self.columns.is_empty() {
            return placements;
        }

        // Defensively clamp gaps to >= 0 in case fields were set directly
        let gap = self.gap.max(0);
        let outer_gap = self.outer_gap.max(0);

        // Use rounding instead of truncation to prevent sub-pixel jitter
        let viewport_left = self.scroll_offset.round() as i32;
        let viewport_right = viewport_left.saturating_add(viewport.width);

        let mut current_x = outer_gap;

        for (col_idx, column) in self.columns.iter().enumerate() {
            // Calculate column position in strip coordinates
            let col_strip_x = current_x;
            let col_strip_right = col_strip_x.saturating_add(column.width);

            // Transform to screen coordinates (relative to viewport)
            let col_screen_x = col_strip_x.saturating_sub(viewport_left).saturating_add(viewport.x);

            // Determine visibility
            let visibility = if col_strip_right <= viewport_left {
                Visibility::OffScreenLeft
            } else if col_strip_x >= viewport_right {
                Visibility::OffScreenRight
            } else {
                Visibility::Visible
            };

            // Calculate window heights (equal split for stacked windows)
            // Clamp usable_height to >= 0 to handle tight viewports
            // Use saturating arithmetic to prevent overflow
            let usable_height = viewport.height.saturating_sub(outer_gap.saturating_mul(2)).max(0);
            let window_count = column.windows.len() as i32;
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

            for (win_idx, &window_id) in column.windows.iter().enumerate() {
                // Adjust height for last window to handle rounding
                // Clamp to >= 0 to prevent negative dimensions
                let height = if win_idx == column.windows.len() - 1 {
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
            self.columns.swap(self.focused_column, self.focused_column - 1);
            self.focused_column -= 1;
        }
    }

    /// Move the focused column right (swap with the column to its right).
    pub fn move_column_right(&mut self) {
        if self.focused_column + 1 < self.columns.len() {
            self.columns.swap(self.focused_column, self.focused_column + 1);
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
                    col_right.saturating_add(outer_gap).saturating_sub(viewport_width) as f64
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
        let mut placements = Vec::new();

        if self.columns.is_empty() {
            return placements;
        }

        // Defensively clamp gaps to >= 0 in case fields were set directly
        let gap = self.gap.max(0);
        let outer_gap = self.outer_gap.max(0);

        // Use animated scroll offset
        let viewport_left = self.effective_scroll_offset().round() as i32;
        let viewport_right = viewport_left.saturating_add(viewport.width);

        let mut current_x = outer_gap;

        for (col_idx, column) in self.columns.iter().enumerate() {
            // Calculate column position in strip coordinates
            let col_strip_x = current_x;
            let col_strip_right = col_strip_x.saturating_add(column.width);

            // Transform to screen coordinates (relative to viewport)
            let col_screen_x = col_strip_x.saturating_sub(viewport_left).saturating_add(viewport.x);

            // Determine visibility
            let visibility = if col_strip_right <= viewport_left {
                Visibility::OffScreenLeft
            } else if col_strip_x >= viewport_right {
                Visibility::OffScreenRight
            } else {
                Visibility::Visible
            };

            // Calculate window heights (equal split for stacked windows)
            let usable_height = viewport.height.saturating_sub(outer_gap.saturating_mul(2)).max(0);
            let window_count = column.windows.len() as i32;
            let window_gaps = if window_count > 1 {
                gap.saturating_mul(window_count - 1)
            } else {
                0
            };
            let window_height = if window_count > 0 {
                ((usable_height - window_gaps).max(0)) / window_count
            } else {
                0
            };

            let mut window_y = viewport.y + outer_gap;

            for (win_idx, &window_id) in column.windows.iter().enumerate() {
                placements.push(WindowPlacement {
                    window_id,
                    rect: Rect::new(col_screen_x, window_y, column.width, window_height),
                    visibility,
                    column_index: col_idx,
                });

                window_y = window_y.saturating_add(window_height);
                if win_idx < column.windows.len() - 1 {
                    window_y = window_y.saturating_add(gap);
                }
            }

            current_x = current_x.saturating_add(column.width).saturating_add(gap);
        }

        placements
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

    /// Set scroll offset directly (test helper).
    pub fn test_set_scroll_offset(&mut self, offset: f64) {
        self.scroll_offset = offset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_empty_workspace() {
        let ws = Workspace::new();
        assert!(ws.is_empty());
        assert_eq!(ws.column_count(), 0);
        assert_eq!(ws.total_width(), 0);
    }

    #[test]
    fn test_insert_window() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();

        assert!(!ws.is_empty());
        assert_eq!(ws.column_count(), 1);
        assert_eq!(ws.focused_column_index(), 0);
        assert_eq!(ws.focused_window(), Some(1));
    }

    #[test]
    fn test_insert_multiple_windows() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(600)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();

        assert_eq!(ws.column_count(), 3);
        // Last inserted window should be focused
        assert_eq!(ws.focused_column_index(), 2);
        assert_eq!(ws.focused_window(), Some(3));

        // Total width: outer_gap + 400 + gap + 600 + gap + 400 + outer_gap
        // = 10 + 400 + 10 + 600 + 10 + 400 + 10 = 1440
        assert_eq!(ws.total_width(), 1440);
    }

    #[test]
    fn test_focus_navigation() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();

        assert_eq!(ws.focused_column_index(), 2); // Last inserted

        ws.focus_left();
        assert_eq!(ws.focused_column_index(), 1);
        assert_eq!(ws.focused_window(), Some(2));

        ws.focus_left();
        assert_eq!(ws.focused_column_index(), 0);

        // Should not go below 0
        ws.focus_left();
        assert_eq!(ws.focused_column_index(), 0);

        ws.focus_right();
        ws.focus_right();
        assert_eq!(ws.focused_column_index(), 2);

        // Should not go beyond last column
        ws.focus_right();
        assert_eq!(ws.focused_column_index(), 2);
    }

    #[test]
    fn test_remove_window() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();

        assert_eq!(ws.column_count(), 3);

        ws.remove_window(2).unwrap();
        assert_eq!(ws.column_count(), 2);

        // Windows 1 and 3 should remain
        assert!(ws
            .columns()
            .iter()
            .any(|c| c.contains(1)));
        assert!(ws
            .columns()
            .iter()
            .any(|c| c.contains(3)));
    }

    #[test]
    fn test_compute_placements_visibility() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)).unwrap(); // x: 10-410
        ws.insert_window(2, Some(400)).unwrap(); // x: 420-820
        ws.insert_window(3, Some(400)).unwrap(); // x: 830-1230

        ws.test_set_scroll_offset(0.0);

        // Viewport of 500px wide starting at (0, 0)
        let viewport = Rect::new(0, 0, 500, 600);
        let placements = ws.compute_placements(viewport);

        assert_eq!(placements.len(), 3);

        // First column should be visible
        assert_eq!(placements[0].visibility, Visibility::Visible);
        assert_eq!(placements[0].window_id, 1);

        // Second column partially visible
        assert_eq!(placements[1].visibility, Visibility::Visible);

        // Third column off-screen right
        assert_eq!(placements[2].visibility, Visibility::OffScreenRight);
    }

    #[test]
    fn test_ensure_focused_visible_center() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.set_centering_mode(CenteringMode::Center);

        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();

        ws.test_set_focus_unchecked(0, 0);
        ws.test_set_scroll_offset(500.0); // Start scrolled right

        ws.ensure_focused_visible(500);

        // Should center column 0 in the viewport
        // Column 0 is at x=10, width=400, center=210
        // Viewport width=500, center=250
        // scroll_offset = 210 - 250 = -40, clamped to 0
        assert_eq!(ws.scroll_offset(), 0.0);
    }

    #[test]
    fn test_stacked_windows() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window_in_column(2, 0).unwrap();
        ws.insert_window_in_column(3, 0).unwrap();

        assert_eq!(ws.column_count(), 1);
        assert_eq!(ws.columns()[0].len(), 3);

        let viewport = Rect::new(0, 0, 500, 600);
        let placements = ws.compute_placements(viewport);

        assert_eq!(placements.len(), 3);
        // All three windows should be in the same column
        assert!(placements.iter().all(|p| p.column_index == 0));
    }

    #[test]
    fn test_resize_column() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();

        assert_eq!(ws.columns()[0].width(), 400);

        ws.resize_focused_column(100);
        assert_eq!(ws.columns()[0].width(), 500);

        ws.resize_focused_column(-200);
        assert_eq!(ws.columns()[0].width(), 300);

        // Should not go below minimum (100)
        ws.resize_focused_column(-500);
        assert_eq!(ws.columns()[0].width(), 100);
    }

    #[test]
    fn test_move_column() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();

        ws.test_set_focus_unchecked(1, 0);
        ws.move_column_left();

        assert_eq!(ws.focused_column_index(), 0);
        assert_eq!(ws.columns()[0].get(0), Some(2));
        assert_eq!(ws.columns()[1].get(0), Some(1));

        ws.move_column_right();
        assert_eq!(ws.focused_column_index(), 1);
        assert_eq!(ws.columns()[0].get(0), Some(1));
        assert_eq!(ws.columns()[1].get(0), Some(2));
    }

    #[test]
    fn test_scroll_by() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();

        let viewport_width = 500;

        ws.scroll_by(100.0, viewport_width);
        assert_eq!(ws.scroll_offset(), 100.0);

        ws.scroll_by(2000.0, viewport_width);
        // Should clamp to max scroll
        let max_scroll = (ws.total_width() - viewport_width).max(0) as f64;
        assert_eq!(ws.scroll_offset(), max_scroll);

        ws.scroll_by(-5000.0, viewport_width);
        assert_eq!(ws.scroll_offset(), 0.0);
    }

    #[test]
    fn test_rect_intersects() {
        let r1 = Rect::new(0, 0, 100, 100);
        let r2 = Rect::new(50, 50, 100, 100);
        let r3 = Rect::new(200, 200, 50, 50);

        assert!(r1.intersects(&r2));
        assert!(r2.intersects(&r1));
        assert!(!r1.intersects(&r3));
        assert!(!r3.intersects(&r1));
    }

    // ====== Tests added from code review (Cycle 1) ======

    #[test]
    fn test_remove_last_window_empties_workspace() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();

        ws.remove_window(1).unwrap();

        assert!(ws.is_empty());
        assert_eq!(ws.column_count(), 0);
        assert_eq!(ws.focused_column_index(), 0);
        assert_eq!(ws.focused_window_index_in_column(), 0);
        assert_eq!(ws.scroll_offset(), 0.0);
    }

    #[test]
    fn test_ensure_focused_visible_just_in_view() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.set_centering_mode(CenteringMode::JustInView);

        ws.insert_window(1, Some(200)).unwrap(); // x: 10-210
        ws.insert_window(2, Some(200)).unwrap(); // x: 220-420
        ws.insert_window(3, Some(200)).unwrap(); // x: 430-630

        ws.test_set_focus_unchecked(0, 0);
        ws.test_set_scroll_offset(0.0);

        // Column 0 is already in view - should NOT scroll
        ws.ensure_focused_visible(500);
        assert_eq!(ws.scroll_offset(), 0.0);

        // Focus column 2, which is partially out of view
        ws.test_set_focus_unchecked(2, 0);
        ws.ensure_focused_visible(500);
        // Should scroll just enough to bring column 2 into view
        assert!(ws.scroll_offset() > 0.0);
    }

    #[test]
    fn test_compute_placements_tight_viewport() {
        let mut ws = Workspace::with_gaps(10, 50); // Large outer_gap
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window_in_column(2, 0).unwrap();
        ws.insert_window_in_column(3, 0).unwrap();

        // Viewport smaller than outer_gaps * 2
        let viewport = Rect::new(0, 0, 500, 80); // Only 80px tall
        let placements = ws.compute_placements(viewport);

        // All heights should be >= 0
        for p in &placements {
            assert!(p.rect.height >= 0, "Height was negative: {}", p.rect.height);
        }
    }

    #[test]
    fn test_insert_window_clamps_width() {
        let mut ws = Workspace::new();

        // Try to insert with zero width
        ws.insert_window(1, Some(0)).unwrap();
        assert_eq!(ws.columns()[0].width(), 100); // Clamped to minimum

        // Try to insert with negative width
        ws.insert_window(2, Some(-50)).unwrap();
        assert_eq!(ws.columns()[1].width(), 100); // Clamped to minimum
    }

    #[test]
    fn test_scroll_offset_rounding() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();

        // Set fractional scroll offset
        ws.test_set_scroll_offset(100.7);

        let viewport = Rect::new(0, 0, 500, 600);
        let placements = ws.compute_placements(viewport);

        // Verify placements use rounded value (101, not truncated 100)
        // The first column at x=10 should be at screen x = 10 - 101 + 0 = -91
        assert_eq!(placements[0].rect.x, -91);
    }

    // ====== Tests added from code review (Cycle 2) ======

    #[test]
    fn test_column_empty_constructor() {
        let col = Column::empty(50);
        assert_eq!(col.width(), 100); // Clamped to MIN_COLUMN_WIDTH
        assert!(col.is_empty());
        assert_eq!(col.len(), 0);
    }

    #[test]
    fn test_rect_right_and_bottom() {
        let r = Rect::new(10, 20, 100, 50);
        assert_eq!(r.right(), 110);
        assert_eq!(r.bottom(), 70);

        // Edge case: negative coordinates
        let r2 = Rect::new(-50, -30, 100, 80);
        assert_eq!(r2.right(), 50);
        assert_eq!(r2.bottom(), 50);
    }

    #[test]
    fn test_focus_operations_on_empty_workspace() {
        let mut ws = Workspace::new();

        // All focus operations should safely do nothing
        ws.focus_left();
        ws.focus_right();
        ws.focus_up();
        ws.focus_down();

        assert!(ws.focused_window().is_none());
        assert_eq!(ws.focused_column_index(), 0);
        assert_eq!(ws.focused_window_index_in_column(), 0);
    }

    #[test]
    fn test_remove_nonexistent_window() {
        let mut ws = Workspace::new();
        let result = ws.remove_window(999);
        assert!(matches!(result, Err(LayoutError::WindowNotFound(999))));
    }

    #[test]
    fn test_remove_window_adjusts_focus_correctly() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();

        // Focus on column 2 (window 3)
        ws.test_set_focus_unchecked(2, 0);

        // Remove from column 0
        ws.remove_window(1).unwrap();

        // Focus should adjust: was 2, column 0 removed, now should be 1
        assert_eq!(ws.focused_column_index(), 1);
        assert_eq!(ws.focused_window(), Some(3));
    }

    #[test]
    fn test_duplicate_window_rejected() {
        let mut ws = Workspace::new();
        ws.insert_window(42, Some(400)).unwrap();

        // Try to insert same window as new column
        let result = ws.insert_window(42, Some(400));
        assert!(matches!(result, Err(LayoutError::DuplicateWindow(42))));

        // Try to insert same window into existing column
        let result = ws.insert_window_in_column(42, 0);
        assert!(matches!(result, Err(LayoutError::DuplicateWindow(42))));

        // Workspace should still have only one column with one window
        assert_eq!(ws.column_count(), 1);
        assert_eq!(ws.columns()[0].len(), 1);
    }

    #[test]
    fn test_rect_clamps_negative_dimensions() {
        let r = Rect::new(10, 20, -100, -50);
        assert_eq!(r.width, 0);
        assert_eq!(r.height, 0);
        assert_eq!(r.x, 10);
        assert_eq!(r.y, 20);
    }

    #[test]
    fn test_total_width_saturates() {
        let mut ws = Workspace::new();

        // Insert many columns with large widths to test saturation
        for i in 0..1000 {
            ws.insert_window(i, Some(i32::MAX / 100)).unwrap();
        }

        // Should saturate to i32::MAX instead of overflowing/panicking
        let width = ws.total_width();
        assert!(width > 0); // Should not wrap to negative
        assert_eq!(width, i32::MAX); // Should saturate at max
    }

    // ====== Tests added from code review (Cycle 4) ======

    #[test]
    fn test_focus_window_by_id() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();

        // Focus is on column 2 (window 3) after inserts
        assert_eq!(ws.focused_window(), Some(3));

        // Focus window 1 by ID
        ws.focus_window(1).unwrap();
        assert_eq!(ws.focused_column_index(), 0);
        assert_eq!(ws.focused_window(), Some(1));

        // Focus window 2 by ID
        ws.focus_window(2).unwrap();
        assert_eq!(ws.focused_column_index(), 1);
        assert_eq!(ws.focused_window(), Some(2));

        // Try to focus nonexistent window
        let result = ws.focus_window(999);
        assert!(matches!(result, Err(LayoutError::WindowNotFound(999))));
    }

    #[test]
    fn test_set_focus_validates() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();
        ws.insert_window_in_column(3, 1).unwrap(); // Stack window 3 in column 1

        // Valid focus
        ws.set_focus(1, 1).unwrap();
        assert_eq!(ws.focused_column_index(), 1);
        assert_eq!(ws.focused_window_index_in_column(), 1);
        assert_eq!(ws.focused_window(), Some(3));

        // Invalid column index
        let result = ws.set_focus(5, 0);
        assert!(matches!(result, Err(LayoutError::ColumnOutOfBounds(5, 1))));

        // Invalid window index in column
        let result = ws.set_focus(0, 10);
        assert!(matches!(result, Err(LayoutError::WindowIndexOutOfBounds(10, 0, 0))));
    }

    #[test]
    fn test_scroll_by_special_floats() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();

        let viewport_width = 500;

        // Scroll to a known position
        ws.scroll_by(50.0, viewport_width);
        assert_eq!(ws.scroll_offset(), 50.0);

        // NaN should be treated as zero (no change)
        ws.scroll_by(f64::NAN, viewport_width);
        assert_eq!(ws.scroll_offset(), 50.0);

        // Infinity should be treated as zero (no change)
        ws.scroll_by(f64::INFINITY, viewport_width);
        assert_eq!(ws.scroll_offset(), 50.0);

        // Negative infinity should be treated as zero (no change)
        ws.scroll_by(f64::NEG_INFINITY, viewport_width);
        assert_eq!(ws.scroll_offset(), 50.0);
    }

    #[test]
    fn test_column_width_getter() {
        let col = Column::new(1, 500);
        assert_eq!(col.width(), 500);

        let col2 = Column::new(2, 50); // Below minimum
        assert_eq!(col2.width(), 100); // Clamped
    }

    #[test]
    fn test_column_contains() {
        let mut col = Column::new(1, 400);
        col.add_window(2);
        col.add_window(3);

        assert!(col.contains(1));
        assert!(col.contains(2));
        assert!(col.contains(3));
        assert!(!col.contains(999));

        // Test get() method
        assert_eq!(col.get(0), Some(1));
        assert_eq!(col.get(1), Some(2));
        assert_eq!(col.get(2), Some(3));
        assert_eq!(col.get(10), None);

        // Test windows() slice
        assert_eq!(col.windows(), &[1, 2, 3]);
    }

    // ====== Tests added from code review (Cycle 5) ======

    #[test]
    fn test_remove_window_before_focus_in_stacked_column() {
        // Bug test: removing a window BEFORE the focused window in a stacked column
        // should keep focus on the same window (index should decrement)
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap(); // Column 0
        ws.insert_window_in_column(2, 0).unwrap(); // Stack: [1, 2]
        ws.insert_window_in_column(3, 0).unwrap(); // Stack: [1, 2, 3]

        // Focus on window 2 (index 1)
        ws.test_set_focus_unchecked(0, 1);
        assert_eq!(ws.focused_window(), Some(2));

        // Remove window 1 (index 0, before focused)
        ws.remove_window(1).unwrap();

        // Focus should still be on window 2, but index should now be 0
        assert_eq!(ws.focused_window(), Some(2));
        assert_eq!(ws.focused_window_index_in_column(), 0);
    }

    #[test]
    fn test_remove_focused_window_in_stacked_column() {
        // Removing the focused window should move focus to next window (or previous if at end)
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window_in_column(2, 0).unwrap();
        ws.insert_window_in_column(3, 0).unwrap(); // Stack: [1, 2, 3]

        // Focus on window 2 (index 1, middle)
        ws.test_set_focus_unchecked(0, 1);
        assert_eq!(ws.focused_window(), Some(2));

        // Remove window 2 (the focused window)
        ws.remove_window(2).unwrap();

        // Stack is now [1, 3], focus index 1 should point to window 3 (next)
        assert_eq!(ws.focused_window(), Some(3));
        assert_eq!(ws.focused_window_index_in_column(), 1);
    }

    #[test]
    fn test_remove_last_focused_window_in_stacked_column() {
        // Removing the last focused window should move focus to previous
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window_in_column(2, 0).unwrap();
        ws.insert_window_in_column(3, 0).unwrap(); // Stack: [1, 2, 3]

        // Focus on window 3 (index 2, last)
        ws.test_set_focus_unchecked(0, 2);
        assert_eq!(ws.focused_window(), Some(3));

        // Remove window 3 (the focused window, at end)
        ws.remove_window(3).unwrap();

        // Stack is now [1, 2], focus should move to index 1 (window 2)
        assert_eq!(ws.focused_window(), Some(2));
        assert_eq!(ws.focused_window_index_in_column(), 1);
    }

    #[test]
    fn test_compute_placements_wide_column() {
        // Column wider than viewport
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(1000)).unwrap(); // Column wider than viewport

        let viewport = Rect::new(0, 0, 500, 600); // Viewport only 500px wide
        let placements = ws.compute_placements(viewport);

        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].visibility, Visibility::Visible);
        assert_eq!(placements[0].rect.width, 1000); // Full column width preserved
    }

    #[test]
    fn test_column_empty_type() {
        // Tests the Column::empty() constructor and its properties.
        // Note: In practice, empty columns are automatically removed from workspaces
        // when the last window is removed, so empty columns don't occur in normal use.
        // Column::empty() exists for construction purposes (e.g., building columns
        // before adding windows).
        let empty_col = Column::empty(300);
        assert!(empty_col.is_empty());
        assert_eq!(empty_col.width(), 300);
        assert_eq!(empty_col.len(), 0);
        assert_eq!(empty_col.windows(), &[]);

        // Verify workspace doesn't produce placements for non-existent windows
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();

        let viewport = Rect::new(0, 0, 2000, 600);
        let placements = ws.compute_placements(viewport);
        assert_eq!(placements.len(), 2); // Only 2 windows, no extras
    }

    #[test]
    fn test_negative_gaps_clamped() {
        // Negative gaps should be clamped to 0
        let ws = Workspace::with_gaps(-100, -50);
        assert_eq!(ws.gap(), 0);
        assert_eq!(ws.outer_gap(), 0);
    }

    #[test]
    fn test_gap_setters_clamp() {
        let mut ws = Workspace::new();

        // Test gap setter
        ws.set_gap(20);
        assert_eq!(ws.gap(), 20);
        ws.set_gap(-50);
        assert_eq!(ws.gap(), 0); // Clamped

        // Test outer_gap setter
        ws.set_outer_gap(15);
        assert_eq!(ws.outer_gap(), 15);
        ws.set_outer_gap(-100);
        assert_eq!(ws.outer_gap(), 0); // Clamped

        // Test default_column_width setter
        ws.set_default_column_width(500);
        assert_eq!(ws.default_column_width(), 500);
        ws.set_default_column_width(50); // Below MIN_COLUMN_WIDTH
        assert_eq!(ws.default_column_width(), 100); // Clamped to minimum

        // Test centering_mode getter/setter
        assert_eq!(ws.centering_mode(), CenteringMode::Center); // Default
        ws.set_centering_mode(CenteringMode::JustInView);
        assert_eq!(ws.centering_mode(), CenteringMode::JustInView);
    }

    #[test]
    fn test_compute_placements_spacing_integrity() {
        // Verify stacked window heights + gaps sum correctly
        let mut ws = Workspace::with_gaps(10, 20);
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window_in_column(2, 0).unwrap();
        ws.insert_window_in_column(3, 0).unwrap(); // Stack: [1, 2, 3]

        let viewport = Rect::new(0, 0, 500, 600);
        let placements = ws.compute_placements(viewport);

        // usable_height = 600 - 20*2 = 560
        // 3 windows with 2 gaps = 560 - 10*2 = 540 for windows
        // Each window ~180px, but last takes remainder

        let total_height: i32 = placements.iter().map(|p| p.rect.height).sum();
        let expected_usable = viewport.height - ws.outer_gap() * 2;
        let expected_gaps = ws.gap() * (placements.len() as i32 - 1);

        // Total heights + gaps should equal usable height
        assert_eq!(total_height + expected_gaps, expected_usable);
    }

    #[test]
    fn test_column_remove_returns_index() {
        let mut col = Column::new(1, 400);
        col.add_window(2);
        col.add_window(3);
        // Windows: [1, 2, 3]

        // Remove middle window
        let removed = col.remove_window(2);
        assert_eq!(removed, Some(1)); // Index 1

        // Remove first window
        let removed = col.remove_window(1);
        assert_eq!(removed, Some(0)); // Index 0

        // Remove nonexistent
        let removed = col.remove_window(999);
        assert_eq!(removed, None);
    }

    // ====== Tests added from code review (Cycle 7) ======

    #[test]
    fn test_compute_placements_zero_viewport_width() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();

        // Zero width viewport - edge case
        let viewport = Rect::new(0, 0, 0, 600);
        let placements = ws.compute_placements(viewport);

        // Should produce placements without panicking
        assert_eq!(placements.len(), 2);
        // All columns should be off-screen right (viewport has no width)
        for p in &placements {
            assert_eq!(p.visibility, Visibility::OffScreenRight);
        }
    }

    #[test]
    fn test_compute_placements_zero_viewport_height() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window_in_column(2, 0).unwrap();

        // Zero height viewport - edge case
        let viewport = Rect::new(0, 0, 500, 0);
        let placements = ws.compute_placements(viewport);

        // Should produce placements without panicking
        assert_eq!(placements.len(), 2);
        // All heights should be >= 0 (clamped)
        for p in &placements {
            assert!(p.rect.height >= 0, "Height was negative: {}", p.rect.height);
        }
    }

    #[test]
    fn test_focus_navigation_clamps_to_shorter_column() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap(); // Column 0: [1]
        ws.insert_window(2, Some(400)).unwrap(); // Column 1: [2]
        ws.insert_window_in_column(3, 0).unwrap(); // Column 0: [1, 3]
        ws.insert_window_in_column(4, 0).unwrap(); // Column 0: [1, 3, 4]

        // Focus on window 4 (column 0, index 2)
        ws.test_set_focus_unchecked(0, 2);
        assert_eq!(ws.focused_window(), Some(4));

        // Move right to column 1 which only has 1 window
        ws.focus_right();

        // Focus should clamp to index 0 (the only window in column 1)
        assert_eq!(ws.focused_column_index(), 1);
        assert_eq!(ws.focused_window_index_in_column(), 0);
        assert_eq!(ws.focused_window(), Some(2));
    }

    #[test]
    fn test_resize_then_ensure_focused_visible() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.set_centering_mode(CenteringMode::JustInView);
        ws.insert_window(1, Some(200)).unwrap();
        ws.insert_window(2, Some(200)).unwrap();
        ws.insert_window(3, Some(200)).unwrap();

        // Focus column 2, resize it significantly
        ws.test_set_focus_unchecked(2, 0);
        ws.resize_focused_column(500); // Now 700px wide

        // Ensure focused visible should adjust scroll
        ws.ensure_focused_visible(500);

        // Should have scrolled to bring the widened column into view
        assert!(ws.scroll_offset() > 0.0);
    }

    #[test]
    fn test_move_column_then_resize() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(300)).unwrap();
        ws.insert_window(3, Some(500)).unwrap();

        // Focus column 1 (window 2, 300px)
        ws.test_set_focus_unchecked(1, 0);
        assert_eq!(ws.columns()[1].width(), 300);

        // Move column left
        ws.move_column_left();
        assert_eq!(ws.focused_column_index(), 0);
        assert_eq!(ws.columns()[0].width(), 300); // Column with window 2

        // Resize the moved column
        ws.resize_focused_column(100);
        assert_eq!(ws.columns()[0].width(), 400);
    }

    #[test]
    fn test_remove_reinsert_same_window_id() {
        let mut ws = Workspace::new();
        ws.insert_window(42, Some(400)).unwrap();
        ws.insert_window(100, Some(300)).unwrap();

        // Remove window 42
        ws.remove_window(42).unwrap();
        assert!(!ws.contains_window(42));

        // Re-insert same ID should work now
        ws.insert_window(42, Some(500)).unwrap();
        assert!(ws.contains_window(42));
        assert_eq!(ws.focused_window(), Some(42));
    }

    #[test]
    fn test_find_window_location() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap(); // Column 0
        ws.insert_window(2, Some(400)).unwrap(); // Column 1
        ws.insert_window_in_column(3, 0).unwrap(); // Column 0, index 1
        ws.insert_window_in_column(4, 1).unwrap(); // Column 1, index 1

        assert_eq!(ws.find_window_location(1), Some((0, 0)));
        assert_eq!(ws.find_window_location(2), Some((1, 0)));
        assert_eq!(ws.find_window_location(3), Some((0, 1)));
        assert_eq!(ws.find_window_location(4), Some((1, 1)));
        assert_eq!(ws.find_window_location(999), None);
    }

    #[test]
    fn test_window_count() {
        let mut ws = Workspace::new();
        assert_eq!(ws.window_count(), 0);

        ws.insert_window(1, Some(400)).unwrap();
        assert_eq!(ws.window_count(), 1);

        ws.insert_window(2, Some(400)).unwrap();
        assert_eq!(ws.window_count(), 2);

        ws.insert_window_in_column(3, 0).unwrap();
        ws.insert_window_in_column(4, 0).unwrap();
        assert_eq!(ws.window_count(), 4);

        ws.remove_window(2).unwrap();
        assert_eq!(ws.window_count(), 3);
    }

    #[test]
    fn test_column_safe_access() {
        let mut ws = Workspace::new();
        assert!(ws.column(0).is_none());

        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(500)).unwrap();

        assert!(ws.column(0).is_some());
        assert_eq!(ws.column(0).unwrap().width(), 400);
        assert!(ws.column(1).is_some());
        assert_eq!(ws.column(1).unwrap().width(), 500);
        assert!(ws.column(2).is_none());
        assert!(ws.column(100).is_none());
    }

    #[test]
    fn test_single_column_move_operations() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400)).unwrap();

        // Move operations on single column should do nothing
        ws.move_column_left();
        assert_eq!(ws.focused_column_index(), 0);
        assert_eq!(ws.focused_window(), Some(1));

        ws.move_column_right();
        assert_eq!(ws.focused_column_index(), 0);
        assert_eq!(ws.focused_window(), Some(1));
    }

    #[test]
    fn test_resize_on_empty_workspace() {
        let mut ws = Workspace::new();

        // Resize on empty workspace should do nothing without panic
        ws.resize_focused_column(100);
        ws.resize_focused_column(-100);
        ws.resize_focused_column(i32::MAX);
        ws.resize_focused_column(i32::MIN);

        assert!(ws.is_empty());
    }

    #[test]
    fn test_invariants_after_complex_sequence() {
        let mut ws = Workspace::with_gaps(10, 10);

        // Insert several windows
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(300)).unwrap();
        ws.insert_window(3, Some(500)).unwrap();
        ws.insert_window_in_column(4, 1).unwrap();
        ws.insert_window_in_column(5, 1).unwrap();

        // Complex sequence of operations
        ws.focus_left();
        ws.focus_down();
        ws.move_column_right();
        ws.resize_focused_column(100);
        ws.focus_right();
        ws.focus_up();
        ws.remove_window(4).unwrap();
        ws.focus_left();
        ws.move_column_left();

        // Verify invariants still hold
        assert!(ws.focused_column < ws.columns.len());
        assert!(ws.focused_window_in_column < ws.columns[ws.focused_column].len());

        // No duplicate windows
        let mut all_windows: Vec<WindowId> = ws.columns
            .iter()
            .flat_map(|c| c.windows().iter().copied())
            .collect();
        all_windows.sort();
        let len_before = all_windows.len();
        all_windows.dedup();
        assert_eq!(all_windows.len(), len_before, "Duplicate windows found");
    }

    #[test]
    fn test_column_partial_eq() {
        let col1 = Column::new(1, 400);
        let col2 = Column::new(1, 400);
        let col3 = Column::new(2, 400);
        let col4 = Column::new(1, 500);

        assert_eq!(col1, col2);
        assert_ne!(col1, col3); // Different window
        assert_ne!(col1, col4); // Different width
    }

    // ========================================================================
    // Animation Tests
    // ========================================================================

    #[test]
    fn test_easing_linear() {
        assert!((Easing::Linear.apply(0.0) - 0.0).abs() < f64::EPSILON);
        assert!((Easing::Linear.apply(0.25) - 0.25).abs() < f64::EPSILON);
        assert!((Easing::Linear.apply(0.5) - 0.5).abs() < f64::EPSILON);
        assert!((Easing::Linear.apply(0.75) - 0.75).abs() < f64::EPSILON);
        assert!((Easing::Linear.apply(1.0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_easing_ease_out() {
        // EaseOut starts fast, ends slow (cubic)
        let ease_out = Easing::EaseOut;

        // At t=0, should be 0
        assert!((ease_out.apply(0.0) - 0.0).abs() < f64::EPSILON);
        // At t=1, should be 1
        assert!((ease_out.apply(1.0) - 1.0).abs() < f64::EPSILON);

        // EaseOut should be ahead of linear in the middle
        assert!(ease_out.apply(0.5) > 0.5);

        // Verify cubic formula: 1 - (1 - t)^3
        let t: f64 = 0.5;
        let expected = 1.0 - (1.0 - t).powi(3);
        assert!((ease_out.apply(t) - expected).abs() < f64::EPSILON);
    }

    #[test]
    fn test_easing_ease_in() {
        // EaseIn starts slow, ends fast (cubic)
        let ease_in = Easing::EaseIn;

        // At t=0, should be 0
        assert!((ease_in.apply(0.0) - 0.0).abs() < f64::EPSILON);
        // At t=1, should be 1
        assert!((ease_in.apply(1.0) - 1.0).abs() < f64::EPSILON);

        // EaseIn should be behind linear in the middle
        assert!(ease_in.apply(0.5) < 0.5);

        // Verify cubic formula: t^3
        let t: f64 = 0.5;
        let expected = t.powi(3);
        assert!((ease_in.apply(t) - expected).abs() < f64::EPSILON);
    }

    #[test]
    fn test_easing_ease_in_out() {
        let ease_in_out = Easing::EaseInOut;

        // At t=0, should be 0
        assert!((ease_in_out.apply(0.0) - 0.0).abs() < f64::EPSILON);
        // At t=1, should be 1
        assert!((ease_in_out.apply(1.0) - 1.0).abs() < f64::EPSILON);
        // At t=0.5, should be 0.5 (inflection point)
        assert!((ease_in_out.apply(0.5) - 0.5).abs() < f64::EPSILON);

        // First half should be behind linear
        assert!(ease_in_out.apply(0.25) < 0.25);
        // Second half should be ahead of linear
        assert!(ease_in_out.apply(0.75) > 0.75);
    }

    #[test]
    fn test_easing_clamps_input() {
        // Values outside [0, 1] should be clamped
        assert!((Easing::Linear.apply(-0.5) - 0.0).abs() < f64::EPSILON);
        assert!((Easing::Linear.apply(1.5) - 1.0).abs() < f64::EPSILON);
        assert!((Easing::EaseOut.apply(-1.0) - 0.0).abs() < f64::EPSILON);
        assert!((Easing::EaseOut.apply(2.0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_easing_default_is_ease_out() {
        assert_eq!(Easing::default(), Easing::EaseOut);
    }

    #[test]
    fn test_scroll_animation_new() {
        let anim = ScrollAnimation::new(0.0, 100.0, 200, Easing::Linear);

        assert!((anim.start_offset - 0.0).abs() < f64::EPSILON);
        assert!((anim.target_offset - 100.0).abs() < f64::EPSILON);
        assert_eq!(anim.duration_ms, 200);
        assert_eq!(anim.elapsed_ms, 0);
        assert_eq!(anim.easing, Easing::Linear);
    }

    #[test]
    fn test_scroll_animation_with_defaults() {
        let anim = ScrollAnimation::with_defaults(50.0, 150.0);

        assert!((anim.start_offset - 50.0).abs() < f64::EPSILON);
        assert!((anim.target_offset - 150.0).abs() < f64::EPSILON);
        assert_eq!(anim.duration_ms, DEFAULT_ANIMATION_DURATION_MS);
        assert_eq!(anim.easing, Easing::default());
    }

    #[test]
    fn test_scroll_animation_progress() {
        let mut anim = ScrollAnimation::new(0.0, 100.0, 100, Easing::Linear);

        assert!((anim.progress() - 0.0).abs() < f64::EPSILON);

        anim.elapsed_ms = 50;
        assert!((anim.progress() - 0.5).abs() < f64::EPSILON);

        anim.elapsed_ms = 100;
        assert!((anim.progress() - 1.0).abs() < f64::EPSILON);

        // Over time should clamp to 1.0
        anim.elapsed_ms = 150;
        assert!((anim.progress() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scroll_animation_progress_zero_duration() {
        let anim = ScrollAnimation::new(0.0, 100.0, 0, Easing::Linear);
        // Zero duration should return 1.0 progress immediately
        assert!((anim.progress() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scroll_animation_current_offset_linear() {
        let mut anim = ScrollAnimation::new(0.0, 100.0, 100, Easing::Linear);

        // At start
        assert!((anim.current_offset() - 0.0).abs() < f64::EPSILON);

        // At midpoint
        anim.elapsed_ms = 50;
        assert!((anim.current_offset() - 50.0).abs() < f64::EPSILON);

        // At end
        anim.elapsed_ms = 100;
        assert!((anim.current_offset() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scroll_animation_current_offset_eased() {
        let mut anim = ScrollAnimation::new(0.0, 100.0, 100, Easing::EaseOut);

        // At midpoint with ease out, should be ahead of 50
        anim.elapsed_ms = 50;
        assert!(anim.current_offset() > 50.0);
    }

    #[test]
    fn test_scroll_animation_negative_direction() {
        let mut anim = ScrollAnimation::new(100.0, 0.0, 100, Easing::Linear);

        // At start
        assert!((anim.current_offset() - 100.0).abs() < f64::EPSILON);

        // At midpoint - should be halfway back to 0
        anim.elapsed_ms = 50;
        assert!((anim.current_offset() - 50.0).abs() < f64::EPSILON);

        // At end
        anim.elapsed_ms = 100;
        assert!((anim.current_offset() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scroll_animation_is_complete() {
        let mut anim = ScrollAnimation::new(0.0, 100.0, 100, Easing::Linear);

        assert!(!anim.is_complete());

        anim.elapsed_ms = 50;
        assert!(!anim.is_complete());

        anim.elapsed_ms = 100;
        assert!(anim.is_complete());

        anim.elapsed_ms = 150;
        assert!(anim.is_complete());
    }

    #[test]
    fn test_scroll_animation_tick() {
        let mut anim = ScrollAnimation::new(0.0, 100.0, 100, Easing::Linear);

        // Tick returns true while running
        assert!(anim.tick(25));
        assert_eq!(anim.elapsed_ms, 25);

        assert!(anim.tick(25));
        assert_eq!(anim.elapsed_ms, 50);

        assert!(anim.tick(25));
        assert_eq!(anim.elapsed_ms, 75);

        // Final tick completes
        assert!(!anim.tick(25));
        assert_eq!(anim.elapsed_ms, 100);

        // Further ticks still return false
        assert!(!anim.tick(50));
        assert_eq!(anim.elapsed_ms, 150);
    }

    #[test]
    fn test_scroll_animation_tick_saturating() {
        let mut anim = ScrollAnimation::new(0.0, 100.0, 100, Easing::Linear);

        // Large tick value should not overflow
        anim.elapsed_ms = u64::MAX - 10;
        anim.tick(100);
        assert_eq!(anim.elapsed_ms, u64::MAX); // Saturates at MAX
    }

    #[test]
    fn test_scroll_animation_target() {
        let anim = ScrollAnimation::new(0.0, 456.78, 100, Easing::Linear);
        assert!((anim.target() - 456.78).abs() < f64::EPSILON);
    }

    // ========================================================================
    // Workspace Animation Tests
    // ========================================================================

    #[test]
    fn test_workspace_is_animating() {
        let mut ws = Workspace::with_gaps(10, 10);
        // Add enough windows to have scrollable content
        for i in 1..=5 {
            ws.insert_window(i, Some(400)).unwrap();
        }
        // Total: 10 + (5*400) + (4*10) + 10 = 2060

        assert!(!ws.is_animating());

        // Viewport 500 means max_scroll = 2060 - 500 = 1560
        ws.start_scroll_animation(100.0, 500, None, None);
        assert!(ws.is_animating());

        // Complete the animation
        ws.tick_animation(300);
        assert!(!ws.is_animating());
    }

    #[test]
    fn test_workspace_effective_scroll_offset() {
        let mut ws = Workspace::with_gaps(10, 10);
        // Add enough windows to have scrollable content
        for i in 1..=5 {
            ws.insert_window(i, Some(400)).unwrap();
        }
        // Total: 10 + (5*400) + (4*10) + 10 = 2060

        // Initially no animation
        assert!((ws.effective_scroll_offset() - 0.0).abs() < 1.0);

        // Start animation to 200 with viewport 500 (max_scroll = 1560)
        ws.start_scroll_animation(200.0, 500, Some(100), Some(Easing::Linear));
        assert!(ws.is_animating());

        // At start, should be near 0
        assert!(ws.effective_scroll_offset() < 10.0);

        // Tick halfway
        ws.tick_animation(50);
        // Should be around 100 (halfway)
        assert!(ws.effective_scroll_offset() > 80.0 && ws.effective_scroll_offset() < 120.0);
    }

    #[test]
    fn test_workspace_start_scroll_animation_clamps_target() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)).unwrap();

        // Total width: 10 + 400 + 10 = 420
        // Max scroll with 1000 viewport = max(420 - 1000, 0) = 0

        ws.start_scroll_animation(500.0, 1000, None, None);

        // Target should be clamped to 0 (can't scroll past content)
        assert!(!ws.is_animating()); // Already at target (both clamped to 0)
    }

    #[test]
    fn test_workspace_tick_animation() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();

        ws.start_scroll_animation(200.0, 500, Some(100), Some(Easing::Linear));

        // Should be animating
        assert!(ws.tick_animation(30));
        assert!(ws.is_animating());

        // Tick to completion
        assert!(!ws.tick_animation(100));
        assert!(!ws.is_animating());

        // After animation, scroll_offset should be at target
        assert!((ws.effective_scroll_offset() - 200.0).abs() < 1.0);
    }

    #[test]
    fn test_workspace_stop_animation() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();

        ws.start_scroll_animation(200.0, 500, Some(100), Some(Easing::Linear));
        ws.tick_animation(50);

        // Stop should snap to target
        ws.stop_animation();
        assert!(!ws.is_animating());
        assert!((ws.effective_scroll_offset() - 200.0).abs() < 1.0);
    }

    #[test]
    fn test_workspace_cancel_animation() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();

        ws.start_scroll_animation(200.0, 500, Some(100), Some(Easing::Linear));
        ws.tick_animation(50);

        let current = ws.effective_scroll_offset();
        // Should be around 100 (halfway)

        // Cancel should stay at current position
        ws.cancel_animation();
        assert!(!ws.is_animating());
        assert!((ws.effective_scroll_offset() - current).abs() < 1.0);
    }

    #[test]
    fn test_workspace_animation_no_effect_when_at_target() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)).unwrap();

        // Already at offset 0, trying to animate to 0 shouldn't start animation
        ws.start_scroll_animation(0.0, 1000, None, None);
        assert!(!ws.is_animating());
    }

    #[test]
    fn test_workspace_animation_interrupt() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();

        // Start animation to 200
        ws.start_scroll_animation(200.0, 500, Some(100), Some(Easing::Linear));
        ws.tick_animation(50);

        // Interrupt with new animation to 300
        ws.start_scroll_animation(300.0, 500, Some(100), Some(Easing::Linear));

        // New animation should start from current position (~100)
        assert!(ws.is_animating());

        // Complete new animation
        ws.tick_animation(150);
        assert!((ws.effective_scroll_offset() - 300.0).abs() < 1.0);
    }

    #[test]
    fn test_compute_placements_animated() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)).unwrap();
        ws.insert_window(2, Some(400)).unwrap();

        let viewport = Rect::new(0, 0, 500, 600);

        // Without animation
        let placements1 = ws.compute_placements_animated(viewport);
        assert_eq!(placements1.len(), 2);

        // Start animation that shifts viewport
        ws.start_scroll_animation(200.0, 500, Some(100), Some(Easing::Linear));
        ws.tick_animation(100); // Complete

        let placements2 = ws.compute_placements_animated(viewport);
        assert_eq!(placements2.len(), 2);

        // Window positions should be shifted left (viewport scrolled right)
        assert!(placements2[0].rect.x < placements1[0].rect.x);
    }

    #[test]
    fn test_ensure_focused_visible_animated_center_mode() {
        let mut ws = Workspace::with_gaps(10, 10);
        // Add enough windows to require scrolling
        for i in 1..=5 {
            ws.insert_window(i, Some(400)).unwrap();
        }
        // Total: 10 + (5*400) + (4*10) + 10 = 2060

        // Focus is at column 4 (last inserted), scroll to make it visible first
        ws.ensure_focused_visible(500);

        // Now focus first column which is off-screen
        ws.focus_left();
        ws.focus_left();
        ws.focus_left();
        ws.focus_left();

        // This should trigger an animation because column 0 is now off-screen
        ws.ensure_focused_visible_animated(500);

        // Should start an animation to scroll back to column 0
        assert!(ws.is_animating());
    }
}
