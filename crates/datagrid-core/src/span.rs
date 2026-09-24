//! Cells that cover more than one column.
//!
//! A column can say, per row, how many columns its cell covers
//! ([`ColumnSpec::span`](crate::ColumnSpec::span)). The columns it covers render
//! no cell of their own, so the row still has as many columns as the header —
//! it just draws fewer boxes.
//!
//! Resolving that is a row-at-a-time job: the same column may span three
//! columns in one row and one in the next. [`RowSpans`] is the answer for one
//! row, and the one place that knows which cell a column belongs to.

use crate::Pinned;

/// Which cell covers each column of one row.
///
/// Built with [`resolve`](RowSpans::resolve) from what each column declares for
/// this row. Every column has exactly one covering cell, identified by the
/// column that cell starts at — its *anchor*.
///
/// ```
/// use datagrid_core::{Pinned, RowSpans};
///
/// // The first cell covers three columns, the last stands alone.
/// let spans = RowSpans::resolve(&[
///     (3, Pinned::None),
///     (1, Pinned::None),
///     (1, Pinned::None),
///     (1, Pinned::None),
/// ]);
///
/// assert_eq!(spans.anchor(2), 0);
/// assert_eq!(spans.width(2), 3);
/// assert!(spans.is_anchor(3));
///
/// // Moving right out of the wide cell skips the columns it covers.
/// assert_eq!(spans.step(0, 1), 3);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RowSpans {
    /// For each column, the column whose cell covers it.
    anchors: Vec<usize>,
}

impl RowSpans {
    /// A row in which every column has its own cell.
    #[must_use]
    pub fn none(columns: usize) -> Self {
        Self {
            anchors: (0..columns).collect(),
        }
    }

    /// Resolves what each column declares for one row, given where each column
    /// is pinned.
    ///
    /// A span is read left to right and clamped twice: to the columns the row
    /// has left, and to the end of the pinned block its first column is in — a
    /// cell cannot be held at an edge and scroll at the same time. A declared
    /// span of `0` covers the column itself, as `1` does. Columns covered by an
    /// earlier cell keep whatever they declare; it is never read.
    #[must_use]
    pub fn resolve(columns: &[(usize, Pinned)]) -> Self {
        let count = columns.len();
        let mut anchors = Vec::with_capacity(count);
        let mut start = 0;

        while let Some(&(declared, pin)) = columns.get(start) {
            let mut width = declared.max(1).min(count - start);
            while width > 1 && columns.get(start + width - 1).map(|(_, at)| *at) != Some(pin) {
                width -= 1;
            }
            anchors.extend(std::iter::repeat_n(start, width));
            start += width;
        }

        Self { anchors }
    }

    /// How many columns the row covers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.anchors.len()
    }

    /// Whether the row has no columns at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.anchors.is_empty()
    }

    /// Whether any cell of this row covers more than one column.
    ///
    /// The cheap way to tell a row that spans nothing — the usual case — from
    /// one that does.
    #[must_use]
    pub fn has_spans(&self) -> bool {
        self.anchors
            .iter()
            .enumerate()
            .any(|(column, anchor)| *anchor != column)
    }

    /// The column the cell covering `column` starts at.
    ///
    /// A column past the end of the row is its own anchor, so a stale
    /// coordinate stays where it is rather than snapping somewhere surprising.
    #[must_use]
    pub fn anchor(&self, column: usize) -> usize {
        self.anchors.get(column).copied().unwrap_or(column)
    }

    /// Whether a cell starts at this column, rather than being covered by one
    /// that started earlier. Only anchors are rendered.
    #[must_use]
    pub fn is_anchor(&self, column: usize) -> bool {
        self.anchor(column) == column
    }

    /// How many columns the cell covering `column` spans. Never zero.
    #[must_use]
    pub fn width(&self, column: usize) -> usize {
        let start = self.anchor(column);
        self.anchors
            .get(start..)
            .map_or(1, |rest| rest.iter().take_while(|at| **at == start).count())
            .max(1)
    }

    /// Where a navigation key that wants to go from column `from` to column
    /// `to` actually lands.
    ///
    /// Focus belongs on a cell, not on a column it covers, so the result is
    /// always an anchor. A move that lands on a covered column lands on the
    /// cell covering it. A move to the right that would stay inside the cell it
    /// started in carries on past that cell instead: an arrow key that visibly
    /// does nothing reads as a broken key.
    ///
    /// A move that keeps the column — down a row, or up one — is not a move to
    /// the right, so it settles on whichever cell covers that column in this
    /// row.
    ///
    /// `to` is clamped to the row, so a move off the end stops at the last
    /// cell.
    #[must_use]
    pub fn step(&self, from: usize, to: usize) -> usize {
        let Some(last) = self.anchors.len().checked_sub(1) else {
            return 0;
        };
        // The column it came from decides the direction, the cell it came from
        // decides what counts as staying put. A move straight down keeps the
        // column, so it is neither.
        let start = self.anchor(from.min(last));
        let mut to = to.min(last);
        if to > from && self.anchor(to) == start {
            to = (start + self.width(start)).min(last);
        }
        self.anchor(to)
    }
}
