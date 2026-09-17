//! The `use_grid` hook and the handle it returns.

use crate::Column;
use datagrid_core::{
    CellFocus, ColumnId, ColumnSpec, ColumnWidth, GridRow, GridState, NavKey, Selection,
    SelectionMode, SortDirection, View, compute_view, navigate, reveal_scroll_top,
    rows_per_viewport, visible_range,
};
use dioxus::html::ScrollBehavior;
use dioxus::html::geometry::PixelsVector2D;
use dioxus::prelude::*;
use std::ops::Range;
use std::rc::Rc;

/// The measured geometry of the grid's scroll container, in CSS pixels.
///
/// Kept up to date by [`GridRoot`](crate::primitives::GridRoot) from `onscroll`
/// and `onresize`; nothing here comes from `web-sys`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layout {
    /// How far the container is scrolled down.
    pub scroll_top: f64,
    /// How far the container is scrolled right. Tracked because scrolling
    /// programmatically sets both axes at once.
    pub scroll_left: f64,
    /// The container's visible height.
    pub viewport_height: f64,
    /// The height of the header row group, which sticks to the top of the
    /// viewport and hides whatever body rows are beneath it.
    pub header_height: f64,
}

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

/// How far one key press resizes a column, in CSS pixels.
pub const COLUMN_RESIZE_STEP: f32 = 16.0;

/// A column resize in progress: where the pointer went down and how wide the
/// column was at that moment. Every move is measured from here rather than
/// accumulated, so dropped or coalesced pointer events cannot make it drift.
#[derive(Clone, Debug, PartialEq)]
struct ColumnResize {
    column: ColumnId,
    start_x: f64,
    start_width: f64,
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
    /// A signal rather than a plain value so that a change reaches every copy of
    /// the handle. As a plain field, cells rendered before the change would keep
    /// click handlers that still select in the old mode.
    mode: Signal<SelectionMode>,
    /// Set while a focus move is waiting for its cell to take DOM focus. A cell
    /// that mounts later — because the move scrolled it into the rendered range —
    /// still claims focus, but a cell that merely remounts during ordinary
    /// scrolling does not.
    focus_pending: Signal<bool>,
    /// Whether DOM focus is somewhere inside the grid.
    focus_within: Signal<bool>,
    /// Set just before the grid focuses its own root, so the root's focus handler
    /// can tell that apart from a user tabbing in.
    root_focus_is_internal: Signal<bool>,
    root: Signal<Option<Rc<MountedData>>>,
    layout: Signal<Layout>,
    /// Row height and overscan of a mounted virtualized body.
    ///
    /// Stored as configuration rather than as the rendered range, so the range is
    /// always derived from the current scroll position. A stored range lags a
    /// render behind a scroll, and during that lag a freshly focused row counts
    /// as not rendered — enough for the grid to park focus on its root right
    /// after the cell took it.
    virtual_body: Signal<Option<(f64, usize)>>,
    /// Rendered header cell widths, as reported by their `onresize`. A drag
    /// starts from the width the column actually has, which for an `Auto` or
    /// `Fraction` column only layout knows.
    measured_widths: Signal<Vec<(ColumnId, f64)>>,
    /// The column resize in progress, if any.
    resize: Signal<Option<ColumnResize>>,
    /// Set when a resize ends, so the click that follows the release is not
    /// taken for a click on the header underneath.
    resize_click: Signal<bool>,
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
        self.state == other.state
            && self.view == other.view
            && self.focus == other.focus
            && self.mode == other.mode
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

    let requested_mode = options.selection;
    let requested_page_size = options.page_size;

    let mut mode = use_signal(|| requested_mode);
    let mut page_size = use_signal(|| requested_page_size);
    let mut state = use_signal(|| options.into_state());
    let mut selection = use_signal(Selection::new);
    let focus = use_signal(CellFocus::default);
    let focus_nonce = use_signal(|| 0_u64);
    let focus_pending = use_signal(|| false);
    let focus_within = use_signal(|| false);
    let root_focus_is_internal = use_signal(|| false);
    let root = use_signal(|| None::<Rc<MountedData>>);
    let layout = use_signal(Layout::default);
    let virtual_body = use_signal(|| None::<(f64, usize)>);
    let measured_widths = use_signal(Vec::new);
    let resize = use_signal(|| None::<ColumnResize>);
    let resize_click = use_signal(|| false);

    // Options are plain values, so a component re-rendering with different ones
    // would otherwise be ignored after the first render. This is the pattern
    // `use_reactive` uses internally: compare against the last seen value and
    // write only on an actual change.
    if *mode.peek() != requested_mode {
        mode.set(requested_mode);
        // A multi-row selection is not valid in single or no-selection mode, and
        // silently keeping part of it would be arbitrary.
        selection.write().clear();
    }
    if *page_size.peek() != requested_page_size {
        page_size.set(requested_page_size);
        state.write().set_page_size(requested_page_size);
    }

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
        focus_pending,
        focus_within,
        root_focus_is_internal,
        root,
        layout,
        virtual_body,
        measured_widths,
        resize,
        resize_click,
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
    ///
    /// Refuses to hide the last visible column: a grid without columns has no
    /// cell to hold focus and would drop out of the tab order entirely.
    pub fn set_column_hidden(&mut self, column: impl Into<ColumnId>, hidden: bool) {
        let column = column.into();
        if hidden && self.is_column_visible(&column) && self.visible_column_count() <= 1 {
            return;
        }
        self.state.write().set_column_hidden(column, hidden);
    }

    /// Whether a column is currently shown, taking both its definition and the
    /// runtime state into account. Unknown ids are not visible.
    #[must_use]
    pub fn is_column_visible(&self, column: &ColumnId) -> bool {
        let state = self.state.read();
        self.columns
            .read()
            .iter()
            .any(|entry| entry.id() == column && entry.spec().is_visible(&state.hidden_columns))
    }

    // -- column widths -------------------------------------------------------

    /// The width to lay a column out with: a width chosen by resizing, clamped
    /// to the column's minimum, or else the column's own width.
    #[must_use]
    pub fn column_width(&self, column: &Column<T>) -> ColumnWidth {
        column.spec().effective_width(&self.state.read())
    }

    /// Sets a column's width, clamped to its minimum. Ignores unknown columns
    /// and columns that are not resizable.
    pub fn set_column_width(&mut self, column: &ColumnId, width: f32) {
        let clamped = {
            let columns = self.columns.peek();
            let Some(spec) = columns
                .iter()
                .map(Column::spec)
                .find(|spec| &spec.id == column && spec.resizable)
            else {
                return;
            };
            spec.clamp_width(width)
        };
        if self.state.peek().column_width(column) != Some(clamped) {
            self.state.write().set_column_width(column.clone(), clamped);
        }
    }

    /// Returns a column to the width it was defined with.
    pub fn reset_column_width(&mut self, column: &ColumnId) {
        if self.state.peek().column_width(column).is_some() {
            self.state.write().reset_column_width(column);
        }
    }

    /// Records the rendered width of a column's header cell.
    pub fn record_column_width(&mut self, column: &ColumnId, width: f64) {
        let known = self
            .measured_widths
            .peek()
            .iter()
            .find(|(id, _)| id == column)
            .map(|(_, measured)| *measured);
        match known {
            Some(measured) if measured == width => {}
            Some(_) => {
                if let Some(entry) = self
                    .measured_widths
                    .write()
                    .iter_mut()
                    .find(|(id, _)| id == column)
                {
                    entry.1 = width;
                }
            }
            None => self.measured_widths.write().push((column.clone(), width)),
        }
    }

    /// The width a column has right now: the one set by resizing, else the one
    /// last measured, else its fixed width. `None` before an `Auto` or
    /// `Fraction` column has been laid out.
    #[must_use]
    pub fn current_column_width(&self, column: &ColumnId) -> Option<f64> {
        let defined = self
            .columns
            .peek()
            .iter()
            .find(|entry| entry.id() == column)
            .map(|entry| entry.spec().effective_width(&self.state.peek()));
        // A resized or fixed width is authoritative; a measurement may still
        // describe the layout from before it was applied.
        if let Some(ColumnWidth::Px(width)) = defined {
            return Some(f64::from(width));
        }
        self.measured_widths
            .peek()
            .iter()
            .find(|(id, _)| id == column)
            .map(|(_, width)| *width)
    }

    /// Widens (positive `delta`) or narrows a column by `delta` pixels, starting
    /// from its current width. The keyboard alternative to dragging.
    pub fn resize_column_by(&mut self, column: &ColumnId, delta: f32) {
        if let Some(width) = self.current_column_width(column) {
            #[allow(clippy::cast_possible_truncation)]
            self.set_column_width(column, width as f32 + delta);
        }
    }

    /// Starts resizing a column from a pointer at `client_x`.
    ///
    /// Does nothing for a column that is not resizable or not yet laid out.
    pub fn start_column_resize(&mut self, column: &ColumnId, client_x: f64) {
        let resizable = self
            .columns
            .peek()
            .iter()
            .any(|entry| entry.id() == column && entry.spec().resizable);
        if !resizable {
            return;
        }
        if let Some(start_width) = self.current_column_width(column) {
            self.resize.set(Some(ColumnResize {
                column: column.clone(),
                start_x: client_x,
                start_width,
            }));
        }
    }

    /// Follows the pointer during a resize. A no-op when none is in progress.
    pub fn update_column_resize(&mut self, client_x: f64) {
        let Some(resize) = self.resize.peek().clone() else {
            return;
        };
        let width = resize.start_width + (client_x - resize.start_x);
        #[allow(clippy::cast_possible_truncation)]
        self.set_column_width(&resize.column, width as f32);
    }

    /// Ends the resize in progress, keeping the width it reached.
    pub fn end_column_resize(&mut self) {
        if self.resize.peek().is_some() {
            self.resize.set(None);
            self.resize_click.set(true);
        }
    }

    /// Whether the click being handled is the one that ended a resize, clearing
    /// the flag. A header checks this before sorting.
    pub fn take_resize_click(&mut self) -> bool {
        let pending = *self.resize_click.peek();
        if pending {
            self.resize_click.set(false);
        }
        pending
    }

    /// Discards a resize click that never arrived, because the pointer was
    /// released somewhere without a click handler. Called on the next press.
    pub fn forget_resize_click(&mut self) {
        if *self.resize_click.peek() {
            self.resize_click.set(false);
        }
    }

    /// The column being resized right now, if any.
    #[must_use]
    pub fn resizing_column(&self) -> Option<ColumnId> {
        self.resize
            .read()
            .as_ref()
            .map(|resize| resize.column.clone())
    }

    // -- selection -----------------------------------------------------------

    /// How rows can be selected.
    #[must_use]
    pub fn selection_mode(&self) -> SelectionMode {
        *self.mode.read()
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
        let mode = *self.mode.read();
        self.selection.write().select(key, mode);
    }

    /// Toggles a row's selection.
    pub fn toggle_select(&mut self, key: T::Key) {
        let mode = *self.mode.read();
        self.selection.write().toggle(key, mode);
    }

    /// Extends the selection from the anchor to `key`, in display order.
    pub fn extend_select(&mut self, key: T::Key) {
        let mode = *self.mode.read();
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
    ///
    /// Clamped to the cells that exist right now. Filtering can remove the
    /// focused row and hiding a column can remove the focused column; without
    /// the clamp no cell would carry `tabindex="0"` and the grid would drop out
    /// of the tab order.
    #[must_use]
    pub fn focus(&self) -> CellFocus {
        let focus = *self.focus.read();
        let last_row = self.focusable_row_count().saturating_sub(1);
        let last_col = self.visible_column_count().saturating_sub(1);
        CellFocus::new(focus.row.min(last_row), focus.col.min(last_col))
    }

    /// Moves the focus directly, without the keyboard.
    pub fn set_focus(&mut self, focus: CellFocus) {
        self.focus.set(focus);
        self.request_focus_pull();
    }

    /// Asks the focused cell to take DOM focus, without moving the focus.
    pub fn request_focus_pull(&mut self) {
        self.focus_pending.set(true);
        *self.focus_nonce.write() += 1;
    }

    /// Whether a focus move is still waiting for its cell to take DOM focus.
    #[must_use]
    pub fn focus_pending(&self) -> bool {
        *self.focus_pending.read()
    }

    /// Marks the pending focus move as done. Called by the cell that took focus.
    pub fn complete_focus_pull(&mut self) {
        if *self.focus_pending.peek() {
            self.focus_pending.set(false);
        }
    }

    /// Records whether DOM focus is inside the grid.
    pub fn set_focus_within(&mut self, within: bool) {
        if *self.focus_within.peek() != within {
            self.focus_within.set(within);
        }
    }

    /// Whether DOM focus is inside the grid.
    #[must_use]
    pub fn focus_within(&self) -> bool {
        *self.focus_within.read()
    }

    /// Whether the focused cell currently exists in the DOM.
    ///
    /// Always true for a non-virtualized grid. For a virtualized one, the focused
    /// row may have scrolled out of the rendered window, in which case the grid
    /// root stands in as the tab stop.
    #[must_use]
    pub fn focus_is_rendered(&self) -> bool {
        let row = self.focus().row;
        match (row.checked_sub(1), self.rendered_range()) {
            // The header is never virtualized away.
            (None, _) | (_, None) => true,
            (Some(body_row), Some(range)) => range.contains(&body_row),
        }
    }

    // -- layout and virtualization -----------------------------------------------

    /// The measured scroll container geometry.
    #[must_use]
    pub fn layout(&self) -> Layout {
        *self.layout.read()
    }

    /// Records a scroll event from the grid's scroll container.
    pub fn record_scroll(&mut self, scroll_top: f64, scroll_left: f64, viewport_height: f64) {
        let current = *self.layout.peek();
        let next = Layout {
            scroll_top,
            scroll_left,
            viewport_height,
            ..current
        };
        if next != current {
            self.layout.set(next);
        }
    }

    /// Records the scroll container's visible height.
    pub fn record_viewport_height(&mut self, height: f64) {
        if self.layout.peek().viewport_height != height {
            self.layout.write().viewport_height = height;
        }
    }

    /// Records the height of the sticky header.
    pub fn record_header_height(&mut self, height: f64) {
        if self.layout.peek().header_height != height {
            self.layout.write().header_height = height;
        }
    }

    /// Remembers the scroll container, so the grid can scroll and focus it.
    pub fn set_root(&mut self, root: Rc<MountedData>) {
        self.root.set(Some(root));
    }

    /// The fixed row height of a mounted virtualized body, if there is one.
    #[must_use]
    pub fn row_height(&self) -> Option<f64> {
        (*self.virtual_body.read()).map(|(row_height, _)| row_height)
    }

    /// Registers a virtualized body's row height and overscan, or clears them
    /// with `None` when it unmounts.
    pub fn set_virtual_body(&mut self, config: Option<(f64, usize)>) {
        if *self.virtual_body.peek() != config {
            self.virtual_body.set(config);
        }
    }

    /// The body rows in the DOM, or `None` when every row is rendered.
    ///
    /// Derived from the current scroll position on every call, so it cannot lag
    /// behind a scroll the way a stored range would.
    #[must_use]
    pub fn rendered_range(&self) -> Option<Range<usize>> {
        let (row_height, overscan) = (*self.virtual_body.read())?;
        Some(self.virtual_range(row_height, overscan))
    }

    /// The part of the viewport that shows body rows: its height minus the
    /// sticky header.
    #[must_use]
    pub fn body_viewport_height(&self) -> f64 {
        let layout = self.layout();
        (layout.viewport_height - layout.header_height).max(0.0)
    }

    /// Which body rows a virtualized body with this row height and overscan
    /// should render right now.
    #[must_use]
    pub fn virtual_range(&self, row_height: f64, overscan: usize) -> Range<usize> {
        let total = self.view.read().indices.len();
        let scroll_top = self.layout().scroll_top;
        visible_range(
            scroll_top,
            self.body_viewport_height(),
            row_height,
            total,
            overscan,
        )
    }

    /// Scrolls the focused row into view, if the grid is virtualized and it is
    /// not already fully visible.
    ///
    /// Updates the recorded scroll position immediately rather than waiting for
    /// the scroll event, so the row is rendered in the same pass and its cell can
    /// take focus straight away.
    pub fn reveal_focus(&mut self) {
        let Some((row_height, _)) = *self.virtual_body.peek() else {
            return;
        };
        let Some(body_row) = self.focus().row.checked_sub(1) else {
            return;
        };
        let layout = *self.layout.peek();
        let body_viewport = (layout.viewport_height - layout.header_height).max(0.0);

        let Some(scroll_top) =
            reveal_scroll_top(body_row, row_height, body_viewport, layout.scroll_top)
        else {
            return;
        };

        self.layout.write().scroll_top = scroll_top;
        if let Some(root) = self.root.peek().clone() {
            let target = PixelsVector2D::new(layout.scroll_left, scroll_top);
            spawn(async move {
                // Instant, not smooth: with a key held down, a smooth scroll would
                // lag behind the rows being rendered for the new position.
                let _ = root.scroll(target, ScrollBehavior::Instant).await;
            });
        }
    }

    /// Moves DOM focus to the grid root, without it counting as the user
    /// entering the grid.
    pub fn focus_root(&mut self) {
        if let Some(root) = self.root.peek().clone() {
            self.root_focus_is_internal.set(true);
            spawn(async move {
                let _ = root.set_focus(true).await;
            });
        }
    }

    /// Whether the root's most recent focus came from [`focus_root`], clearing
    /// the flag.
    ///
    /// [`focus_root`]: GridHandle::focus_root
    pub fn take_internal_root_focus(&mut self) -> bool {
        let internal = *self.root_focus_is_internal.peek();
        if internal {
            self.root_focus_is_internal.set(false);
        }
        internal
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
        // A virtualized grid pages by what fits in the viewport. Otherwise paging
        // already limits the rows on screen, so a page key crosses the whole page.
        let page_rows = match (*self.virtual_body.peek()).map(|(row_height, _)| row_height) {
            Some(row_height) => rows_per_viewport(self.body_viewport_height(), row_height),
            None => rows.max(1),
        };
        let moved = navigate(self.focus(), key, rows, cols, page_rows);
        self.set_focus(moved);
        self.reveal_focus();
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
