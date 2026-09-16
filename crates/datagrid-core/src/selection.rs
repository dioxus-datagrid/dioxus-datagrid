//! Row selection, including shift-range selection.

use std::collections::HashSet;
use std::hash::Hash;

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
        if let Some(anchor) = &self.anchor {
            if !keys.contains(anchor) {
                self.anchor = None;
            }
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
