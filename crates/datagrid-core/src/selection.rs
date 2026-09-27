//! Selection: rows by their key, cells by their place.
//!
//! The two are different in kind, and the difference is not a detail. A row is
//! selected by [`GridRow::key`](crate::GridRow::key), so the selection survives
//! sorting, filtering and paging. A cell has no key — it is a place in the view,
//! and a rectangle of cells is a rectangle of places. See [`CellRange`].

use crate::CellFocus;
use std::collections::HashSet;
use std::hash::Hash;
use std::ops::RangeInclusive;

/// How many rows the user may select.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SelectionMode {
    /// Selection is disabled; every mutating call is a no-op.
    #[default]
    None,
    /// At most one row at a time.
    Single,
    /// Any number of rows, including shift-ranges.
    Multi,
}

/// The set of selected row keys, plus the anchor a shift-range extends from.
///
/// Keys are whatever [`GridRow::key`](crate::GridRow::key) returns, so selection
/// survives sorting, filtering and paging — unlike selection by row index.
#[derive(Clone, Debug)]
pub struct Selection<K> {
    selected: HashSet<K>,
    anchor: Option<K>,
}

impl<K> Default for Selection<K> {
    fn default() -> Self {
        Self {
            selected: HashSet::new(),
            anchor: None,
        }
    }
}

impl<K> PartialEq for Selection<K>
where
    K: Eq + Hash,
{
    /// Compares the selected set. The anchor is interaction bookkeeping, not
    /// part of the selection's identity.
    fn eq(&self, other: &Self) -> bool {
        self.selected == other.selected
    }
}

impl<K> Selection<K>
where
    K: Clone + Eq + Hash,
{
    /// Creates an empty selection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a row is selected.
    #[must_use]
    pub fn contains(&self, key: &K) -> bool {
        self.selected.contains(key)
    }

    /// How many rows are selected.
    #[must_use]
    pub fn len(&self) -> usize {
        self.selected.len()
    }

    /// Whether nothing is selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// Iterates the selected keys in arbitrary order.
    pub fn iter(&self) -> impl Iterator<Item = &K> {
        self.selected.iter()
    }

    /// The key a shift-range would extend from.
    #[must_use]
    pub fn anchor(&self) -> Option<&K> {
        self.anchor.as_ref()
    }

    /// Selects a row, replacing the selection in
    /// [`Single`](SelectionMode::Single) mode and adding to it in
    /// [`Multi`](SelectionMode::Multi).
    ///
    /// Sets the anchor for a later [`extend_to`](Selection::extend_to).
    pub fn select(&mut self, key: K, mode: SelectionMode) {
        match mode {
            SelectionMode::None => {}
            SelectionMode::Single => {
                self.selected.clear();
                self.selected.insert(key.clone());
                self.anchor = Some(key);
            }
            SelectionMode::Multi => {
                self.selected.insert(key.clone());
                self.anchor = Some(key);
            }
        }
    }

    /// Toggles a row.
    ///
    /// In [`Single`](SelectionMode::Single) mode, selecting a different row
    /// replaces the selection and re-selecting the current one clears it.
    pub fn toggle(&mut self, key: K, mode: SelectionMode) {
        match mode {
            SelectionMode::None => {}
            SelectionMode::Single => {
                if self.selected.contains(&key) {
                    self.selected.clear();
                    self.anchor = None;
                } else {
                    self.select(key, mode);
                }
            }
            SelectionMode::Multi => {
                if self.selected.remove(&key) {
                    self.anchor = Some(key);
                } else {
                    self.selected.insert(key.clone());
                    self.anchor = Some(key);
                }
            }
        }
    }

    /// Deselects a row without touching the rest of the selection.
    pub fn deselect(&mut self, key: &K) {
        self.selected.remove(key);
    }

    /// Clears the selection and the anchor.
    pub fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    /// Selects every key between the anchor and `key`, inclusive.
    ///
    /// `ordered_keys` supplies the order the user sees — typically the keys of
    /// the current view — because "everything between these two rows" only
    /// means something in display order.
    ///
    /// Only does anything in [`Multi`](SelectionMode::Multi) mode. Falls back to
    /// a plain [`select`](Selection::select) when there is no anchor, or when
    /// either end is missing from `ordered_keys` — a range against rows that are
    /// no longer displayed would be arbitrary.
    ///
    /// The anchor is deliberately left where it was, so dragging a shift-range
    /// back and forth keeps extending from the same origin.
    pub fn extend_to(&mut self, ordered_keys: &[K], key: K, mode: SelectionMode) {
        if mode != SelectionMode::Multi {
            self.select(key, mode);
            return;
        }

        let Some(anchor) = self.anchor.clone() else {
            self.select(key, mode);
            return;
        };

        let positions = ordered_keys
            .iter()
            .position(|candidate| candidate == &anchor)
            .zip(ordered_keys.iter().position(|candidate| candidate == &key));

        let Some((from, to)) = positions else {
            self.select(key, mode);
            return;
        };

        let (start, end) = if from <= to { (from, to) } else { (to, from) };
        if let Some(range) = ordered_keys.get(start..=end) {
            for candidate in range {
                self.selected.insert(candidate.clone());
            }
        }
    }

    /// Replaces the selection with exactly these keys and clears the anchor.
    pub fn set(&mut self, keys: impl IntoIterator<Item = K>) {
        self.selected = keys.into_iter().collect();
        self.anchor = None;
    }

    /// Drops selected keys that are no longer present in `keys`.
    ///
    /// Useful when the underlying data changed and selection should not keep
    /// referring to rows that disappeared.
    pub fn retain_existing(&mut self, keys: &HashSet<K>) {
        self.selected.retain(|key| keys.contains(key));
        if let Some(anchor) = &self.anchor
            && !keys.contains(anchor)
        {
            self.anchor = None;
        }
    }
}

impl<K> FromIterator<K> for Selection<K>
where
    K: Clone + Eq + Hash,
{
    fn from_iter<I: IntoIterator<Item = K>>(iter: I) -> Self {
        Self {
            selected: iter.into_iter().collect(),
            anchor: None,
        }
    }
}

/// How the user may select cells, independent of row selection.
///
/// Cells and rows are selected separately: a grid can offer both, either or
/// neither. Where they compete for a key, the cells win while cell selection is
/// on — see the keyboard table in `docs/ACCESSIBILITY.md`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CellSelectionMode {
    /// Cells cannot be selected; every call is a no-op.
    #[default]
    None,
    /// One cell at a time. Moving the focus takes the selection with it.
    Single,
    /// A rectangle, extended with `Shift` and the arrow keys or a shift-click.
    Range,
}

impl CellSelectionMode {
    /// Whether cells can be selected at all.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        !matches!(self, Self::None)
    }

    /// Whether a selection may cover more than one cell.
    #[must_use]
    pub const fn is_range(self) -> bool {
        matches!(self, Self::Range)
    }
}

/// A rectangle of cells, held as the two corners the user made it from.
///
/// The `anchor` is where the selection started and stays put while the `focus`
/// end moves, which is what makes `Shift` and the arrow keys grow and shrink the
/// same rectangle instead of starting a new one. Neither corner is necessarily
/// the top left; [`top_left`](CellRange::top_left) and
/// [`bottom_right`](CellRange::bottom_right) sort that out.
///
/// **A place, not a thing.** Both corners are view coordinates, as
/// [`CellFocus`] is: they say *where* in the grid as it stands, not *which* row
/// of the data. Sorting, filtering or paging therefore leaves the rectangle
/// where it is, covering whatever is now shown there — the same rule the focus
/// follows, and what the user sees highlighted is always what is selected.
///
/// ```
/// use datagrid_core::{CellFocus, CellRange};
///
/// let range = CellRange::single(CellFocus::new(3, 1)).extended_to(CellFocus::new(1, 2));
///
/// // Built upwards and to the right, read as a rectangle.
/// assert_eq!(range.top_left(), CellFocus::new(1, 1));
/// assert_eq!(range.bottom_right(), CellFocus::new(3, 2));
/// assert_eq!(range.cell_count(), 6);
/// assert!(range.contains(CellFocus::new(2, 1)));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellRange {
    /// Where the selection started, and where it stays while it is extended.
    pub anchor: CellFocus,
    /// The moving end: the cell the user last reached.
    pub focus: CellFocus,
}

impl CellRange {
    /// A range covering one cell.
    #[must_use]
    pub const fn single(at: CellFocus) -> Self {
        Self {
            anchor: at,
            focus: at,
        }
    }

    /// The same rectangle with its moving end somewhere else. The anchor stays.
    #[must_use]
    pub const fn extended_to(self, focus: CellFocus) -> Self {
        Self {
            anchor: self.anchor,
            focus,
        }
    }

    /// The corner with the smallest row and column.
    #[must_use]
    pub fn top_left(&self) -> CellFocus {
        CellFocus::new(
            self.anchor.row.min(self.focus.row),
            self.anchor.col.min(self.focus.col),
        )
    }

    /// The corner with the largest row and column.
    #[must_use]
    pub fn bottom_right(&self) -> CellFocus {
        CellFocus::new(
            self.anchor.row.max(self.focus.row),
            self.anchor.col.max(self.focus.col),
        )
    }

    /// The rows it covers, top to bottom.
    #[must_use]
    pub fn rows(&self) -> RangeInclusive<usize> {
        self.top_left().row..=self.bottom_right().row
    }

    /// The columns it covers, start to end.
    #[must_use]
    pub fn columns(&self) -> RangeInclusive<usize> {
        self.top_left().col..=self.bottom_right().col
    }

    /// Whether a cell falls inside the rectangle.
    #[must_use]
    pub fn contains(&self, at: CellFocus) -> bool {
        self.rows().contains(&at.row) && self.columns().contains(&at.col)
    }

    /// How many cells it covers. Never zero: a range always holds its anchor.
    #[must_use]
    pub fn cell_count(&self) -> usize {
        let size = self.bottom_right();
        let start = self.top_left();
        (size.row - start.row + 1) * (size.col - start.col + 1)
    }

    /// Whether it covers more than one cell.
    #[must_use]
    pub fn is_single(&self) -> bool {
        self.anchor == self.focus
    }

    /// The same rectangle pulled inside a grid of `rows` by `cols`.
    ///
    /// A rectangle left over from a larger view — a filter has since narrowed it
    /// — would otherwise point at cells that are not there. An empty grid leaves
    /// it at the origin; there is nothing to point at.
    #[must_use]
    pub fn clamped(self, rows: usize, cols: usize) -> Self {
        let pull = |at: CellFocus| {
            CellFocus::new(
                at.row.min(rows.saturating_sub(1)),
                at.col.min(cols.saturating_sub(1)),
            )
        };
        Self {
            anchor: pull(self.anchor),
            focus: pull(self.focus),
        }
    }
}
