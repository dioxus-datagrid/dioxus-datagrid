//! The `use_grid` hook and the handle it returns.

use crate::Column;
use datagrid_core::{
    CellFocus, ColumnId, ColumnSpec, GridRow, GridState, NavKey, Selection, SelectionMode,
    SortDirection, View, compute_view, navigate,
};
use dioxus::prelude::*;

/// How a grid behaves, passed once to [`use_grid`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GridOptions {
    /// Rows per page, or `None` to show every row.
    pub page_size: Option<usize>,
    /// Whether and how rows can be selected.
    pub selection: SelectionMode,
    /// State to start from — a persisted [`GridState`], for instance.
    ///
    /// Takes precedence over [`page_size`](GridOptions::page_size).
    pub initial_state: Option<GridState>,
}

impl GridOptions {
    /// Options for a paged grid.
    #[must_use]
    pub fn paged(page_size: usize) -> Self {
        Self {
            page_size: Some(page_size),
            ..Self::default()
        }
    }

    /// Sets the selection mode.
    #[must_use]
    pub fn selection(mut self, mode: SelectionMode) -> Self {
        self.selection = mode;
        self
    }

    /// Starts from previously persisted state.
    #[must_use]
    pub fn initial_state(mut self, state: GridState) -> Self {
        self.initial_state = Some(state);
        self
    }

    /// The state a grid with these options starts in.
    fn into_state(self) -> GridState {
        self.initial_state.unwrap_or_else(|| match self.page_size {
            Some(size) => GridState::paged(size),
            None => GridState::new(),
        })
    }
}

/// A source of rows or columns for [`use_grid`].
///
/// Dioxus 0.7 has no `From<T>` for `ReadSignal<T>`, so this bridges the gap and
/// lets the hook take either a signal or a plain value.
///
/// A plain `Vec` is captured once, on first render — which is what you want for
/// the usual `use_hook(|| vec![...])` column list, and is why data that changes
/// should be passed as a signal.
pub trait IntoReadSignal<T: 'static> {
    /// Produces the signal the grid will read from.
    ///
    /// Called inside a hook, so implementations may create a signal.
    fn into_read_signal(self) -> ReadSignal<T>;
}

impl<T: 'static> IntoReadSignal<Vec<T>> for Vec<T> {
    fn into_read_signal(self) -> ReadSignal<Vec<T>> {
        ReadSignal::new(Signal::new(self))
    }
}

impl<T: 'static> IntoReadSignal<T> for ReadSignal<T> {
    fn into_read_signal(self) -> Self {
        self
    }
}

impl<T: 'static> IntoReadSignal<T> for Signal<T> {
    fn into_read_signal(self) -> ReadSignal<T> {
        ReadSignal::new(self)
    }
}

impl<T: PartialEq + 'static> IntoReadSignal<T> for Memo<T> {
    fn into_read_signal(self) -> ReadSignal<T> {
        ReadSignal::new(self)
    }
}

/// A handle to a grid's state and derived view.
///
/// `Copy`, because everything it holds is a signal. Pass it around freely; it is
/// the single thing the primitives need in order to render and to react.
pub struct GridHandle<T: GridRow + 'static> {
    data: ReadSignal<Vec<T>>,
    columns: ReadSignal<Vec<Column<T>>>,
    state: Signal<GridState>,
    selection: Signal<Selection<T::Key>>,
    focus: Signal<CellFocus>,
    /// Bumped whenever the focus is moved deliberately, so the newly focused
    /// cell knows to pull DOM focus to itself. Without it every re-render would
    /// drag focus back into the grid.
    focus_nonce: Signal<u64>,
    mode: SelectionMode,
    view: Memo<View>,
}

impl<T: GridRow> Clone for GridHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: GridRow> Copy for GridHandle<T> {}

impl<T: GridRow> PartialEq for GridHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state && self.view == other.view && self.focus == other.focus
    }
}

/// Creates a grid over `data` and `columns`.
///
/// The returned [`GridHandle`] owns the sort, filter, paging, selection and
/// focus state, and recomputes the [`View`] whenever any of it changes.
///
/// ```
/// use datagrid_core::GridRow;
/// use dioxus::prelude::*;
/// use dioxus_datagrid::{Column, GridOptions, use_grid};
///
/// #[derive(Clone, PartialEq)]
/// struct User {
///     id: u32,
///     name: String,
/// }
///
/// impl GridRow for User {
///     type Key = u32;
///     fn key(&self) -> u32 {
///         self.id
///     }
/// }
///
/// #[component]
/// fn Demo() -> Element {
///     let users = use_signal(Vec::<User>::new);
///     let columns = use_hook(|| {
///         vec![
///             Column::new("name", "Name")
///                 .cell(|user: &User| rsx! { "{user.name}" })
///                 .sort_by_text(|user: &User| user.name.as_str()),
///         ]
///     });
///
///     let grid = use_grid(users, columns, GridOptions::paged(25));
///     rsx! { "{grid.filtered_len()} rows" }
/// }
/// ```
pub fn use_grid<T>(
    data: impl IntoReadSignal<Vec<T>> + 'static,
    columns: impl IntoReadSignal<Vec<Column<T>>> + 'static,
    options: GridOptions,
) -> GridHandle<T>
where
    T: GridRow + PartialEq + 'static,
{
    // Converted inside hooks so that a plain `Vec` becomes a signal exactly
    // once, rather than a fresh one on every render.
    let data = use_hook(move || data.into_read_signal());
    let columns = use_hook(move || columns.into_read_signal());
    let mode = options.selection;

    let state = use_signal(|| options.into_state());
    let selection = use_signal(Selection::new);
    let focus = use_signal(CellFocus::default);
    let focus_nonce = use_signal(|| 0_u64);

    let view = use_memo(move || {
        let rows = data.read();
        let specs: Vec<ColumnSpec<T>> = columns
            .read()
            .iter()
            .map(|column| column.spec().clone())
            .collect();
        compute_view(&rows, &specs, &state.read())
    });

    GridHandle {
        data,
        columns,
        state,
        selection,
        focus,
        focus_nonce,
        mode,
        view,
    }
}

impl<T: GridRow> GridHandle<T> {
    /// The rows the grid was given, unfiltered and unsorted.
    #[must_use]
    pub fn data(&self) -> ReadSignal<Vec<T>> {
        self.data
    }

    /// The column definitions.
    #[must_use]
    pub fn columns(&self) -> ReadSignal<Vec<Column<T>>> {
        self.columns
    }

    /// The current view: which rows to draw, in which order.
    #[must_use]
    pub fn view(&self) -> Memo<View> {
        self.view
    }

    /// The indices of the rows on the current page, in display order.
    #[must_use]
    pub fn visible_indices(&self) -> Vec<usize> {
        self.view.read().indices.clone()
    }

    /// The rows on the current page, in display order.
    ///
    /// Clones each row. For a paged grid that is one page worth of clones; where
    /// that matters, use [`visible_indices`](GridHandle::visible_indices) and
    /// read from [`data`](GridHandle::data) instead.
    #[must_use]
    pub fn visible_rows(&self) -> Vec<T> {
        let rows = self.data.read();
        self.view
            .read()
            .indices
            .iter()
            .filter_map(|&index| rows.get(index).cloned())
            .collect()
    }

    /// How many rows survived filtering, across every page.
    #[must_use]
    pub fn filtered_len(&self) -> usize {
        self.view.read().filtered_len
    }

    /// How many pages the filtered rows span; `0` when paging is off.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.view.read().page_count
    }

    /// The current page index, or `None` when paging is off.
    #[must_use]
    pub fn page(&self) -> Option<usize> {
        self.state.read().page.map(|page| page.index)
    }

    /// The columns that are visible right now, in order.
    ///
    /// Position within this list is what `aria-colindex` counts and what the
    /// keyboard navigation moves through.
    #[must_use]
    pub fn visible_columns(&self) -> Vec<Column<T>> {
        let hidden = self.state.read().hidden_columns.clone();
        self.columns
            .read()
            .iter()
            .filter(|column| column.spec().is_visible(&hidden))
            .cloned()
            .collect()
    }

    /// How many columns are visible.
    #[must_use]
    pub fn visible_column_count(&self) -> usize {
        let hidden = self.state.read().hidden_columns.clone();
        self.columns
            .read()
            .iter()
            .filter(|column| column.spec().is_visible(&hidden))
            .count()
    }

    // -- state ---------------------------------------------------------------

    /// A snapshot of the current state, suitable for persisting.
    #[must_use]
    pub fn state(&self) -> GridState {
        self.state.read().clone()
    }

    /// The state signal itself, for callers that want to react to it.
    #[must_use]
    pub fn state_signal(&self) -> Signal<GridState> {
        self.state
    }

    /// Replaces the whole state, for instance when restoring it from storage.
    pub fn set_state(&mut self, state: GridState) {
        self.state.set(state);
    }

    /// Cycles a column through ascending, descending and unsorted.
    ///
    /// With `additive` the column joins an existing multi-column sort instead of
    /// replacing it.
    pub fn toggle_sort(&mut self, column: impl Into<ColumnId>, additive: bool) {
        self.state.write().toggle_sort(column, additive);
    }

    /// The direction a column is sorted in, if it is sorted at all.
    #[must_use]
    pub fn sort_direction(&self, column: &ColumnId) -> Option<SortDirection> {
        self.state.read().sort_direction(column)
    }

    /// The priority of a column within a multi-column sort; `0` is primary.
    #[must_use]
    pub fn sort_priority(&self, column: &ColumnId) -> Option<usize> {
        self.state.read().sort_priority(column)
    }

    /// Sets a column's filter text; an empty string clears it.
    pub fn set_filter(&mut self, column: impl Into<ColumnId>, text: impl Into<String>) {
        self.state.write().set_filter(column, text);
    }

    /// The filter text currently set for a column.
    #[must_use]
    pub fn filter(&self, column: &ColumnId) -> Option<String> {
        self.state.read().filter(column).map(ToOwned::to_owned)
    }

    /// Sets the global search term; an empty string clears it.
    pub fn set_search(&mut self, text: impl Into<String>) {
        self.state.write().set_search(text);
    }

    /// The current search term.
    #[must_use]
    pub fn search(&self) -> Option<String> {
        self.state.read().search.clone()
    }

    /// Jumps to a page. Out-of-range indices are clamped when the view is
    /// recomputed, so this cannot fail.
    pub fn set_page(&mut self, index: usize) {
        self.state.write().set_page(index);
    }

    /// Moves to the next page if there is one.
    pub fn next_page(&mut self) {
        if let Some(current) = self.page() {
            let last = self.page_count().saturating_sub(1);
            self.set_page(current.saturating_add(1).min(last));
        }
    }

    /// Moves to the previous page if there is one.
    pub fn previous_page(&mut self) {
        if let Some(current) = self.page() {
            self.set_page(current.saturating_sub(1));
        }
    }

    /// Shows or hides a column at runtime.
    pub fn set_column_hidden(&mut self, column: impl Into<ColumnId>, hidden: bool) {
        self.state.write().set_column_hidden(column, hidden);
    }

    // -- selection -----------------------------------------------------------

    /// How rows can be selected.
    #[must_use]
    pub fn selection_mode(&self) -> SelectionMode {
        self.mode
    }

    /// Whether a row is selected.
    #[must_use]
    pub fn is_selected(&self, key: &T::Key) -> bool {
        self.selection.read().contains(key)
    }

    /// How many rows are selected.
    #[must_use]
    pub fn selected_count(&self) -> usize {
        self.selection.read().len()
    }

    /// The selected keys, in arbitrary order.
    #[must_use]
    pub fn selected_keys(&self) -> Vec<T::Key> {
        self.selection.read().iter().cloned().collect()
    }

    /// Selects a row, replacing the selection in single-selection mode.
    pub fn select(&mut self, key: T::Key) {
        let mode = self.mode;
        self.selection.write().select(key, mode);
    }

    /// Toggles a row's selection.
    pub fn toggle_select(&mut self, key: T::Key) {
        let mode = self.mode;
        self.selection.write().toggle(key, mode);
    }

    /// Extends the selection from the anchor to `key`, in display order.
    pub fn extend_select(&mut self, key: T::Key) {
        let mode = self.mode;
        let ordered = self.visible_keys();
        self.selection.write().extend_to(&ordered, key, mode);
    }

    /// Clears the selection.
    pub fn clear_selection(&mut self) {
        self.selection.write().clear();
    }

    /// The keys of the rows on the current page, in display order.
    #[must_use]
    pub fn visible_keys(&self) -> Vec<T::Key> {
        let rows = self.data.read();
        self.view
            .read()
            .indices
            .iter()
            .filter_map(|&index| rows.get(index))
            .map(GridRow::key)
            .collect()
    }

    /// The key of the row at a position within the current page.
    #[must_use]
    pub fn key_at(&self, row: usize) -> Option<T::Key> {
        let rows = self.data.read();
        let index = *self.view.read().indices.get(row)?;
        rows.get(index).map(GridRow::key)
    }

    // -- focus ---------------------------------------------------------------

    /// Which cell currently holds focus, in view coordinates.
    #[must_use]
    pub fn focus(&self) -> CellFocus {
        *self.focus.read()
    }

    /// Moves the focus directly, without the keyboard.
    pub fn set_focus(&mut self, focus: CellFocus) {
        self.focus.set(focus);
        *self.focus_nonce.write() += 1;
    }

    /// How many rows the keyboard can reach: the header row, plus the rows on
    /// the current page.
    ///
    /// The ARIA grid pattern treats the header as part of the grid, so
    /// [`CellFocus::row`] counts it: row `0` is the header and row `n` is the
    /// `n - 1`-th row of the page. That also lines it up with `aria-rowindex`,
    /// which is 1-based and includes header rows.
    #[must_use]
    pub fn focusable_row_count(&self) -> usize {
        self.view.read().indices.len() + 1
    }

    /// Whether the focus currently sits on the header row.
    #[must_use]
    pub fn focus_is_header(&self) -> bool {
        self.focus().row == 0
    }

    /// Applies a navigation key to the focus.
    pub fn move_focus(&mut self, key: NavKey) {
        let rows = self.focusable_row_count();
        let cols = self.visible_column_count();
        // Paging already limits how many rows are on screen, so a page key moves
        // across the whole page. Phase 4 will narrow this to the virtual window.
        let page_rows = rows.max(1);
        let moved = navigate(self.focus(), key, rows, cols, page_rows);
        self.set_focus(moved);
    }

    /// Counts how often the focus has been moved deliberately.
    ///
    /// A cell watches this to tell "I am focused because the user just navigated
    /// here" from "I am focused and the grid merely re-rendered", so that
    /// re-renders do not steal focus back from elsewhere on the page.
    #[must_use]
    pub fn focus_nonce(&self) -> u64 {
        *self.focus_nonce.read()
    }
}
