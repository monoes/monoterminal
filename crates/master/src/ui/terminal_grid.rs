//! Terminal Grid - Cell-based storage for terminal state
//!
//! Manages terminal cells with attributes, cursor position, and dirty tracking
//! Target: <0.5ms dirty tracking per SRS §2.1.1

use super::vt_parser::{CellStyle, Color, GridChange};

/// Terminal grid managing cells and cursor
pub struct TerminalGrid {
    /// Rows
    rows: u16,
    /// Columns
    cols: u16,
    /// Cell grid (row-major order)
    cells: Vec<Cell>,
    /// Current cursor position
    cursor_row: u16,
    cursor_col: u16,
    /// Current style
    current_style: CellStyle,
    /// Dirty region tracking
    dirty: DirtyRegion,
}

impl TerminalGrid {
    /// Create new terminal grid
    pub fn new(rows: u16, cols: u16) -> Self {
        let cell_count = (rows as usize) * (cols as usize);
        let cells = vec![Cell::default(); cell_count];

        Self {
            rows,
            cols,
            cells,
            cursor_row: 0,
            cursor_col: 0,
            current_style: CellStyle::default(),
            dirty: DirtyRegion::new(rows, cols),
        }
    }

    /// Apply changes from VT parser
    /// Target: <0.5ms dirty tracking
    pub fn apply_changes(&mut self, changes: Vec<GridChange>) {
        for change in changes {
            match change {
                GridChange::PrintChar(ch) => {
                    self.print_char(ch);
                }
                GridChange::Newline => {
                    self.newline();
                }
                GridChange::CarriageReturn => {
                    self.cursor_col = 0;
                    self.dirty.mark_row(self.cursor_row);
                }
                GridChange::Tab => {
                    // Tab to next 8-column boundary
                    let next_col = ((self.cursor_col / 8) + 1) * 8;
                    self.cursor_col = next_col.min(self.cols - 1);
                    self.dirty.mark_row(self.cursor_row);
                }
                GridChange::Backspace => {
                    if self.cursor_col > 0 {
                        self.cursor_col -= 1;
                        self.dirty.mark_cell(self.cursor_row, self.cursor_col);
                    }
                }
                GridChange::ClearScreen => {
                    self.clear();
                }
                GridChange::CursorMove { row, col } => {
                    self.cursor_row = row.min(self.rows - 1);
                    self.cursor_col = col.min(self.cols - 1);
                }
                GridChange::StyleChange(style) => {
                    self.current_style = style;
                }
                GridChange::ScrollUp => {
                    self.scroll_up();
                }
                GridChange::ScrollDown => {
                    self.scroll_down();
                }
            }
        }
    }

    /// Print character at cursor position
    fn print_char(&mut self, ch: char) {
        if self.cursor_row >= self.rows || self.cursor_col >= self.cols {
            return;
        }

        let idx = (self.cursor_row as usize * self.cols as usize) + self.cursor_col as usize;
        self.cells[idx] = Cell {
            ch,
            style: self.current_style,
        };

        self.dirty.mark_cell(self.cursor_row, self.cursor_col);

        // Advance cursor
        self.cursor_col += 1;
        if self.cursor_col >= self.cols {
            self.newline();
        }
    }

    /// Newline (move to next row, keep column or reset to 0)
    fn newline(&mut self) {
        self.cursor_row += 1;
        self.cursor_col = 0; // Most terminals do CR+LF

        if self.cursor_row >= self.rows {
            // Scroll up
            self.scroll_up();
            self.cursor_row = self.rows - 1;
        }

        self.dirty.mark_row(self.cursor_row);
    }

    /// Scroll up one line
    fn scroll_up(&mut self) {
        // Move all rows up by one
        self.cells.copy_within(self.cols as usize.., 0);

        // Clear bottom row
        let bottom_row_start = (self.rows - 1) as usize * self.cols as usize;
        for i in bottom_row_start..(bottom_row_start + self.cols as usize) {
            self.cells[i] = Cell::default();
        }

        self.dirty.mark_all();
    }

    /// Scroll down one line
    fn scroll_down(&mut self) {
        // Move all rows down by one
        let total_cells = (self.rows as usize) * (self.cols as usize);
        self.cells
            .copy_within(0..(total_cells - self.cols as usize), self.cols as usize);

        // Clear top row
        for i in 0..self.cols as usize {
            self.cells[i] = Cell::default();
        }

        self.dirty.mark_all();
    }

    /// Clear entire screen
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = Cell::default();
        }
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.dirty.mark_all();
    }

    /// Get cell at position
    pub fn get_cell(&self, row: u16, col: u16) -> Option<&Cell> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        let idx = (row as usize * self.cols as usize) + col as usize;
        self.cells.get(idx)
    }

    /// Get dirty region
    pub fn dirty_region(&self) -> &DirtyRegion {
        &self.dirty
    }

    /// Clear dirty region
    pub fn clear_dirty(&mut self) {
        self.dirty.clear();
    }

    /// Get dimensions
    pub fn dimensions(&self) -> (u16, u16) {
        (self.rows, self.cols)
    }

    /// Get cursor position
    pub fn cursor_position(&self) -> (u16, u16) {
        (self.cursor_row, self.cursor_col)
    }

    /// Resize grid
    pub fn resize(&mut self, rows: u16, cols: u16) {
        let new_cell_count = (rows as usize) * (cols as usize);
        let mut new_cells = vec![Cell::default(); new_cell_count];

        // Copy old cells to new grid (up to min dimensions)
        let copy_rows = self.rows.min(rows);
        let copy_cols = self.cols.min(cols);

        for r in 0..copy_rows {
            for c in 0..copy_cols {
                let old_idx = (r as usize * self.cols as usize) + c as usize;
                let new_idx = (r as usize * cols as usize) + c as usize;
                new_cells[new_idx] = self.cells[old_idx];
            }
        }

        self.rows = rows;
        self.cols = cols;
        self.cells = new_cells;
        self.dirty = DirtyRegion::new(rows, cols);
        self.dirty.mark_all();
    }
}

/// Terminal cell with character and style
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub style: CellStyle,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            style: CellStyle::default(),
        }
    }
}

/// Dirty region tracking for efficient rendering
/// Target: <0.5ms per frame
pub struct DirtyRegion {
    rows: u16,
    cols: u16,
    /// Dirty cells bitmap (1 bit per cell)
    dirty_cells: Vec<bool>,
    /// Is entire grid dirty?
    all_dirty: bool,
}

impl DirtyRegion {
    pub fn new(rows: u16, cols: u16) -> Self {
        let cell_count = (rows as usize) * (cols as usize);
        Self {
            rows,
            cols,
            dirty_cells: vec![false; cell_count],
            all_dirty: true, // Start dirty for initial render
        }
    }

    /// Mark cell as dirty
    pub fn mark_cell(&mut self, row: u16, col: u16) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        let idx = (row as usize * self.cols as usize) + col as usize;
        self.dirty_cells[idx] = true;
    }

    /// Mark entire row as dirty
    pub fn mark_row(&mut self, row: u16) {
        if row >= self.rows {
            return;
        }
        let start = row as usize * self.cols as usize;
        let end = start + self.cols as usize;
        for i in start..end {
            self.dirty_cells[i] = true;
        }
    }

    /// Mark all cells as dirty
    pub fn mark_all(&mut self) {
        self.all_dirty = true;
    }

    /// Check if cell is dirty
    pub fn is_dirty(&self, row: u16, col: u16) -> bool {
        if self.all_dirty {
            return true;
        }
        if row >= self.rows || col >= self.cols {
            return false;
        }
        let idx = (row as usize * self.cols as usize) + col as usize;
        self.dirty_cells[idx]
    }

    /// Clear dirty region
    pub fn clear(&mut self) {
        self.all_dirty = false;
        for dirty in &mut self.dirty_cells {
            *dirty = false;
        }
    }

    /// Iterate over dirty cells (row, col)
    pub fn iter_dirty(&self) -> impl Iterator<Item = (u16, u16)> + '_ {
        (0..self.rows).flat_map(move |row| {
            (0..self.cols).filter_map(move |col| {
                if self.is_dirty(row, col) {
                    Some((row, col))
                } else {
                    None
                }
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_print() {
        let mut grid = TerminalGrid::new(24, 80);
        grid.apply_changes(vec![GridChange::PrintChar('H'), GridChange::PrintChar('i')]);

        assert_eq!(grid.get_cell(0, 0).unwrap().ch, 'H');
        assert_eq!(grid.get_cell(0, 1).unwrap().ch, 'i');
        assert_eq!(grid.cursor_position(), (0, 2));
    }

    #[test]
    fn test_grid_newline() {
        let mut grid = TerminalGrid::new(24, 80);
        grid.apply_changes(vec![GridChange::PrintChar('A'), GridChange::Newline]);

        assert_eq!(grid.cursor_position(), (1, 0));
    }

    #[test]
    fn test_dirty_tracking() {
        let mut grid = TerminalGrid::new(24, 80);
        grid.clear_dirty();

        grid.apply_changes(vec![GridChange::PrintChar('X')]);

        assert!(grid.dirty_region().is_dirty(0, 0));
        assert!(!grid.dirty_region().is_dirty(0, 1));
    }
}
