//! The state a grid derives its view from.

use crate::{ColumnFilter, ColumnId, SortDirection, SortState};

/// Which page of the filtered rows is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageState {
    /// Zero-based page index. An index past the last page is clamped when the
    /// view is computed; the state itself is left alone.
    pub index: usize,
    /// Rows per page. A size of `0` disables paging for that computation.
    pub size: usize,
}

impl PageState {
    /// Creates a page state pointing at the first page.
    #[must_use]
    pub const fn new(size: usize) -> Self {
        Self { index: 0, size }
    }
}

/// Everything the user can change about how rows are presented.
///
/// This is the unit of persistence: with the `serde` feature enabled it round
/// trips to and from whatever storage the host application uses. It deliberately
/// holds no row data and no closures, only column ids and plain values.
// Not `Eq`: column widths are `f32`, which has no total equality.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct GridState {
    /// Active sort, highest priority first.
    pub sort: Vec<SortState>,
    /// Per-column filter text, as typed in a filter bar. Empty strings are
    /// ignored. A leading operator such as `>100` makes a comparison; see
    /// [`Condition::from_text`](crate::Condition::from_text).
    pub column_filters: Vec<(ColumnId, String)>,
    /// Per-column typed filters, as built by a filter menu. They apply on top
    /// of [`column_filters`](GridState::column_filters): a row must pass both.
    pub filters: Vec<(ColumnId, ColumnFilter)>,
    /// Global search across every visible filterable column.
    pub search: Option<String>,
    /// Paging, or `None` to show every filtered row.
    pub page: Option<PageState>,
    /// Widths set by interactive resizing, in CSS pixels.
    pub column_widths: Vec<(ColumnId, f32)>,
    /// Columns hidden at runtime, on top of
    /// [`ColumnSpec::visible`](crate::ColumnSpec::visible).
    pub hidden_columns: Vec<ColumnId>,
}

impl GridState {
    /// Creates empty state: no sort, no filters, no paging.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates state that pages through the rows.
    #[must_use]
    pub fn paged(page_size: usize) -> Self {
        Self {
            page: Some(PageState::new(page_size)),
            ..Self::default()
        }
    }

    /// The direction a column is currently sorted in, if it is sorted at all.
    #[must_use]
    pub fn sort_direction(&self, column: &ColumnId) -> Option<SortDirection> {
        self.sort
            .iter()
            .find(|entry| &entry.column == column)
            .map(|entry| entry.direction)
    }

    /// The priority of a column within a multi-column sort, if it is sorted.
    ///
    /// `0` is the primary sort column.
    #[must_use]
    pub fn sort_priority(&self, column: &ColumnId) -> Option<usize> {
        self.sort.iter().position(|entry| &entry.column == column)
    }

    /// Cycles a column through ascending, descending and unsorted.
    ///
    /// With `additive` set the column joins an existing multi-column sort;
    /// otherwise it replaces it. Changing the sort resets paging to the first
    /// page, because page 3 of the old order means nothing in the new one.
    pub fn toggle_sort(&mut self, column: impl Into<ColumnId>, additive: bool) {
        let column = column.into();
        let existing = self.sort.iter().position(|entry| entry.column == column);
        let current = existing
            .and_then(|position| self.sort.get(position))
            .map(|entry| entry.direction);

        // unsorted -> ascending -> descending -> unsorted
        let next = match current {
            None => Some(SortDirection::Asc),
            Some(SortDirection::Asc) => Some(SortDirection::Desc),
            Some(SortDirection::Desc) => None,
        };

        if additive {
            // Update in place so that changing a column's direction does not
            // demote it to the end of the sort priority.
            match (existing, next) {
                (Some(position), Some(direction)) => {
                    if let Some(entry) = self.sort.get_mut(position) {
                        entry.direction = direction;
                    }
                }
                (Some(position), None) => {
                    self.sort.remove(position);
                }
                (None, Some(direction)) => self.sort.push(SortState::new(column, direction)),
                (None, None) => {}
            }
        } else {
            self.sort.clear();
            if let Some(direction) = next {
                self.sort.push(SortState::new(column, direction));
            }
        }

        self.reset_page();
    }

    /// Sets the filter text for a column. An empty string removes the filter.
    ///
    /// Resets paging to the first page.
    pub fn set_filter(&mut self, column: impl Into<ColumnId>, text: impl Into<String>) {
        let column = column.into();
        let text = text.into();

        if text.is_empty() {
            self.column_filters.retain(|(id, _)| id != &column);
        } else if let Some(entry) = self.column_filters.iter_mut().find(|(id, _)| *id == column) {
            entry.1 = text;
        } else {
            self.column_filters.push((column, text));
        }

        self.reset_page();
    }

    /// The filter text currently set for a column.
    #[must_use]
    pub fn filter(&self, column: &ColumnId) -> Option<&str> {
        self.column_filters
            .iter()
            .find(|(id, _)| id == column)
            .map(|(_, text)| text.as_str())
    }

    /// Sets a column's typed filter. An empty filter removes it.
    ///
    /// Resets paging to the first page.
    pub fn set_column_filter(&mut self, column: impl Into<ColumnId>, filter: ColumnFilter) {
        let column = column.into();
        if filter.is_empty() {
            self.filters.retain(|(id, _)| id != &column);
        } else if let Some(entry) = self.filters.iter_mut().find(|(id, _)| *id == column) {
            entry.1 = filter;
        } else {
            self.filters.push((column, filter));
        }
        self.reset_page();
    }

    /// The typed filter currently set for a column.
    #[must_use]
    pub fn column_filter(&self, column: &ColumnId) -> Option<&ColumnFilter> {
        self.filters
            .iter()
            .find(|(id, _)| id == column)
            .map(|(_, filter)| filter)
    }

    /// Whether a column is filtered, by text or by a typed filter.
    #[must_use]
    pub fn is_filtered(&self, column: &ColumnId) -> bool {
        self.filter(column).is_some_and(|text| !text.is_empty())
            || self.column_filter(column).is_some()
    }

    /// Sets the global search term. An empty string clears it.
    ///
    /// Resets paging to the first page.
    pub fn set_search(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.search = if text.is_empty() { None } else { Some(text) };
        self.reset_page();
    }

    /// Jumps to a page. Out-of-range indices are clamped when the view is
    /// computed, so this never fails.
    pub fn set_page(&mut self, index: usize) {
        if let Some(page) = self.page.as_mut() {
            page.index = index;
        }
    }

    /// Turns paging on with the given page size, or off with `None`.
    ///
    /// Returns to the first page when the size actually changes, because the old
    /// index points at different rows under a different size.
    pub fn set_page_size(&mut self, size: Option<usize>) {
        let current = self.page.map(|page| page.size);
        if current == size {
            return;
        }
        self.page = size.map(PageState::new);
    }

    /// Shows or hides a column at runtime.
    pub fn set_column_hidden(&mut self, column: impl Into<ColumnId>, hidden: bool) {
        let column = column.into();
        if hidden {
            if !self.hidden_columns.contains(&column) {
                self.hidden_columns.push(column);
            }
        } else {
            self.hidden_columns.retain(|id| id != &column);
        }
    }

    /// Records an interactively chosen column width, in CSS pixels.
    ///
    /// A non-finite width is ignored: it would be no usable layout, and JSON
    /// has no representation for it, so the state would stop round tripping.
    /// Clamping to the column's minimum happens when the width is applied, in
    /// [`ColumnSpec::effective_width`](crate::ColumnSpec::effective_width).
    pub fn set_column_width(&mut self, column: impl Into<ColumnId>, width: f32) {
        if !width.is_finite() {
            return;
        }
        let column = column.into();
        if let Some(entry) = self.column_widths.iter_mut().find(|(id, _)| *id == column) {
            entry.1 = width;
        } else {
            self.column_widths.push((column, width));
        }
    }

    /// Forgets an interactively chosen width, so the column falls back to its
    /// own [`width`](crate::ColumnSpec::width).
    pub fn reset_column_width(&mut self, column: &ColumnId) {
        self.column_widths.retain(|(id, _)| id != column);
    }

    /// The interactively chosen width for a column, if any.
    #[must_use]
    pub fn column_width(&self, column: &ColumnId) -> Option<f32> {
        self.column_widths
            .iter()
            .find(|(id, _)| id == column)
            .map(|(_, width)| *width)
    }

    /// Returns to the first page, if paging is enabled.
    fn reset_page(&mut self) {
        if let Some(page) = self.page.as_mut() {
            page.index = 0;
        }
    }
}
