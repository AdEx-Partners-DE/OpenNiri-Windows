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

    #[error("Cannot remove the last column")]
    CannotRemoveLastColumn,

    #[error("Column is empty")]
    EmptyColumn,
}

/// A rectangle in screen coordinates (pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    /// Create a new rectangle.
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    /// Width of the column in pixels.
    pub width: i32,
    /// Windows in this column (vertically stacked).
    pub windows: Vec<WindowId>,
}

impl Column {
    /// Create a new column with a single window.
    pub fn new(window_id: WindowId, width: i32) -> Self {
        Self {
            width,
            windows: vec![window_id],
        }
    }

    /// Create an empty column with specified width.
    pub fn empty(width: i32) -> Self {
        Self {
            width,
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
    pub fn remove_window(&mut self, window_id: WindowId) -> bool {
        if let Some(pos) = self.windows.iter().position(|&w| w == window_id) {
            self.windows.remove(pos);
            true
        } else {
            false
        }
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Columns in the workspace, ordered left to right.
    pub columns: Vec<Column>,
    /// Index of the currently focused column.
    pub focused_column: usize,
    /// Index of the focused window within the focused column.
    pub focused_window_in_column: usize,
    /// Current scroll offset (x position of viewport's left edge on the strip).
    pub scroll_offset: f64,
    /// Gap between columns in pixels.
    pub gap: i32,
    /// Gap at the edges of the viewport.
    pub outer_gap: i32,
    /// Default width for new columns.
    pub default_column_width: i32,
    /// Centering mode for focus changes.
    pub centering_mode: CenteringMode,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            focused_column: 0,
            focused_window_in_column: 0,
            scroll_offset: 0.0,
            gap: 10,
            outer_gap: 10,
            default_column_width: 800,
            centering_mode: CenteringMode::default(),
        }
    }
}

impl Workspace {
    /// Create a new empty workspace with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a workspace with custom gap settings.
    pub fn with_gaps(gap: i32, outer_gap: i32) -> Self {
        Self {
            gap,
            outer_gap,
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

    /// Get the total width of the strip (sum of all column widths + gaps).
    pub fn total_width(&self) -> i32 {
        if self.columns.is_empty() {
            return 0;
        }

        let column_widths: i32 = self.columns.iter().map(|c| c.width).sum();
        let gaps = self.gap * (self.columns.len() as i32 - 1);
        let outer_gaps = self.outer_gap * 2;

        column_widths + gaps + outer_gaps
    }

    /// Insert a new window as a new column to the right of the focused column.
    pub fn insert_window(&mut self, window_id: WindowId, width: Option<i32>) {
        let column_width = width.unwrap_or(self.default_column_width);
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
    }

    /// Insert a window into an existing column (stacking).
    pub fn insert_window_in_column(
        &mut self,
        window_id: WindowId,
        column_index: usize,
    ) -> Result<(), LayoutError> {
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
    pub fn remove_window(&mut self, window_id: WindowId) -> Result<(), LayoutError> {
        for (col_idx, column) in self.columns.iter_mut().enumerate() {
            if column.remove_window(window_id) {
                // If column is now empty, remove it (unless it's the last column)
                if column.is_empty() && self.columns.len() > 1 {
                    self.columns.remove(col_idx);
                    // Adjust focused column if needed
                    if self.focused_column >= self.columns.len() {
                        self.focused_column = self.columns.len().saturating_sub(1);
                    } else if self.focused_column > col_idx {
                        self.focused_column = self.focused_column.saturating_sub(1);
                    }
                }
                // Adjust focused window in column if needed
                if col_idx == self.focused_column {
                    let col_len = self.columns.get(self.focused_column).map_or(0, |c| c.len());
                    if self.focused_window_in_column >= col_len {
                        self.focused_window_in_column = col_len.saturating_sub(1);
                    }
                }
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

    /// Calculate the x-coordinate of a column's left edge on the strip.
    fn column_x(&self, column_index: usize) -> i32 {
        let mut x = self.outer_gap;
        for (i, col) in self.columns.iter().enumerate() {
            if i == column_index {
                return x;
            }
            x += col.width + self.gap;
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
    pub fn ensure_focused_visible(&mut self, viewport_width: i32) {
        if self.columns.is_empty() {
            return;
        }

        let Some((col_x, col_width)) = self.focused_column_bounds() else {
            return;
        };

        match self.centering_mode {
            CenteringMode::Center => {
                // Center the focused column in the viewport
                let col_center = col_x + col_width / 2;
                self.scroll_offset = (col_center - viewport_width / 2) as f64;
            }
            CenteringMode::JustInView => {
                // Only scroll if the focused column is outside the viewport
                let viewport_left = self.scroll_offset as i32;
                let viewport_right = viewport_left + viewport_width;

                if col_x < viewport_left {
                    // Column is to the left of viewport, scroll left
                    self.scroll_offset = (col_x - self.outer_gap) as f64;
                } else if col_x + col_width > viewport_right {
                    // Column is to the right of viewport, scroll right
                    self.scroll_offset =
                        (col_x + col_width + self.outer_gap - viewport_width) as f64;
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
    pub fn compute_placements(&self, viewport: Rect) -> Vec<WindowPlacement> {
        let mut placements = Vec::new();

        if self.columns.is_empty() {
            return placements;
        }

        let viewport_left = self.scroll_offset as i32;
        let viewport_right = viewport_left + viewport.width;

        let mut current_x = self.outer_gap;

        for (col_idx, column) in self.columns.iter().enumerate() {
            // Calculate column position in strip coordinates
            let col_strip_x = current_x;
            let col_strip_right = col_strip_x + column.width;

            // Transform to screen coordinates (relative to viewport)
            let col_screen_x = col_strip_x - viewport_left + viewport.x;

            // Determine visibility
            let visibility = if col_strip_right <= viewport_left {
                Visibility::OffScreenLeft
            } else if col_strip_x >= viewport_right {
                Visibility::OffScreenRight
            } else {
                Visibility::Visible
            };

            // Calculate window heights (equal split for stacked windows)
            let usable_height = viewport.height - self.outer_gap * 2;
            let window_count = column.windows.len() as i32;
            let window_gaps = if window_count > 1 {
                self.gap * (window_count - 1)
            } else {
                0
            };
            let window_height = if window_count > 0 {
                (usable_height - window_gaps) / window_count
            } else {
                0
            };

            let mut current_y = viewport.y + self.outer_gap;

            for (win_idx, &window_id) in column.windows.iter().enumerate() {
                // Adjust height for last window to handle rounding
                let height = if win_idx == column.windows.len() - 1 {
                    viewport.y + viewport.height - self.outer_gap - current_y
                } else {
                    window_height
                };

                placements.push(WindowPlacement {
                    window_id,
                    rect: Rect::new(col_screen_x, current_y, column.width, height),
                    visibility,
                    column_index: col_idx,
                });

                current_y += height + self.gap;
            }

            current_x += column.width + self.gap;
        }

        placements
    }

    /// Resize the focused column by a delta amount.
    pub fn resize_focused_column(&mut self, delta: i32) {
        if let Some(column) = self.columns.get_mut(self.focused_column) {
            let new_width = (column.width + delta).max(100); // Minimum width
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
    pub fn scroll_by(&mut self, delta: f64, viewport_width: i32) {
        self.scroll_offset += delta;
        let max_scroll = (self.total_width() - viewport_width).max(0);
        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll as f64);
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
        ws.insert_window(1, Some(400));

        assert!(!ws.is_empty());
        assert_eq!(ws.column_count(), 1);
        assert_eq!(ws.focused_column, 0);
        assert_eq!(ws.focused_window(), Some(1));
    }

    #[test]
    fn test_insert_multiple_windows() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400));
        ws.insert_window(2, Some(600));
        ws.insert_window(3, Some(400));

        assert_eq!(ws.column_count(), 3);
        // Last inserted window should be focused
        assert_eq!(ws.focused_column, 2);
        assert_eq!(ws.focused_window(), Some(3));

        // Total width: outer_gap + 400 + gap + 600 + gap + 400 + outer_gap
        // = 10 + 400 + 10 + 600 + 10 + 400 + 10 = 1440
        assert_eq!(ws.total_width(), 1440);
    }

    #[test]
    fn test_focus_navigation() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400));
        ws.insert_window(2, Some(400));
        ws.insert_window(3, Some(400));

        assert_eq!(ws.focused_column, 2); // Last inserted

        ws.focus_left();
        assert_eq!(ws.focused_column, 1);
        assert_eq!(ws.focused_window(), Some(2));

        ws.focus_left();
        assert_eq!(ws.focused_column, 0);

        // Should not go below 0
        ws.focus_left();
        assert_eq!(ws.focused_column, 0);

        ws.focus_right();
        ws.focus_right();
        assert_eq!(ws.focused_column, 2);

        // Should not go beyond last column
        ws.focus_right();
        assert_eq!(ws.focused_column, 2);
    }

    #[test]
    fn test_remove_window() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400));
        ws.insert_window(2, Some(400));
        ws.insert_window(3, Some(400));

        assert_eq!(ws.column_count(), 3);

        ws.remove_window(2).unwrap();
        assert_eq!(ws.column_count(), 2);

        // Windows 1 and 3 should remain
        assert!(ws
            .columns
            .iter()
            .any(|c| c.windows.contains(&1)));
        assert!(ws
            .columns
            .iter()
            .any(|c| c.windows.contains(&3)));
    }

    #[test]
    fn test_compute_placements_visibility() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400)); // x: 10-410
        ws.insert_window(2, Some(400)); // x: 420-820
        ws.insert_window(3, Some(400)); // x: 830-1230

        ws.scroll_offset = 0.0;

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
        ws.centering_mode = CenteringMode::Center;

        ws.insert_window(1, Some(400));
        ws.insert_window(2, Some(400));
        ws.insert_window(3, Some(400));

        ws.focused_column = 0;
        ws.scroll_offset = 500.0; // Start scrolled right

        ws.ensure_focused_visible(500);

        // Should center column 0 in the viewport
        // Column 0 is at x=10, width=400, center=210
        // Viewport width=500, center=250
        // scroll_offset = 210 - 250 = -40, clamped to 0
        assert_eq!(ws.scroll_offset, 0.0);
    }

    #[test]
    fn test_stacked_windows() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400));
        ws.insert_window_in_column(2, 0).unwrap();
        ws.insert_window_in_column(3, 0).unwrap();

        assert_eq!(ws.column_count(), 1);
        assert_eq!(ws.columns[0].len(), 3);

        let viewport = Rect::new(0, 0, 500, 600);
        let placements = ws.compute_placements(viewport);

        assert_eq!(placements.len(), 3);
        // All three windows should be in the same column
        assert!(placements.iter().all(|p| p.column_index == 0));
    }

    #[test]
    fn test_resize_column() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400));

        assert_eq!(ws.columns[0].width, 400);

        ws.resize_focused_column(100);
        assert_eq!(ws.columns[0].width, 500);

        ws.resize_focused_column(-200);
        assert_eq!(ws.columns[0].width, 300);

        // Should not go below minimum (100)
        ws.resize_focused_column(-500);
        assert_eq!(ws.columns[0].width, 100);
    }

    #[test]
    fn test_move_column() {
        let mut ws = Workspace::new();
        ws.insert_window(1, Some(400));
        ws.insert_window(2, Some(400));
        ws.insert_window(3, Some(400));

        ws.focused_column = 1;
        ws.move_column_left();

        assert_eq!(ws.focused_column, 0);
        assert_eq!(ws.columns[0].windows[0], 2);
        assert_eq!(ws.columns[1].windows[0], 1);

        ws.move_column_right();
        assert_eq!(ws.focused_column, 1);
        assert_eq!(ws.columns[0].windows[0], 1);
        assert_eq!(ws.columns[1].windows[0], 2);
    }

    #[test]
    fn test_scroll_by() {
        let mut ws = Workspace::with_gaps(10, 10);
        ws.insert_window(1, Some(400));
        ws.insert_window(2, Some(400));
        ws.insert_window(3, Some(400));

        let viewport_width = 500;

        ws.scroll_by(100.0, viewport_width);
        assert_eq!(ws.scroll_offset, 100.0);

        ws.scroll_by(2000.0, viewport_width);
        // Should clamp to max scroll
        let max_scroll = (ws.total_width() - viewport_width).max(0) as f64;
        assert_eq!(ws.scroll_offset, max_scroll);

        ws.scroll_by(-5000.0, viewport_width);
        assert_eq!(ws.scroll_offset, 0.0);
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
}
