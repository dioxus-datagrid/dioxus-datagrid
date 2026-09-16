//! Keyboard navigation as a pure state machine.
//!
//! Keeping this separate from the renderer means the ARIA grid pattern's
//! movement rules can be tested exhaustively without a DOM.

/// Which cell currently holds focus, in view coordinates.
///
/// `row` indexes the rows currently displayed, not the underlying data, so it
/// stays valid across sorting, filtering and paging.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellFocus {
    /// Zero-based row within the current view.
    pub row: usize,
    /// Zero-based column within the visible columns.
    pub col: usize,
}

impl CellFocus {
    /// Creates a focus position.
    #[must_use]
    pub const fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

/// A navigation key press, already resolved from the raw keyboard event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NavKey {
    /// One row up.
    Up,
    /// One row down.
    Down,
    /// One column left.
    Left,
    /// One column right.
    Right,
    /// First column of the current row.
    Home,
    /// Last column of the current row.
    End,
    /// First cell of the grid.
    CtrlHome,
    /// Last cell of the grid.
    CtrlEnd,
    /// One viewport up.
    PageUp,
    /// One viewport down.
    PageDown,
}

/// Moves the focus according to the ARIA grid pattern.
///
/// Movement **clamps at the edges** rather than wrapping: the authoring
/// practices treat a grid as a plane, and wrapping would make the arrow keys
/// jump unpredictably across rows.
///
/// `page_rows` is how far `PageUp`/`PageDown` travel — usually the number of
/// rows that fit in the viewport. A value of `0` leaves the focus unmoved.
///
/// When the grid has no rows or no columns there is nothing to focus, and the
/// result is `(0, 0)`.
///
/// # Examples
///
/// ```
/// use datagrid_core::{navigate, CellFocus, NavKey};
///
/// let focus = CellFocus::new(3, 2);
///
/// // Arrow keys move by one and clamp at the edge.
/// assert_eq!(navigate(focus, NavKey::Down, 10, 5, 8), CellFocus::new(4, 2));
/// assert_eq!(
///     navigate(CellFocus::new(9, 2), NavKey::Down, 10, 5, 8),
///     CellFocus::new(9, 2)
/// );
///
/// // Home and End stay within the row; Ctrl versions jump to the corners.
/// assert_eq!(navigate(focus, NavKey::End, 10, 5, 8), CellFocus::new(3, 4));
/// assert_eq!(navigate(focus, NavKey::CtrlEnd, 10, 5, 8), CellFocus::new(9, 4));
/// ```
#[must_use]
pub fn navigate(
    focus: CellFocus,
    key: NavKey,
    rows: usize,
    cols: usize,
    page_rows: usize,
) -> CellFocus {
    if rows == 0 || cols == 0 {
        return CellFocus::new(0, 0);
    }

    let last_row = rows - 1;
    let last_col = cols - 1;

    // A focus carried over from a larger grid must land somewhere valid.
    let row = focus.row.min(last_row);
    let col = focus.col.min(last_col);

    match key {
        NavKey::Up => CellFocus::new(row.saturating_sub(1), col),
        NavKey::Down => CellFocus::new((row + 1).min(last_row), col),
        NavKey::Left => CellFocus::new(row, col.saturating_sub(1)),
        NavKey::Right => CellFocus::new(row, (col + 1).min(last_col)),
        NavKey::Home => CellFocus::new(row, 0),
        NavKey::End => CellFocus::new(row, last_col),
        NavKey::CtrlHome => CellFocus::new(0, 0),
        NavKey::CtrlEnd => CellFocus::new(last_row, last_col),
        NavKey::PageUp => CellFocus::new(row.saturating_sub(page_rows), col),
        NavKey::PageDown => CellFocus::new(row.saturating_add(page_rows).min(last_row), col),
    }
}
