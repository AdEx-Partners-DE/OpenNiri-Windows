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

/// Preferred size of a floating window without an explicit rect.
const DEFAULT_FLOATING_SIZE: (i32, i32) = (800, 600);
/// Total horizontal/vertical margin kept free around a default floating window.
const FLOATING_VIEWPORT_MARGIN: i32 = 40;

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
}

/// Default rect for a window that starts floating without explicit geometry:
/// 800x600 (shrunk to leave a margin inside small viewports), centered in `viewport`.
pub fn default_floating_rect(viewport: Rect) -> Rect {
    let (preferred_w, preferred_h) = DEFAULT_FLOATING_SIZE;
    let width = preferred_w
        .min(viewport.width.saturating_sub(FLOATING_VIEWPORT_MARGIN))
        .max(0);
    let height = preferred_h
        .min(viewport.height.saturating_sub(FLOATING_VIEWPORT_MARGIN))
        .max(0);
    Rect::new(
        viewport
            .x
            .saturating_add(viewport.width.saturating_sub(width) / 2),
        viewport
            .y
            .saturating_add(viewport.height.saturating_sub(height) / 2),
        width,
        height,
    )
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
#[derive(Debug, Clone, PartialEq)]
struct ScrollAnimation {
    start_offset: f64,
    target_offset: f64,
    duration_ms: u64,
    elapsed_ms: u64,
    easing: Easing,
}

impl ScrollAnimation {
    fn new(start: f64, target: f64, duration_ms: u64, easing: Easing) -> Self {
        Self {
            start_offset: start,
            target_offset: target,
            duration_ms,
            elapsed_ms: 0,
            easing,
        }
    }

    fn is_complete(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }

    /// Current progress (0.0 to 1.0).
    fn progress(&self) -> f64 {
        if self.duration_ms == 0 {
            return 1.0;
        }
        (self.elapsed_ms as f64 / self.duration_ms as f64).clamp(0.0, 1.0)
    }

    fn current_offset(&self) -> f64 {
        let eased_progress = self.easing.apply(self.progress());
        self.start_offset + (self.target_offset - self.start_offset) * eased_progress
    }

    /// Advance by `delta_ms`. Returns true while the animation is still running.
    fn tick(&mut self, delta_ms: u64) -> bool {
        self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
        !self.is_complete()
    }
}

/// Horizontal view position over the strip: either resting at an offset or
/// animating towards one. Every write replaces the whole state, so a direct
/// scroll can never be overwritten later by a stale animation.
#[derive(Debug, Clone, PartialEq)]
enum View {
    Static(f64),
    Animating(ScrollAnimation),
}

impl View {
    /// Offset currently on screen.
    fn current(&self) -> f64 {
        match self {
            View::Static(offset) => *offset,
            View::Animating(anim) => anim.current_offset(),
        }
    }

    /// Offset the view will rest at once any animation finishes.
    fn target(&self) -> f64 {
        match self {
            View::Static(offset) => *offset,
            View::Animating(anim) => anim.target_offset,
        }
    }
}

impl Default for View {
    fn default() -> Self {
        View::Static(0.0)
    }
}

/// Serializes the view as the plain `scroll_offset` number used by earlier
/// versions (the resting target; animations are transient).
mod view_serde {
    use super::View;
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(view: &View, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(view.target())
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<View, D::Error> {
        let offset = f64::deserialize(deserializer)?;
        Ok(View::Static(if offset.is_finite() {
            offset.max(0.0)
        } else {
            0.0
        }))
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
    /// Remembered focused row; `< windows.len()` whenever the column is non-empty.
    #[serde(default)]
    focused_row: usize,
}

impl Column {
    /// Create a new column with a single window.
    /// Width is clamped to MIN_COLUMN_WIDTH (100px) minimum.
    pub fn new(window_id: WindowId, width: i32) -> Self {
        Self {
            width: width.max(MIN_COLUMN_WIDTH),
            windows: vec![window_id],
            focused_row: 0,
        }
    }

    /// Create an empty column with specified width.
    /// Width is clamped to MIN_COLUMN_WIDTH (100px) minimum.
    pub fn empty(width: i32) -> Self {
        Self {
            width: width.max(MIN_COLUMN_WIDTH),
            windows: Vec::new(),
            focused_row: 0,
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
    ///
    /// The remembered focused row stays on the same window when a window above
    /// it is removed; when the focused window itself is removed, focus moves to
    /// the next window (or the previous one at the bottom of the stack).
    pub fn remove_window(&mut self, window_id: WindowId) -> Option<usize> {
        let pos = self.windows.iter().position(|&w| w == window_id)?;
        self.windows.remove(pos);
        if pos < self.focused_row {
            self.focused_row -= 1;
        }
        self.focused_row = self.focused_row.min(self.windows.len().saturating_sub(1));
        Some(pos)
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

/// Whether a newly inserted tiled window takes keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InsertFocus {
    /// The new window becomes the focused window.
    #[default]
    Take,
    /// Focus stays where it is (the first window in an empty workspace still gets it).
    Keep,
}

/// Horizontal focus direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Left,
    Right,
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
///    Otherwise, `focused_column < columns.len()` and every column's remembered
///    row is within that column.
/// 3. **Valid column widths:** All column widths are >= `MIN_COLUMN_WIDTH` (100px).
/// 4. **Fullscreen:** `fullscreen_window`, if set, is the focused tiled window and
///    is not minimized. Any focus change to another window leaves fullscreen.
/// 5. **Strip geometry:** Columns without a visible (non-minimized) window take no
///    width. Offsets written through `scroll_by`/`set_scroll_offset`/`ensure_*`
///    are clamped to `0..=max(total_width() - viewport_width, 0)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Columns in the workspace, ordered left to right.
    columns: Vec<Column>,
    /// Index of the currently focused column.
    focused_column: usize,
    /// View position over the strip (x of the viewport's left edge).
    #[serde(rename = "scroll_offset", with = "view_serde")]
    view: View,
    /// Gap between columns in pixels (always >= 0).
    gap: i32,
    /// Gap at the edges of the viewport (always >= 0).
    outer_gap: i32,
    /// Default width for new columns (always >= MIN_COLUMN_WIDTH).
    default_column_width: i32,
    /// Centering mode for focus changes.
    centering_mode: CenteringMode,
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
            view: View::default(),
            gap: DEFAULT_GAP,
            outer_gap: DEFAULT_OUTER_GAP,
            default_column_width: DEFAULT_COLUMN_WIDTH,
            centering_mode: CenteringMode::default(),
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
        let before = self.floating_windows.len();
        self.floating_windows.retain(|f| f.id != window_id);
        self.floating_windows.len() != before
    }

    /// Record a new position/size for a floating window (e.g. after the user
    /// moved or resized it), so later placements keep it there.
    ///
    /// Returns false if `window_id` is not a floating window of this workspace.
    pub fn update_floating(&mut self, window_id: WindowId, rect: Rect) -> bool {
        match self.floating_windows.iter_mut().find(|f| f.id == window_id) {
            Some(floating) => {
                floating.rect = rect;
                true
            }
            None => false,
        }
    }

    /// Get all floating windows.
    pub fn floating_windows(&self) -> &[FloatingWindow] {
        &self.floating_windows
    }

    /// Gap and outer gap, defensively clamped to >= 0 (fields may come from
    /// deserialized state).
    fn gaps(&self) -> (i32, i32) {
        (self.gap.max(0), self.outer_gap.max(0))
    }

    /// Whether a column has at least one non-minimized window. Columns without
    /// one take no space on the strip.
    fn column_is_visible(&self, column: &Column) -> bool {
        column
            .windows
            .iter()
            .any(|w| !self.minimized_windows.contains(w))
    }

    /// First non-minimized row of column `col`, searching down from `from`
    /// (inclusive) and then up.
    fn nearest_visible(&self, col: usize, from: usize) -> Option<usize> {
        let windows = &self.columns.get(col)?.windows;
        let visible = |i: &usize| !self.minimized_windows.contains(&windows[*i]);
        (from..windows.len())
            .find(visible)
            .or_else(|| (0..from.min(windows.len())).rev().find(visible))
    }

    /// Get the total width of the strip (visible column widths + gaps).
    pub fn total_width(&self) -> i32 {
        let (gap, outer_gap) = self.gaps();
        let (count, widths) = self
            .columns
            .iter()
            .filter(|c| self.column_is_visible(c))
            .fold((0i32, 0i32), |(n, sum), c| {
                (n.saturating_add(1), sum.saturating_add(c.width))
            });
        if count == 0 {
            return 0;
        }
        widths
            .saturating_add(gap.saturating_mul(count - 1))
            .saturating_add(outer_gap.saturating_mul(2))
    }

    /// Largest valid scroll offset for a viewport of `viewport_width`.
    fn max_scroll(&self, viewport_width: i32) -> f64 {
        f64::from(self.total_width().saturating_sub(viewport_width).max(0))
    }

    /// Clamp a finite offset into the valid scroll range.
    fn clamp_offset(&self, offset: f64, viewport_width: i32) -> f64 {
        offset.clamp(0.0, self.max_scroll(viewport_width))
    }

    /// Commit focus to `(col, row)`. This is the only place tiled focus moves;
    /// fullscreen ends when focus lands on a different window.
    fn set_focus_to(&mut self, col: usize, row: usize) {
        self.focused_column = col;
        self.columns[col].focused_row = row;
        if self.fullscreen_window.is_some() && self.fullscreen_window != self.focused_window() {
            self.fullscreen_window = None;
        }
    }

    /// Insert a new window as a new column to the right of the focused column;
    /// the new window takes focus. Shorthand for
    /// `insert_window_with(window_id, width, InsertFocus::Take)`.
    ///
    /// # Errors
    ///
    /// Returns `LayoutError::DuplicateWindow` if the window ID already exists.
    pub fn insert_window(
        &mut self,
        window_id: WindowId,
        width: Option<i32>,
    ) -> Result<(), LayoutError> {
        self.insert_window_with(window_id, width, InsertFocus::Take)
    }

    /// Insert a new window as a new column to the right of the focused column.
    /// Column width defaults to `default_column_width` and is clamped to
    /// MIN_COLUMN_WIDTH (100px) minimum. `focus` decides whether the new window
    /// takes focus (it always does in an empty workspace).
    ///
    /// # Errors
    ///
    /// Returns `LayoutError::DuplicateWindow` if the window ID already exists.
    pub fn insert_window_with(
        &mut self,
        window_id: WindowId,
        width: Option<i32>,
        focus: InsertFocus,
    ) -> Result<(), LayoutError> {
        if self.contains_window(window_id) {
            return Err(LayoutError::DuplicateWindow(window_id));
        }

        let column_width = width.unwrap_or(self.default_column_width);
        let insert_pos = if self.columns.is_empty() {
            0
        } else {
            self.focused_column + 1
        };
        self.columns
            .insert(insert_pos, Column::new(window_id, column_width));

        if focus == InsertFocus::Take || self.columns.len() == 1 {
            self.set_focus_to(insert_pos, 0);
        }
        Ok(())
    }

    /// Remove a window from the workspace.
    /// If removing the last window from a column, the column is removed.
    /// If removing the last column, the workspace becomes empty and the view resets.
    ///
    /// # Focus Policy
    ///
    /// Every column remembers its focused row (see [`Column::remove_window`]).
    /// When the focused column disappears, focus moves to the column that slid
    /// into its place (or the new last column).
    pub fn remove_window(&mut self, window_id: WindowId) -> Result<(), LayoutError> {
        let (col_idx, _) = self
            .find_window_location(window_id)
            .ok_or(LayoutError::WindowNotFound(window_id))?;

        self.minimized_windows.remove(&window_id);
        self.clear_fullscreen_if_window(window_id);

        let column = &mut self.columns[col_idx];
        column.remove_window(window_id);
        if column.is_empty() {
            self.columns.remove(col_idx);
            if self.columns.is_empty() {
                self.focused_column = 0;
                self.view = View::default();
            } else if self.focused_column >= self.columns.len() {
                self.focused_column = self.columns.len() - 1;
            } else if self.focused_column > col_idx {
                self.focused_column -= 1;
            }
        }
        Ok(())
    }

    /// Move focus to the nearest column on the left that has a visible window.
    pub fn focus_left(&mut self) {
        self.focus_horizontal(Direction::Left);
    }

    /// Move focus to the nearest column on the right that has a visible window.
    pub fn focus_right(&mut self) {
        self.focus_horizontal(Direction::Right);
    }

    /// Move focus to the nearest column in `dir` that has a visible window,
    /// landing on its remembered row (or the nearest visible one). Fully
    /// minimized columns are skipped. Without such a column focus stays in the
    /// current column, stepping off a minimized row if needed. Nothing is
    /// mutated until the target is known.
    fn focus_horizontal(&mut self, dir: Direction) {
        let current = self.focused_column;
        let is_target = |c: &usize| self.column_is_visible(&self.columns[*c]);
        let target = match dir {
            Direction::Left => (0..current).rev().find(is_target),
            Direction::Right => (current + 1..self.columns.len()).find(is_target),
        }
        .unwrap_or(current);

        let Some(column) = self.columns.get(target) else {
            return;
        };
        if let Some(row) = self.nearest_visible(target, column.focused_row) {
            self.set_focus_to(target, row);
        }
    }

    /// Move focus to the window above in the current column, skipping
    /// minimized windows.
    pub fn focus_up(&mut self) {
        let col = self.focused_column;
        let Some(column) = self.columns.get(col) else {
            return;
        };
        let target = (0..column.focused_row)
            .rev()
            .find(|&r| !self.minimized_windows.contains(&column.windows[r]));
        if let Some(row) = target {
            self.set_focus_to(col, row);
        }
    }

    /// Move focus to the window below in the current column, skipping
    /// minimized windows.
    pub fn focus_down(&mut self) {
        let col = self.focused_column;
        let Some(column) = self.columns.get(col) else {
            return;
        };
        let target = (column.focused_row + 1..column.len())
            .find(|&r| !self.minimized_windows.contains(&column.windows[r]));
        if let Some(row) = target {
            self.set_focus_to(col, row);
        }
    }

    /// Get the currently focused window ID.
    pub fn focused_window(&self) -> Option<WindowId> {
        self.columns
            .get(self.focused_column)
            .and_then(|col| col.get(col.focused_row))
    }

    /// Get the focused window ID, but only if it is not minimized.
    /// Falls back to the nearest non-minimized window in the focused column.
    /// Returns `None` if the workspace is empty or every window is minimized.
    pub fn focused_visible_window(&self) -> Option<WindowId> {
        let col = self.columns.get(self.focused_column)?;
        let row = self.nearest_visible(self.focused_column, col.focused_row)?;
        col.get(row)
    }

    /// Get the index of the currently focused column.
    pub fn focused_column_index(&self) -> usize {
        self.focused_column
    }

    /// Get the index of the focused window within the focused column.
    pub fn focused_window_index_in_column(&self) -> usize {
        self.columns
            .get(self.focused_column)
            .map_or(0, |col| col.focused_row)
    }

    /// Get the scroll offset currently on screen (mid-animation if one is running).
    pub fn scroll_offset(&self) -> f64 {
        self.view.current()
    }

    /// Get a slice of all columns.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Get a column by index (safe access).
    pub fn column(&self, index: usize) -> Option<&Column> {
        self.columns.get(index)
    }

    /// Find a window's tiled location as `(column_index, window_index_in_column)`.
    fn find_window_location(&self, window_id: WindowId) -> Option<(usize, usize)> {
        self.columns
            .iter()
            .enumerate()
            .find_map(|(col_idx, column)| {
                column
                    .windows
                    .iter()
                    .position(|&w| w == window_id)
                    .map(|win_idx| (col_idx, win_idx))
            })
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

    /// Focus a tiled window by its ID.
    ///
    /// # Errors
    ///
    /// Returns `LayoutError::WindowNotFound` if the window is not tiled in the workspace.
    pub fn focus_window(&mut self, window_id: WindowId) -> Result<(), LayoutError> {
        let (col, row) = self
            .find_window_location(window_id)
            .ok_or(LayoutError::WindowNotFound(window_id))?;
        self.set_focus_to(col, row);
        Ok(())
    }

    /// Strip x-coordinate of a column's left edge. Columns without a visible
    /// window before it take no space.
    fn column_x(&self, column_index: usize) -> i32 {
        let (gap, outer_gap) = self.gaps();
        self.columns[..column_index.min(self.columns.len())]
            .iter()
            .filter(|c| self.column_is_visible(c))
            .fold(outer_gap, |x, c| {
                x.saturating_add(c.width).saturating_add(gap)
            })
    }

    /// Scroll offset that brings the focused column into view according to the
    /// centering mode. Returns the current target when no scroll is needed or
    /// the focused column has no visible window. The result is not clamped.
    fn focus_target_offset(&self, viewport_width: i32) -> f64 {
        let current = self.view.target();
        let Some(column) = self
            .columns
            .get(self.focused_column)
            .filter(|c| self.column_is_visible(c))
        else {
            return current;
        };

        let (_, outer_gap) = self.gaps();
        let col_x = self.column_x(self.focused_column);
        let col_width = column.width;

        // A column that cannot fit is left-aligned in every mode (like niri):
        // centering would cut off its left edge, and fitting it from either side
        // would flip between two targets on every focus.
        if col_width.saturating_add(outer_gap.saturating_mul(2)) >= viewport_width {
            return f64::from(col_x.saturating_sub(outer_gap));
        }

        match self.centering_mode {
            CenteringMode::Center => {
                let col_center = col_x.saturating_add(col_width / 2);
                f64::from(col_center.saturating_sub(viewport_width / 2))
            }
            CenteringMode::JustInView => {
                let viewport_left = current.round() as i32;
                let viewport_right = viewport_left.saturating_add(viewport_width);
                let col_right = col_x.saturating_add(col_width);
                if col_x < viewport_left {
                    f64::from(col_x.saturating_sub(outer_gap))
                } else if col_right > viewport_right {
                    f64::from(
                        col_right
                            .saturating_add(outer_gap)
                            .saturating_sub(viewport_width),
                    )
                } else {
                    current
                }
            }
        }
    }

    /// Ensure the focused column is visible in the viewport, jumping there
    /// immediately (any running animation is replaced). The offset is clamped
    /// to the valid range.
    pub fn ensure_focused_visible(&mut self, viewport_width: i32) {
        let target = self.focus_target_offset(viewport_width);
        self.view = View::Static(self.clamp_offset(target, viewport_width));
    }

    /// Ensure the focused column is visible, animating the scroll. An animation
    /// already heading to the same target keeps running.
    pub fn ensure_focused_visible_animated(&mut self, viewport_width: i32) {
        let target = self.focus_target_offset(viewport_width);
        self.animate_to(
            target,
            viewport_width,
            DEFAULT_ANIMATION_DURATION_MS,
            Easing::default(),
        );
    }

    /// Compute placements for all windows at the offset currently on screen
    /// (following any running scroll animation).
    ///
    /// Tiled windows come first in column order, then floating windows
    /// (`column_index == usize::MAX`). Minimized windows are omitted. While a
    /// window is fullscreen it covers the viewport and every other window is
    /// reported off-screen with an empty rect.
    pub fn compute_placements(&self, viewport: Rect) -> Vec<WindowPlacement> {
        let mut placements = self.strip_placements(viewport);
        placements.extend(
            self.floating_windows
                .iter()
                .map(|floating| WindowPlacement {
                    window_id: floating.id,
                    rect: floating.rect,
                    visibility: Visibility::Visible,
                    column_index: usize::MAX, // Sentinel for floating windows
                }),
        );

        if let Some(fs_wid) = self.fullscreen_window {
            for placement in &mut placements {
                if placement.window_id == fs_wid {
                    placement.rect = viewport;
                    placement.visibility = Visibility::Visible;
                } else {
                    placement.rect = Rect::new(0, 0, 0, 0);
                    placement.visibility = Visibility::OffScreenLeft;
                }
            }
        }
        placements
    }

    /// Placements of the tiled (non-minimized) windows on the strip.
    fn strip_placements(&self, viewport: Rect) -> Vec<WindowPlacement> {
        let mut placements = Vec::new();
        let (gap, outer_gap) = self.gaps();

        // Use rounding instead of truncation to prevent sub-pixel jitter
        let viewport_left = self.view.current().round() as i32;
        let viewport_right = viewport_left.saturating_add(viewport.width);

        // Clamp usable_height to >= 0 to handle tight viewports
        let usable_height = viewport
            .height
            .saturating_sub(outer_gap.saturating_mul(2))
            .max(0);
        let top = viewport.y.saturating_add(outer_gap);
        let bottom = viewport
            .y
            .saturating_add(viewport.height)
            .saturating_sub(outer_gap);

        let mut current_x = outer_gap;

        for (col_idx, column) in self.columns.iter().enumerate() {
            let visible_windows: Vec<WindowId> = column
                .windows()
                .iter()
                .copied()
                .filter(|w| !self.minimized_windows.contains(w))
                .collect();
            // Fully minimized columns take no space on the strip.
            if visible_windows.is_empty() {
                continue;
            }

            let col_strip_x = current_x;
            let col_strip_right = col_strip_x.saturating_add(column.width);
            let col_screen_x = col_strip_x
                .saturating_sub(viewport_left)
                .saturating_add(viewport.x);

            let visibility = if col_strip_right <= viewport_left {
                Visibility::OffScreenLeft
            } else if col_strip_x >= viewport_right {
                Visibility::OffScreenRight
            } else {
                Visibility::Visible
            };

            // Equal split for stacked windows; the last one absorbs rounding.
            let window_count = i32::try_from(visible_windows.len()).unwrap_or(i32::MAX);
            let window_gaps = gap.saturating_mul(window_count - 1);
            let window_height = usable_height.saturating_sub(window_gaps).max(0) / window_count;

            let mut current_y = top;
            for (win_idx, &window_id) in visible_windows.iter().enumerate() {
                let height = if win_idx == visible_windows.len() - 1 {
                    bottom.saturating_sub(current_y).max(0)
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
            column.set_width(column.width.saturating_add(delta));
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

    /// Scroll the viewport by a pixel delta from the offset currently on screen.
    /// Cancels any running animation; the result is clamped to the valid range.
    ///
    /// Non-finite deltas (NaN, Infinity) are ignored.
    pub fn scroll_by(&mut self, delta: f64, viewport_width: i32) {
        if !delta.is_finite() {
            return;
        }
        let offset = self.view.current() + delta;
        self.view = View::Static(self.clamp_offset(offset, viewport_width));
    }

    /// Set the scroll offset directly, e.g. when restoring saved state.
    /// Cancels any running animation and clamps to the valid range for the
    /// current windows. Non-finite offsets are ignored.
    pub fn set_scroll_offset(&mut self, offset: f64, viewport_width: i32) {
        if !offset.is_finite() {
            return;
        }
        self.view = View::Static(self.clamp_offset(offset, viewport_width));
    }

    // ========================================================================
    // Animation Methods
    // ========================================================================

    /// Check if a scroll animation is currently active.
    pub fn is_animating(&self) -> bool {
        matches!(self.view, View::Animating(_))
    }

    /// Animate from the offset currently on screen to `target` (clamped).
    /// An animation already heading to the same target keeps running; a
    /// zero-duration or zero-distance move completes immediately.
    fn animate_to(&mut self, target: f64, viewport_width: i32, duration_ms: u64, easing: Easing) {
        if !target.is_finite() {
            return;
        }
        let target = self.clamp_offset(target, viewport_width);
        if self.is_animating() && (self.view.target() - target).abs() < 0.5 {
            return;
        }
        let start = self.view.current();
        self.view = if duration_ms == 0 || (start - target).abs() < 0.5 {
            View::Static(target)
        } else {
            View::Animating(ScrollAnimation::new(start, target, duration_ms, easing))
        };
    }

    /// Advance the active animation by the given delta time in milliseconds.
    /// Returns true if an animation is still active, false if complete or no animation.
    pub fn tick_animation(&mut self, delta_ms: u64) -> bool {
        let View::Animating(anim) = &mut self.view else {
            return false;
        };
        if anim.tick(delta_ms) {
            return true;
        }
        self.view = View::Static(anim.target_offset);
        false
    }

    /// Cancel the current animation and stay at the current position.
    pub fn cancel_animation(&mut self) {
        self.view = View::Static(self.view.current());
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
        if self.find_window_location(window_id).is_none() {
            return false;
        }
        self.clear_fullscreen_if_window(window_id);
        self.minimized_windows.insert(window_id)
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

    // ========================================================================
    // Fullscreen Methods
    // ========================================================================

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

    /// Toggle fullscreen mode for the focused window. If the focused row is
    /// minimized, focus first moves to the nearest visible window in the column.
    /// Returns true if entering fullscreen, false if exiting (or nothing to show).
    pub fn toggle_fullscreen(&mut self) -> bool {
        if self.fullscreen_window.take().is_some() {
            return false;
        }
        let col = self.focused_column;
        let Some(row) = self
            .columns
            .get(col)
            .and_then(|c| self.nearest_visible(col, c.focused_row))
        else {
            return false;
        };
        self.set_focus_to(col, row);
        self.fullscreen_window = self.focused_window();
        true
    }

    // ========================================================================
    // Toggle Floating
    // ========================================================================

    /// Move the focused tiled window to floating, centered with
    /// [`default_floating_rect`]. Minimized windows are not floated.
    /// Returns the window ID that was toggled, if any.
    pub fn toggle_floating(&mut self, viewport: Rect) -> Option<WindowId> {
        let wid = self.focused_window()?;
        if self.is_minimized(wid) {
            return None;
        }
        // Clears fullscreen if `wid` was the fullscreen window.
        self.remove_window(wid).ok()?;
        self.floating_windows.push(FloatingWindow {
            id: wid,
            rect: default_floating_rect(viewport),
        });
        Some(wid)
    }

    /// Move a floating window back to the tiling layout.
    /// Returns true if the window was unfloated.
    pub fn unfloat_window(&mut self, window_id: WindowId) -> bool {
        if !self.remove_floating(window_id) {
            return false;
        }
        // Cannot fail: the id was just removed and ids are unique.
        let _ = self.insert_window(window_id, None);
        true
    }

    // ========================================================================
    // Column Width Presets
    // ========================================================================

    /// Set the focused column's width as a fraction of the viewport width.
    /// Fraction is clamped to 0.1..=1.0. Gaps are accounted for so that N
    /// columns of `1/N` (plus gaps) fill the viewport exactly up to rounding
    /// down: `width = (usable + gap) * fraction - gap`, `usable = viewport - 2 * outer_gap`.
    pub fn set_focused_column_width_fraction(&mut self, fraction: f64, viewport_width: i32) {
        if !fraction.is_finite() {
            return;
        }
        let fraction = fraction.clamp(0.1, 1.0);
        let (gap, outer_gap) = self.gaps();
        let usable_width = viewport_width.saturating_sub(outer_gap.saturating_mul(2));
        let share = (f64::from(usable_width.saturating_add(gap)) * fraction).floor() as i32;

        if let Some(column) = self.columns.get_mut(self.focused_column) {
            column.set_width(share.saturating_sub(gap));
        }
    }

    /// Equalize the widths of all visible columns so they share the viewport
    /// equally. Fully minimized columns take no width and keep their own.
    pub fn equalize_column_widths(&mut self, viewport_width: i32) {
        let visible: Vec<bool> = self
            .columns
            .iter()
            .map(|c| self.column_is_visible(c))
            .collect();
        let count = visible.iter().filter(|&&v| v).count();
        if count == 0 {
            return;
        }

        let (gap, outer_gap) = self.gaps();
        let n = i32::try_from(count).unwrap_or(i32::MAX);
        let total_gaps = gap
            .saturating_mul(n - 1)
            .saturating_add(outer_gap.saturating_mul(2));
        let per_column = viewport_width
            .saturating_sub(total_gaps)
            .max(MIN_COLUMN_WIDTH.saturating_mul(n))
            / n;

        for (col, _) in self.columns.iter_mut().zip(visible).filter(|(_, v)| *v) {
            col.set_width(per_column);
        }
    }
}

// Test-only helpers for direct state manipulation and inspection.
#[cfg(test)]
impl Workspace {
    /// Set focus state directly without validation (test helper).
    pub fn test_set_focus_unchecked(&mut self, column: usize, win: usize) {
        self.focused_column = column;
        if let Some(col) = self.columns.get_mut(column) {
            col.focused_row = win;
        }
    }

    /// Stack a window at the bottom of an existing column.
    fn insert_window_in_column(
        &mut self,
        window_id: WindowId,
        column_index: usize,
    ) -> Result<(), LayoutError> {
        if self.contains_window(window_id) {
            return Err(LayoutError::DuplicateWindow(window_id));
        }
        let max = self.columns.len().saturating_sub(1);
        self.columns
            .get_mut(column_index)
            .ok_or(LayoutError::ColumnOutOfBounds(column_index, max))?
            .add_window(window_id);
        Ok(())
    }

    /// Stop the current animation and snap to its target.
    fn stop_animation(&mut self) {
        self.view = View::Static(self.view.target());
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
    fn rect_clamps_negative_size() {
        let r = Rect::new(10, 20, -100, -50);
        assert_eq!((r.x, r.y, r.width, r.height), (10, 20, 0, 0));
    }

    #[test]
    fn default_floating_rect_centers_and_fits_small_viewports() {
        for (viewport, expected) in [
            (Rect::new(0, 0, 1920, 1080), Rect::new(560, 240, 800, 600)),
            // Smaller than 800x600 + margin: shrink to leave 20px on each side.
            (Rect::new(100, 50, 600, 400), Rect::new(120, 70, 560, 360)),
            (Rect::new(0, 0, 10, 10), Rect::new(5, 5, 0, 0)),
        ] {
            assert_eq!(default_floating_rect(viewport), expected, "{viewport:?}");
        }
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
    fn insert_window_keep_focus_preserves_focus() {
        let mut ws = Workspace::new();
        // Into an empty workspace the first window must still take focus.
        ws.insert_window_with(1, Some(400), InsertFocus::Keep)
            .unwrap();
        assert_eq!(ws.focused_window(), Some(1));

        ws.insert_window(2, Some(400)).unwrap();
        ws.insert_window(3, Some(400)).unwrap();
        ws.insert_window_with(4, Some(400), InsertFocus::Keep)
            .unwrap();
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
                matches!(ws.insert_window_with(id, Some(400), InsertFocus::Keep), Err(LayoutError::DuplicateWindow(d)) if d == id)
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
        // Static view, and (regression #9) a view that is mid-animation.
        for animating in [false, true] {
            let mut ws = ws_with_columns(10, 10, &[400, 400]);
            if animating {
                ws.animate_to(300.0, 500, 100, Easing::Linear);
                ws.tick_animation(50);
                assert!(ws.is_animating());
            } else {
                ws.set_scroll_offset(200.0, 500);
            }
            ws.remove_window(1).unwrap();
            ws.remove_window(2).unwrap();

            assert!(ws.is_empty());
            assert_eq!(ws.focused_window(), None);
            assert_eq!(ws.focused_column_index(), 0);
            assert_eq!(ws.focused_window_index_in_column(), 0);
            assert!(!ws.is_animating(), "animating={animating}");
            assert!(!ws.tick_animation(100), "animating={animating}");
            assert_eq!(ws.scroll_offset(), 0.0, "animating={animating}");
        }
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
    fn removing_focused_column_lands_on_neighbours_remembered_row() {
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.insert_window_in_column(4, 2).unwrap();
        ws.insert_window_in_column(5, 2).unwrap(); // col 2: [3, 4, 5]
        ws.focus_window(5).unwrap(); // col 2 remembers row 2

        ws.focus_window(2).unwrap();
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
    fn failed_horizontal_focus_keeps_focused_row() {
        // Regression #6: the scan used to clamp the row while probing columns and
        // did not restore it on fallback. Layout: the focused stack [10, 11, 12]
        // with row 2 focused, next to a fully minimized single-window column.
        for (stack_col, dir) in [(1, Direction::Left), (0, Direction::Right)] {
            let mut ws = Workspace::new();
            let lone = 1;
            if stack_col == 1 {
                ws.insert_window(lone, Some(400)).unwrap();
            }
            ws.insert_window(10, Some(400)).unwrap();
            for id in [11, 12] {
                ws.insert_window_in_column(id, stack_col).unwrap();
            }
            if stack_col == 0 {
                ws.insert_window_with(lone, Some(400), InsertFocus::Keep)
                    .unwrap();
            }
            ws.mark_minimized(lone);
            ws.focus_window(12).unwrap();

            ws.focus_horizontal(dir);

            assert_eq!(ws.focused_column_index(), stack_col, "{dir:?}");
            assert_eq!(ws.focused_window(), Some(12), "{dir:?}");
        }
    }

    #[test]
    fn columns_remember_their_focused_row() {
        // col 0 = [1, 3, 4] focused on 4; col 1 = [2, 5] focused on 2.
        let mut ws = ws_with_columns(10, 10, &[400, 400]);
        ws.insert_window_in_column(3, 0).unwrap();
        ws.insert_window_in_column(4, 0).unwrap();
        ws.insert_window_in_column(5, 1).unwrap();
        ws.focus_window(4).unwrap();

        ws.focus_right();
        assert_eq!(ws.focused_window(), Some(2));
        ws.focus_left();
        assert_eq!(ws.focused_window(), Some(4));

        // A remembered row that became minimized falls back to a visible one.
        ws.mark_minimized(4);
        ws.focus_right();
        ws.focus_left();
        assert_eq!(ws.focused_window(), Some(3));
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
        assert!(ws.columns.iter().all(|c| c.focused_row < c.len()));

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
    fn column_width_fraction_accounts_for_gaps_and_clamps() {
        // usable = 1920 - 2 * outer_gap(10) = 1900; width = floor((1900 + gap) * f) - gap;
        // fraction clamped to [0.1, 1.0]; non-finite fractions are ignored.
        for (fraction, expected) in [
            (0.5, 945),
            (0.333, 626),
            (2.0, 1900),
            (0.0, 181),
            (f64::NAN, 400),
        ] {
            let mut ws = ws_with_columns(10, 10, &[400]);
            ws.set_focused_column_width_fraction(fraction, 1920);
            assert_eq!(ws.columns()[0].width(), expected, "fraction {fraction}");
        }
    }

    #[test]
    fn n_columns_of_one_nth_fit_the_viewport() {
        // Regression #8: two 0.5 columns used to overflow by the inter-column gap.
        for (gap, outer_gap) in [(10, 10), (16, 4), (0, 0)] {
            for n in 1..=4usize {
                let widths = vec![400; n];
                let mut ws = ws_with_columns(gap, outer_gap, &widths);
                for col in 0..n {
                    ws.test_set_focus_unchecked(col, 0);
                    ws.set_focused_column_width_fraction(1.0 / n as f64, 1920);
                }
                let total = ws.total_width();
                let case = (gap, outer_gap, n);
                assert!(total <= 1920, "{case:?}: {total} overflows");
                assert!(total > 1920 - n as i32, "{case:?}: {total} leaves slack");
            }
        }
        // Halves fit exactly.
        let mut ws = ws_with_columns(10, 10, &[400, 400]);
        for col in 0..2 {
            ws.test_set_focus_unchecked(col, 0);
            ws.set_focused_column_width_fraction(0.5, 1920);
        }
        assert_eq!(ws.total_width(), 1920);
    }

    #[test]
    fn equalize_column_widths_shares_viewport_with_minimum_floor() {
        let mut ws = ws_with_columns(10, 10, &[300, 600, 400]);
        // (1920 - 2 * gap - 2 * outer_gap) / 3 = 626
        ws.equalize_column_widths(1920);
        assert!(ws.columns().iter().all(|c| c.width() == 626));

        ws.equalize_column_widths(200);
        assert!(ws.columns().iter().all(|c| c.width() == MIN_COLUMN_WIDTH));

        // Fully minimized columns are ignored: the two visible ones fill the
        // viewport ((1920 - gap - 2 * outer_gap) / 2 = 945) and window 2 keeps 600.
        let mut ws = ws_with_columns(10, 10, &[300, 600, 400]);
        ws.mark_minimized(2);
        ws.equalize_column_widths(1920);
        let widths: Vec<i32> = ws.columns().iter().map(Column::width).collect();
        assert_eq!(widths, vec![945, 600, 945]);
        assert_eq!(ws.total_width(), 1920);
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
            ws.set_scroll_offset(start, 500);
            ws.ensure_focused_visible(500);
            assert_eq!(ws.scroll_offset(), expected, "column {col}");
        }

        // A column at least as wide as the viewport (incl. both outer gaps) is
        // left-aligned, not centered. Column 1 starts at strip x 420; a trailing
        // 1000px column keeps the result away from the max-scroll clamp.
        // (column width, expected offset): 480 + 20 >= 500 -> 420 - 10;
        // 470 + 20 < 500 -> centered at 420 + 235 - 250.
        for (width, expected) in [(1000, 410.0), (480, 410.0), (470, 405.0)] {
            let mut ws = ws_with_columns(10, 10, &[400, width, 1000]);
            ws.set_centering_mode(CenteringMode::Center);
            ws.test_set_focus_unchecked(1, 0);
            ws.ensure_focused_visible(500);
            assert_eq!(ws.scroll_offset(), expected, "width {width}");
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

        ws.resize_focused_column(500); // 700 wide: cannot fit, left-aligned at 430 - outer 10
        ws.ensure_focused_visible(500);
        assert_eq!(ws.scroll_offset(), 420.0);

        ws.test_set_focus_unchecked(0, 0);
        ws.ensure_focused_visible(500);
        assert_eq!(ws.scroll_offset(), 0.0); // left edge 10 - outer 10
    }

    #[test]
    fn just_in_view_wide_column_target_is_stable() {
        // Regression #2: a column wider than the viewport flipped between its left-
        // and right-aligned offsets on every focus event. Column 1 (strip x 420,
        // width 1000) does not fit a 500px viewport; the target is its left edge.
        let base = || {
            let mut ws = ws_with_columns(10, 10, &[400, 1000, 400]);
            ws.set_centering_mode(CenteringMode::JustInView);
            ws.test_set_focus_unchecked(1, 0);
            ws
        };
        for start in [0.0, 410.0, 600.0, 930.0] {
            let mut snapped = base();
            let mut animated = base();
            snapped.set_scroll_offset(start, 500);
            animated.set_scroll_offset(start, 500);
            for round in 0..3 {
                snapped.ensure_focused_visible(500);
                assert_eq!(
                    snapped.scroll_offset(),
                    410.0,
                    "start {start} round {round}"
                );

                animated.ensure_focused_visible_animated(500);
                animated.stop_animation();
                assert_eq!(
                    animated.scroll_offset(),
                    410.0,
                    "start {start} round {round}"
                );
            }
        }
    }

    #[test]
    fn set_scroll_offset_clamps_rejects_non_finite_and_cancels_animation() {
        // Regression #1: a restored offset from a wider session left every window
        // off-screen. Single 400px column, viewport 1000 -> max scroll 0.
        let mut ws = ws_with_columns(10, 10, &[400]);
        ws.set_scroll_offset(3000.0, 1000);
        assert_eq!(ws.scroll_offset(), 0.0);
        let p = ws.compute_placements(Rect::new(0, 0, 1000, 600));
        assert_eq!(p[0].visibility, Visibility::Visible);

        // total 1240, viewport 500 -> max 740.
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        for (offset, expected) in [
            (300.0, 300.0),
            (5000.0, 740.0),
            (-50.0, 0.0),
            (f64::NAN, 0.0), // ignored
            (f64::INFINITY, 0.0),
        ] {
            ws.set_scroll_offset(offset, 500);
            assert_eq!(ws.scroll_offset(), expected, "offset {offset}");
        }
    }

    #[test]
    fn direct_view_writes_replace_running_animation() {
        // Regression #3: a direct write during an animation was overwritten by the
        // animation's old target on the next tick. Every write must win.
        type Write = fn(&mut Workspace);
        let writes: [(&str, Write, f64); 4] = [
            ("scroll_by", |ws| ws.scroll_by(50.0, 500), 150.0),
            (
                "set_scroll_offset",
                |ws| ws.set_scroll_offset(20.0, 500),
                20.0,
            ),
            (
                "ensure_focused_visible",
                |ws| ws.ensure_focused_visible(500),
                740.0,
            ),
            ("cancel_animation", |ws| ws.cancel_animation(), 100.0),
        ];
        for (name, write, expected) in writes {
            let mut ws = ws_with_columns(10, 10, &[400, 400, 400]); // col 2 focused
            ws.animate_to(200.0, 500, 100, Easing::Linear);
            ws.tick_animation(50); // on screen: 100
            write(&mut ws);
            assert!(!ws.is_animating(), "{name}");
            assert!(!ws.tick_animation(100), "{name}");
            assert!(
                approx(ws.scroll_offset(), expected),
                "{name}: {}",
                ws.scroll_offset()
            );
        }
    }

    #[test]
    fn fully_minimized_columns_take_no_width() {
        // Regression #7: [400, 400 (minimized), 400] used to leave a 410px hole.
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.mark_minimized(2);

        assert_eq!(ws.total_width(), 830);
        let xs: Vec<(WindowId, i32)> = ws
            .compute_placements(Rect::new(0, 0, 2000, 600))
            .iter()
            .map(|p| (p.window_id, p.rect.x))
            .collect();
        assert_eq!(xs, vec![(1, 10), (3, 420)]);

        // Scroll targets use the packed strip: col 2 (400 + 2 * outer >= 400, so
        // left-aligned) at x 420 -> 410 (it would be 820 with the hole).
        ws.ensure_focused_visible(400);
        assert_eq!(ws.scroll_offset(), 410.0);

        // A focused column without a visible window does not scroll the view
        // (centering it would give 420 + 200 - 100 = 520).
        ws.set_scroll_offset(100.0, 200);
        ws.test_set_focus_unchecked(1, 0);
        ws.ensure_focused_visible(200);
        assert_eq!(ws.scroll_offset(), 100.0);
    }

    // ========================================================================
    // Placements
    // ========================================================================

    #[test]
    fn compute_placements_classifies_visibility_by_scroll_offset() {
        use Visibility::{OffScreenLeft as L, OffScreenRight as R, Visible as V};
        // Columns span strip x = 10-410, 420-820, 830-1230, 1240-1640; viewport is 500 wide.
        let base = ws_with_columns(10, 10, &[400, 400, 400, 400]);
        let cases = [
            (0.0, [V, V, R, R]),
            (330.0, [V, V, R, R]), // column left edge == viewport right is off-screen
            (420.0, [L, V, V, R]),
            (820.0, [L, L, V, V]), // column right edge == viewport left is off-screen
        ];
        for (offset, expected) in cases {
            let mut ws = base.clone();
            ws.set_scroll_offset(offset, 500);
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
        ws.set_scroll_offset(100.7, 500); // rounds to 101 (not truncated to 100)
        let viewport = Rect::new(1920, 50, 500, 600);
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

        ws.animate_to(200.0, 500, 100, Easing::Linear);
        assert!(ws.is_animating());
        assert!(approx(ws.scroll_offset(), 0.0));

        assert!(ws.tick_animation(50));
        assert!(approx(ws.scroll_offset(), 100.0));

        assert!(!ws.tick_animation(50));
        assert!(!ws.is_animating());
        assert_eq!(ws.scroll_offset(), 200.0);

        ws.animate_to(0.0, 500, DEFAULT_ANIMATION_DURATION_MS, Easing::default());
        assert!(ws.tick_animation(DEFAULT_ANIMATION_DURATION_MS - 1));
        assert!(!ws.tick_animation(1));
        assert_eq!(ws.scroll_offset(), 0.0);
    }

    #[test]
    fn animate_to_clamps_target_and_completes_trivial_moves_immediately() {
        // Content narrower than viewport: any target clamps to the current 0.
        let mut ws = ws_with_columns(10, 10, &[400]);
        ws.animate_to(500.0, 1000, 100, Easing::default());
        assert!(!ws.is_animating());

        // Over-range target clamps to max scroll.
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.animate_to(10_000.0, 500, 100, Easing::default());
        assert!(ws.is_animating());
        ws.stop_animation();
        assert_eq!(ws.scroll_offset(), 740.0);

        // Zero duration jumps straight to the target.
        ws.animate_to(100.0, 500, 0, Easing::default());
        assert!(!ws.is_animating());
        assert_eq!(ws.scroll_offset(), 100.0);
    }

    #[test]
    fn stop_snaps_to_target_and_cancel_keeps_current_position() {
        let start_halfway = || {
            let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
            ws.animate_to(200.0, 500, 100, Easing::Linear);
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
        ws.animate_to(200.0, 500, 100, Easing::Linear);
        ws.tick_animation(50);

        // Same target: the running animation keeps its timing.
        ws.animate_to(200.0, 500, 100, Easing::Linear);
        assert!(ws.tick_animation(25));
        assert!(approx(ws.scroll_offset(), 150.0));

        ws.animate_to(300.0, 500, 100, Easing::Linear);
        assert!(ws.is_animating());
        assert!(approx(ws.scroll_offset(), 150.0));

        ws.tick_animation(150);
        assert!(approx(ws.scroll_offset(), 300.0));
    }

    #[test]
    fn compute_placements_follow_running_animation() {
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        let viewport = Rect::new(0, 0, 500, 600);
        ws.animate_to(200.0, 500, 100, Easing::Linear);
        ws.tick_animation(50); // on screen: 100
        assert_eq!(ws.compute_placements(viewport)[0].rect.x, -90);
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
        ws.set_scroll_offset(300.0, 500); // floating windows ignore scroll
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

        // A minimized focused window is not floated.
        ws.mark_minimized(2);
        assert_eq!(ws.toggle_floating(viewport), None);
        assert!(!ws.is_floating(2) && ws.is_minimized(2));
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
        assert_eq!(ws.fullscreen_window_id(), None);

        ws.insert_window(1, Some(400)).unwrap();
        assert!(ws.toggle_fullscreen());
        assert_eq!(ws.fullscreen_window_id(), Some(1));
        assert!(!ws.toggle_fullscreen());
        assert_eq!(ws.fullscreen_window_id(), None);
    }

    #[test]
    fn toggle_fullscreen_targets_and_focuses_visible_window_when_focus_is_minimized() {
        let mut ws = ws_with_stack(10, 10, &[1, 2]);
        ws.mark_minimized(1);
        assert_eq!(ws.focused_window(), Some(1));

        assert!(ws.toggle_fullscreen());
        assert_eq!(ws.fullscreen_window_id(), Some(2));
        assert_eq!(
            ws.focused_window(),
            Some(2),
            "focus follows the fullscreen window"
        );

        // Everything minimized: nothing to fullscreen.
        ws.toggle_fullscreen();
        ws.mark_minimized(2);
        assert!(!ws.toggle_fullscreen());
        assert_eq!(ws.fullscreen_window_id(), None);
    }

    #[test]
    fn focus_change_to_another_window_leaves_fullscreen() {
        // Regression #5: focusing another window kept fullscreen, so the newly
        // focused window was off-screen with a zero rect.
        // Layout: col 0 = [1, 4], col 1 = [2], col 2 = [3]; fullscreen on 2.
        type Op = fn(&mut Workspace);
        let cases: [(&str, Op, Option<WindowId>); 9] = [
            ("focus_left", |ws| ws.focus_left(), None),
            ("focus_right", |ws| ws.focus_right(), None),
            ("focus_window other", |ws| ws.focus_window(3).unwrap(), None),
            (
                "insert taking focus",
                |ws| ws.insert_window(9, None).unwrap(),
                None,
            ),
            (
                "unfloat",
                |ws| {
                    ws.add_floating(9, Rect::new(0, 0, 10, 10)).unwrap();
                    ws.unfloat_window(9);
                },
                None,
            ),
            (
                "focus_window same",
                |ws| ws.focus_window(2).unwrap(),
                Some(2),
            ),
            (
                "insert keeping focus",
                |ws| ws.insert_window_with(9, None, InsertFocus::Keep).unwrap(),
                Some(2),
            ),
            ("focus_up at edge", |ws| ws.focus_up(), Some(2)),
            ("move_column_left", |ws| ws.move_column_left(), Some(2)),
        ];
        for (name, op, expected) in cases {
            let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
            ws.insert_window_in_column(4, 0).unwrap();
            ws.focus_window(2).unwrap();
            assert!(ws.toggle_fullscreen());

            op(&mut ws);

            assert_eq!(ws.fullscreen_window_id(), expected, "{name}");
            if expected.is_none() {
                let focused = ws.focused_window().unwrap();
                let viewport = Rect::new(0, 0, 1920, 1080);
                let p = ws.compute_placements(viewport);
                let placed = p.iter().find(|p| p.window_id == focused).unwrap();
                assert_ne!(placed.rect.width, 0, "{name}: focused window has no rect");
            }
        }

        // Vertical focus inside a fullscreen column also leaves fullscreen.
        let mut ws = ws_with_stack(10, 10, &[1, 2]);
        ws.toggle_fullscreen();
        ws.focus_down();
        assert_eq!(ws.fullscreen_window_id(), None);
    }

    #[test]
    fn fullscreen_placements_cover_viewport_and_hide_others() {
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.add_floating(4, Rect::new(0, 0, 100, 100)).unwrap();
        ws.focus_left();
        ws.toggle_fullscreen();
        assert_eq!(ws.fullscreen_window_id(), Some(2));

        let viewport = Rect::new(0, 0, 1920, 1080);
        let placements = ws.compute_placements(viewport);
        assert_eq!(placements.len(), 4);
        for p in &placements {
            if p.window_id == 2 {
                assert_eq!((p.rect, p.visibility), (viewport, Visibility::Visible));
            } else {
                assert_eq!(p.visibility, Visibility::OffScreenLeft, "{p:?}");
            }
        }
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

        // clear_fullscreen_if_window only clears the matching target.
        ws.toggle_fullscreen();
        assert!(!ws.clear_fullscreen_if_window(99));
        assert_eq!(ws.fullscreen_window_id(), Some(2));
        assert!(ws.clear_fullscreen_if_window(2));
        assert_eq!(ws.fullscreen_window_id(), None);
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
        assert_eq!(ws.fullscreen_window_id(), None);
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
        assert!(ws.minimized_windows.is_empty());
    }

    #[test]
    fn placements_skip_minimized_windows() {
        let mut ws = ws_with_columns(0, 0, &[400, 400]);
        let viewport = Rect::new(0, 0, 1920, 1080);
        assert_eq!(ws.compute_placements(viewport).len(), 2);

        ws.mark_minimized(1);
        let p = ws.compute_placements(viewport);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].window_id, 2);
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

    // ========================================================================
    // Serialization
    // ========================================================================

    #[test]
    fn workspace_snapshot_format_stays_compatible() {
        // Snapshot written by the previous version (daemon workspace-state.json).
        let old = r#"{
            "columns": [{"width": 800, "windows": [1, 2]}, {"width": 400, "windows": [3]}],
            "focused_column": 1, "focused_window_in_column": 0, "scroll_offset": 250.0,
            "gap": 10, "outer_gap": 10, "default_column_width": 800,
            "centering_mode": "Center"
        }"#;
        let ws: Workspace = serde_json::from_str(old).unwrap();
        assert_eq!(ws.scroll_offset(), 250.0);
        assert_eq!(ws.focused_window(), Some(3));
        assert!(!ws.is_animating());

        // A running animation is persisted as its resting target.
        let mut ws = ws_with_columns(10, 10, &[400, 400, 400]);
        ws.animate_to(200.0, 500, 100, Easing::Linear);
        ws.tick_animation(50);
        let json = serde_json::to_value(&ws).unwrap();
        assert_eq!(json["scroll_offset"], 200.0);
        let back: Workspace = serde_json::from_value(json).unwrap();
        assert_eq!(back.scroll_offset(), 200.0);
    }
}
