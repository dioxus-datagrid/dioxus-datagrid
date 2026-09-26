//! The `use_grid` hook and the handle it returns.

use crate::Column;
use crate::edit::{EditRows, EditStatus, EditTarget, Editing, Session};
use datagrid_core::{
    CellFocus, ColumnFilter, ColumnId, ColumnSpec, ColumnWidth, DEFAULT_REMOTE_PAGE_SIZE,
    DistinctValues, GridLocale, GridQuery, GridRow, GridState, GroupSpan, NavKey, Pinned, RowSpans,
    Selection, SelectionMode, SortDirection, ValueKind, View, ViewRow, compute_view,
    distinct_values, group_header_rows, group_levels, navigate, reveal_scroll_top,
    rows_per_viewport, visible_range,
};
use dioxus::html::ScrollBehavior;
use dioxus::html::geometry::PixelsVector2D;
use dioxus::prelude::*;
use std::collections::HashSet;
use std::future::Future;
use std::ops::Range;
use std::pin::Pin;
use std::rc::Rc;
use std::time::Duration;

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
    /// The height of a footer that sticks to the bottom of the viewport, such
    /// as the totals, which hides the body rows beneath it.
    pub footer_height: f64,
}

/// Answers a remote grid's value lists: the column, the query without paging,
/// and the most values wanted.
pub(crate) type DistinctSource = Rc<
    dyn Fn(
        ColumnId,
        GridQuery,
        usize,
    ) -> Pin<Box<dyn Future<Output = Result<DistinctValues, String>>>>,
>;

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
    /// For [`use_grid_remote`](crate::use_grid_remote): how long typing in
    /// search or a column filter must pause before a request goes out. `None`
    /// means [`DEFAULT_DEBOUNCE`]; `Some(Duration::ZERO)` sends every keystroke.
    /// Ignored by [`use_grid`], which filters locally.
    pub debounce: Option<Duration>,
    /// The texts and number formats the grid's primitives use. English unless
    /// set; changing it on a later render switches the grid over.
    pub locale: GridLocale,
}

/// How long a remote grid waits for typing to pause before it sends a request.
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(300);

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

    /// Sets how long typing must pause before a remote grid sends a request.
    #[must_use]
    pub fn debounce(mut self, debounce: Duration) -> Self {
        self.debounce = Some(debounce);
        self
    }

    /// Sets the texts and number formats.
    #[must_use]
    pub fn locale(mut self, locale: GridLocale) -> Self {
        self.locale = locale;
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
    /// Whether the pointer has moved since it went down. A press that never
    /// moves is a click or half of a double-click, not a drag.
    moved: bool,
    /// Whether the same handle was pressed, without dragging, right before.
    /// Released without a drag, this press completes a double press.
    repeat: bool,
}

/// A handle to a grid's state and derived view.
///
/// `Copy`, because everything it holds is a signal. Pass it around freely; it is
/// the single thing the primitives need in order to render and to react.
pub struct GridHandle<T: GridRow + 'static> {
    pub(crate) data: ReadSignal<Vec<T>>,
    pub(crate) columns: ReadSignal<Vec<Column<T>>>,
    pub(crate) state: Signal<GridState>,
    pub(crate) selection: Signal<Selection<T::Key>>,
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
    /// The header cells, so their widths can be measured on demand. `onresize`
    /// reaches only the grid root (see `docs/VERIFICATION.md` §13).
    column_elements: Signal<Vec<(ColumnId, Rc<MountedData>)>>,
    /// The column resize in progress, if any.
    resize: Signal<Option<ColumnResize>>,
    /// Set when a resize ends, so the click that follows the release is not
    /// taken for a click on the header underneath.
    resize_click: Signal<bool>,
    /// The column whose handle was last pressed without dragging, so a
    /// double-click can reset it.
    pressed_handle: Signal<Option<ColumnId>>,
    /// Whether a remote grid is waiting for a response. Always `false` locally.
    loading: Signal<bool>,
    /// Why the last remote request failed, until the next one succeeds.
    load_error: Signal<Option<String>>,
    /// Bumped by [`GridHandle::reload`] to repeat the current request.
    /// Texts and formats; see [`GridOptions::locale`].
    pub(crate) locale: Signal<GridLocale>,
    /// Where a remote grid gets value lists from; `None` for a local grid,
    /// which computes them from its rows.
    distinct: CopyValue<Option<DistinctSource>>,
    reload_nonce: Signal<u64>,
    view: Memo<View>,
    /// Whether rows come from a server, which decides how saved edits show.
    pub(crate) remote: bool,
    /// How the grid edits; see [`GridHandle::set_editing`]. Not reactive: it
    /// is set on every render and only read when an edit starts or ends.
    pub(crate) edit_config: Signal<Option<Editing<T>>>,
    /// The edit in progress. Changes with every keystroke, so only editors
    /// read it; cells read [`edit_target`](Self::edit_target).
    pub(crate) edit_session: Signal<Option<Session<T>>>,
    pub(crate) edit_target: Memo<Option<EditTarget>>,
    pub(crate) edit_rows: Signal<EditRows<T>>,
    pub(crate) edit_status: Signal<EditStatus>,
    /// Rows waiting for the user to confirm deleting them.
    pub(crate) edit_confirm: Signal<Option<Vec<T>>>,
    /// The column header being dragged, for a group panel to drop.
    pub(crate) dragged_column: Signal<Option<ColumnId>>,
    /// Whether a footer row with the totals is mounted, which the keyboard
    /// reaches after the last body row.
    pub(crate) footer: Signal<bool>,
    /// Whether a group panel is mounted, which makes column headers draggable.
    pub(crate) group_panel: Signal<bool>,
    /// Whether a column menu add-on is mounted, so headers show its button.
    pub(crate) column_menu: Signal<bool>,
    /// Which column's menu is open, if any. In the handle so a header cell can
    /// open the menu it contains without reaching into it.
    pub(crate) open_column_menu: Signal<Option<ColumnId>>,
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

    let base = use_grid_base(data, columns, options, None);
    let state = base.state;

    // A selected row that is no longer in the data cannot be seen or
    // deselected, so it leaves the selection with its row. Only local grids do
    // this: a remote grid holds one page, and rows on other pages still exist.
    let mut selection = base.selection;
    use_effect(move || {
        let keys: HashSet<T::Key> = data.read().iter().map(GridRow::key).collect();
        let stale = selection.peek().iter().any(|key| !keys.contains(key));
        if stale {
            selection.write().retain_existing(&keys);
        }
    });

    let view = use_memo(move || {
        let rows = data.read();
        let specs: Vec<ColumnSpec<T>> = columns
            .read()
            .iter()
            .map(|column| column.spec().clone())
            .collect();
        compute_view(&rows, &specs, &state.read())
    });

    base.with_view(view)
}

/// Everything a grid owns except the view, which a local grid computes from its
/// rows and a remote grid takes from the server's answer.
pub(crate) struct GridBase<T: GridRow + 'static> {
    pub(crate) data: ReadSignal<Vec<T>>,
    pub(crate) columns: ReadSignal<Vec<Column<T>>>,
    pub(crate) state: Signal<GridState>,
    pub(crate) selection: Signal<Selection<T::Key>>,
    focus: Signal<CellFocus>,
    focus_nonce: Signal<u64>,
    mode: Signal<SelectionMode>,
    focus_pending: Signal<bool>,
    focus_within: Signal<bool>,
    root_focus_is_internal: Signal<bool>,
    root: Signal<Option<Rc<MountedData>>>,
    layout: Signal<Layout>,
    virtual_body: Signal<Option<(f64, usize)>>,
    measured_widths: Signal<Vec<(ColumnId, f64)>>,
    column_elements: Signal<Vec<(ColumnId, Rc<MountedData>)>>,
    resize: Signal<Option<ColumnResize>>,
    resize_click: Signal<bool>,
    pressed_handle: Signal<Option<ColumnId>>,
    pub(crate) loading: Signal<bool>,
    pub(crate) load_error: Signal<Option<String>>,
    pub(crate) reload_nonce: Signal<u64>,
    locale: Signal<GridLocale>,
    distinct: CopyValue<Option<DistinctSource>>,
    remote: bool,
    edit_config: Signal<Option<Editing<T>>>,
    edit_session: Signal<Option<Session<T>>>,
    pub(crate) edit_rows: Signal<EditRows<T>>,
    edit_status: Signal<EditStatus>,
    edit_confirm: Signal<Option<Vec<T>>>,
    dragged_column: Signal<Option<ColumnId>>,
    footer: Signal<bool>,
    group_panel: Signal<bool>,
    column_menu: Signal<bool>,
    open_column_menu: Signal<Option<ColumnId>>,
}

/// Creates the signals shared by local and remote grids, in a fixed hook order.
pub(crate) fn use_grid_base<T>(
    data: ReadSignal<Vec<T>>,
    columns: ReadSignal<Vec<Column<T>>>,
    options: GridOptions,
    distinct: Option<DistinctSource>,
) -> GridBase<T>
where
    T: GridRow + PartialEq + 'static,
{
    let requested_mode = options.selection;
    let requested_page_size = options.page_size;
    let requested_locale = options.locale.clone();

    let mut mode = use_signal(|| requested_mode);
    let mut page_size = use_signal(|| requested_page_size);
    let mut state = use_signal(|| options.into_state());
    let mut selection = use_signal(Selection::new);
    let mut locale = use_signal(|| requested_locale.clone());

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
    if *locale.peek() != requested_locale {
        locale.set(requested_locale);
    }

    let remote = distinct.is_some();
    let edit_session = use_signal(|| None::<Session<T>>);

    GridBase {
        data,
        columns,
        state,
        selection,
        // The tab stop starts on the column headers. With a multi-level header
        // that is not row 0: landing on a group label, which does nothing, is
        // not where a user tabbing in wants to be.
        focus: use_signal(|| {
            let columns = columns.peek();
            let specs: Vec<&ColumnSpec<T>> = columns.iter().map(Column::spec).collect();
            CellFocus::new(group_levels(&specs), 0)
        }),
        focus_nonce: use_signal(|| 0_u64),
        mode,
        focus_pending: use_signal(|| false),
        focus_within: use_signal(|| false),
        root_focus_is_internal: use_signal(|| false),
        root: use_signal(|| None::<Rc<MountedData>>),
        layout: use_signal(Layout::default),
        virtual_body: use_signal(|| None::<(f64, usize)>),
        measured_widths: use_signal(Vec::new),
        column_elements: use_signal(Vec::new),
        resize: use_signal(|| None::<ColumnResize>),
        resize_click: use_signal(|| false),
        pressed_handle: use_signal(|| None::<ColumnId>),
        loading: use_signal(|| false),
        load_error: use_signal(|| None::<String>),
        reload_nonce: use_signal(|| 0_u64),
        locale,
        distinct: use_hook(move || CopyValue::new(distinct)),
        remote,
        edit_config: use_signal(|| None::<Editing<T>>),
        edit_session,
        edit_rows: use_signal(EditRows::default),
        edit_status: use_signal(EditStatus::default),
        edit_confirm: use_signal(|| None::<Vec<T>>),
        dragged_column: use_signal(|| None::<ColumnId>),
        footer: use_signal(|| false),
        group_panel: use_signal(|| false),
        column_menu: use_signal(|| false),
        open_column_menu: use_signal(|| None),
    }
}

impl<T: GridRow + PartialEq> GridBase<T> {
    /// Completes the handle with the view it should render.
    ///
    /// A hook: call it once per render, in the same place.
    pub(crate) fn with_view(self, view: Memo<View>) -> GridHandle<T> {
        let data = self.data;
        let mut edit_session = self.edit_session;

        // Where the edited row is now. By key, so that sorting or a reload
        // while editing moves the editor with its row.
        let edit_target = use_memo(move || {
            let session = edit_session.read();
            let session = session.as_ref()?;
            let row_index = if session.creating {
                None
            } else {
                let rows = data.read();
                view.read()
                    .data_rows()
                    .find(|&(_, index)| rows.get(index).is_some_and(|row| row.key() == session.key))
                    .map(|(position, _)| position)
            };
            Some(session.target(row_index))
        });

        // An edit in the cells whose row left the page cannot be finished
        // there. Dropping it gives the grid its keys back.
        use_effect(move || {
            let lost = edit_target
                .read()
                .as_ref()
                .is_some_and(|target| !target.form && target.row_index.is_none());
            if lost {
                edit_session.set(None);
            }
        });

        GridHandle {
            data: self.data,
            columns: self.columns,
            state: self.state,
            selection: self.selection,
            focus: self.focus,
            focus_nonce: self.focus_nonce,
            mode: self.mode,
            focus_pending: self.focus_pending,
            focus_within: self.focus_within,
            root_focus_is_internal: self.root_focus_is_internal,
            root: self.root,
            layout: self.layout,
            virtual_body: self.virtual_body,
            measured_widths: self.measured_widths,
            column_elements: self.column_elements,
            resize: self.resize,
            resize_click: self.resize_click,
            pressed_handle: self.pressed_handle,
            loading: self.loading,
            load_error: self.load_error,
            reload_nonce: self.reload_nonce,
            locale: self.locale,
            distinct: self.distinct,
            view,
            remote: self.remote,
            edit_config: self.edit_config,
            edit_session: self.edit_session,
            edit_target,
            edit_rows: self.edit_rows,
            edit_status: self.edit_status,
            edit_confirm: self.edit_confirm,
            dragged_column: self.dragged_column,
            footer: self.footer,
            group_panel: self.group_panel,
            column_menu: self.column_menu,
            open_column_menu: self.open_column_menu,
        }
    }
}

impl<T: GridRow> GridHandle<T> {
    /// The texts and number formats this grid uses. Reading it subscribes the
    /// caller, so a component that renders a text re-renders when the locale
    /// changes.
    #[must_use]
    pub fn locale(&self) -> ReadSignal<GridLocale> {
        self.locale.into()
    }

    /// Switches the grid to other texts and number formats.
    ///
    /// For a grid whose locale comes from [`GridOptions::locale`], the next
    /// render with different options switches it again.
    pub fn set_locale(&mut self, locale: GridLocale) {
        self.locale.set(locale);
    }

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
        let state = self.state.read();
        let columns = self.columns.read();
        let declared: Vec<ColumnId> = columns.iter().map(|column| column.id().clone()).collect();

        let mut shown: Vec<Column<T>> = state
            .ordered_columns(&declared)
            .iter()
            .filter_map(|id| columns.iter().find(|column| column.id() == id))
            .filter(|column| column.spec().is_visible(&state.hidden_columns))
            .cloned()
            .collect();

        // Pinned columns are laid out as a block at their edge, so that one can
        // never end up between two scrolling columns — the order within each
        // block is the column order, which a stable sort keeps.
        shown.sort_by_key(|column| match column.spec().effective_pin(&state) {
            Pinned::Start => 0,
            Pinned::None => 1,
            Pinned::End => 2,
        });
        shown
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

    /// Sorts by a column in a given direction, replacing any other sort. What a
    /// menu does, where the direction is picked rather than cycled through.
    pub fn set_sort(&mut self, column: impl Into<ColumnId>, direction: SortDirection) {
        self.state.write().set_sort(column, direction);
    }

    /// Takes a column out of the sort, leaving the others in place.
    pub fn clear_sort(&mut self, column: &ColumnId) {
        self.state.write().clear_sort(column);
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

    /// Sets a column's typed filter, as a filter menu builds it; an empty
    /// filter removes it.
    pub fn set_column_filter(&mut self, column: impl Into<ColumnId>, filter: ColumnFilter) {
        self.state.write().set_column_filter(column, filter);
    }

    /// The typed filter currently set for a column.
    #[must_use]
    pub fn column_filter(&self, column: &ColumnId) -> Option<ColumnFilter> {
        self.state.read().column_filter(column).cloned()
    }

    /// Removes every filter on a column: its filter text and its typed filter.
    pub fn clear_column_filters(&mut self, column: &ColumnId) {
        let mut state = self.state.write();
        state.set_filter(column.clone(), "");
        state.set_column_filter(column.clone(), ColumnFilter::default());
    }

    /// Whether a column is filtered, by text or by a typed filter.
    #[must_use]
    pub fn is_filtered(&self, column: &ColumnId) -> bool {
        self.state.read().is_filtered(column)
    }

    /// What kind of value a column holds, from its declared kind or the rows
    /// at hand. See [`ColumnSpec::value_kind`].
    #[must_use]
    pub fn value_kind(&self, column: &ColumnId) -> Option<ValueKind> {
        let columns = self.columns.read();
        let column = columns.iter().find(|candidate| candidate.id() == column)?;
        column.spec().value_kind(&self.data.read())
    }

    /// The values a value list for `column` offers, with their counts, at most
    /// `limit` of them. See [`distinct_values`].
    ///
    /// A local grid computes them from its rows at once; a remote grid asks its
    /// [`DataSource`](datagrid_core::DataSource). Does not subscribe to
    /// anything, so call it when the list is needed, such as when a menu opens.
    pub fn distinct_values(
        &self,
        column: ColumnId,
        limit: usize,
    ) -> Pin<Box<dyn Future<Output = Result<DistinctValues, String>>>> {
        let state = self.state.peek().clone();
        let remote = self.distinct.peek().clone();
        if let Some(source) = remote {
            let query = GridQuery::from_state(&state, DEFAULT_REMOTE_PAGE_SIZE);
            return source(column, query, limit);
        }

        let specs: Vec<ColumnSpec<T>> = self
            .columns
            .peek()
            .iter()
            .map(|column| column.spec().clone())
            .collect();
        let values =
            distinct_values(&self.data.peek(), &specs, &state, &column, limit).unwrap_or_default();
        Box::pin(async move { Ok(values) })
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

    // -- column order --------------------------------------------------------

    /// Every column the grid declares, in the order it shows them — hidden
    /// columns included, so that unhiding one puts it back where it was.
    #[must_use]
    pub fn column_order(&self) -> Vec<ColumnId> {
        let declared: Vec<ColumnId> = self
            .columns
            .read()
            .iter()
            .map(|column| column.id().clone())
            .collect();
        self.state.read().ordered_columns(&declared)
    }

    /// Sets the column order outright. Ids the grid does not declare are kept
    /// but have no effect; see [`GridState::ordered_columns`].
    pub fn set_column_order(&mut self, order: Vec<ColumnId>) {
        self.state.write().set_column_order(order);
    }

    /// Moves a column so that it sits directly before `before`, or last when
    /// `before` is `None`. What a drop on another header does.
    pub fn move_column_before(&mut self, column: &ColumnId, before: Option<&ColumnId>) {
        let declared: Vec<ColumnId> = self
            .columns
            .read()
            .iter()
            .map(|column| column.id().clone())
            .collect();
        self.state.write().move_column(&declared, column, before);
    }

    /// Moves a column past `steps` of the columns beside it — negative towards
    /// the start — and takes the focus with it. What `Alt+Shift+ArrowLeft` and
    /// `Alt+Shift+ArrowRight` do on a header.
    ///
    /// Hidden columns are stepped over rather than counted, so the move matches
    /// what is on screen. Returns whether anything moved: it does not at the
    /// ends, where the caller should leave the key to the browser.
    pub fn move_column_by(&mut self, column: &ColumnId, steps: isize) -> bool {
        let visible: Vec<ColumnId> = self
            .visible_columns()
            .iter()
            .map(|column| column.id().clone())
            .collect();
        let Some(from) = visible.iter().position(|id| id == column) else {
            return false;
        };

        let last = visible.len().saturating_sub(1);
        let target = from.saturating_add_signed(steps).min(last);
        if target == from {
            return false;
        }

        // Positions shift once the column is taken out, so the neighbour it
        // lands in front of is named rather than counted.
        let before = if target > from {
            visible.get(target + 1)
        } else {
            visible.get(target)
        };
        self.move_column_before(column, before);

        let focus = self.focus();
        if focus.col == from {
            self.set_focus(CellFocus::new(focus.row, target));
        }
        true
    }

    /// Moves the column being dragged next to `target` and ends the drag: it
    /// lands after `target` when it came from the left, before it when it came
    /// from the right, so it ends up where the pointer let go. Returns whether
    /// a column moved.
    ///
    /// What a drop on a header does. A drop on a group panel groups instead;
    /// see [`drop_dragged_column`](GridHandle::drop_dragged_column).
    pub fn drop_dragged_column_at(&mut self, target: &ColumnId) -> bool {
        let Some(dragged) = self.dragged_column() else {
            return false;
        };
        self.end_column_drag();

        let visible: Vec<ColumnId> = self
            .visible_columns()
            .iter()
            .map(|column| column.id().clone())
            .collect();
        let (Some(from), Some(to)) = (
            visible.iter().position(|id| id == &dragged),
            visible.iter().position(|id| id == target),
        ) else {
            return false;
        };
        if from == to {
            return false;
        }

        let before = if from < to {
            visible.get(to + 1)
        } else {
            Some(target)
        };
        self.move_column_before(&dragged, before);
        true
    }

    // -- the header ----------------------------------------------------------

    /// The group header rows above the columns, outermost first, derived from
    /// the visible columns' groups. Empty when no visible column is grouped.
    #[must_use]
    pub fn group_header_rows(&self) -> Vec<Vec<GroupSpan>> {
        let columns = self.visible_columns();
        let specs: Vec<&ColumnSpec<T>> = columns.iter().map(Column::spec).collect();
        group_header_rows(&specs)
    }

    /// How many rows the header takes: the row of column headers, plus one for
    /// each level of column groups above it.
    ///
    /// Rows below it are data rows, which is what
    /// [`focus_data_row`](GridHandle::focus_data_row) and `aria-rowindex` count
    /// from.
    #[must_use]
    pub fn header_rows(&self) -> usize {
        let columns = self.visible_columns();
        let specs: Vec<&ColumnSpec<T>> = columns.iter().map(Column::spec).collect();
        group_levels(&specs) + 1
    }

    // -- cells over several columns ------------------------------------------

    /// Which cell covers each column of the data row at `row_index` on the
    /// current page.
    ///
    /// A column can cover several columns with
    /// [`Column::span`](crate::Column::span), row by row. This resolves what
    /// every column declares for this one row, in the order and the pinning the
    /// columns are laid out in right now. A grid whose columns span nothing â
    /// the usual case â gets a row of single cells without reading a row at
    /// all.
    ///
    /// Read from the stored row, not from an edit in progress: how wide a cell
    /// is should not change under the editor while it is open.
    #[must_use]
    pub fn row_spans(&self, row_index: usize) -> RowSpans {
        let columns = self.visible_columns();
        if columns.iter().all(|column| column.spec().span.is_none()) {
            return RowSpans::none(columns.len());
        }

        let pins: Vec<Pinned> = {
            let state = self.state.read();
            columns
                .iter()
                .map(|column| column.spec().effective_pin(&state))
                .collect()
        };
        let index = self.view().read().data_index(row_index);
        let data = self.data.read();
        let Some(row) = index.and_then(|index| data.get(index)) else {
            return RowSpans::none(columns.len());
        };

        let declared: Vec<(usize, Pinned)> = columns
            .iter()
            .zip(&pins)
            .map(|(column, pin)| (column.spec().span_at(row), *pin))
            .collect();
        RowSpans::resolve(&declared)
    }

    /// The cells of the row the focus coordinates call `row`, whichever kind of
    /// row that is: a group header row above the columns, the column headers
    /// themselves, a group's own row, or a data row.
    ///
    /// What the keyboard needs in order to land on cells rather than on columns
    /// covered by one.
    fn focus_row_spans(&self, row: usize) -> RowSpans {
        let columns = self.visible_columns().len();
        let header_rows = self.header_rows();

        // A group header row: each cell covers the columns it names.
        if row + 1 < header_rows {
            let mut declared = vec![(1, Pinned::None); columns];
            for span in self.group_header_rows().get(row).into_iter().flatten() {
                if let Some(slot) = declared.get_mut(span.start) {
                    slot.0 = span.span;
                }
            }
            return RowSpans::resolve(&declared);
        }

        let Some(row_index) = row.checked_sub(header_rows) else {
            // The column headers themselves; they never span.
            return RowSpans::none(columns);
        };

        match self.row_kind(row_index) {
            // A group's own row is one cell across the whole grid.
            Some(ViewRow::GroupHeader(_)) => {
                let mut declared = vec![(1, Pinned::None); columns];
                if let Some(first) = declared.first_mut() {
                    first.0 = columns;
                }
                RowSpans::resolve(&declared)
            }
            Some(ViewRow::Data(_)) => self.row_spans(row_index),
            _ => RowSpans::none(columns),
        }
    }

    // -- pinned columns ------------------------------------------------------

    /// Where a column is held while the grid scrolls sideways.
    #[must_use]
    pub fn column_pin(&self, column: &ColumnId) -> Pinned {
        let state = self.state.read();
        self.columns
            .read()
            .iter()
            .find(|entry| entry.id() == column)
            .map_or(Pinned::None, |entry| entry.spec().effective_pin(&state))
    }

    /// Pins a column at an edge, or unpins it with [`Pinned::None`]. It moves
    /// to the block of pinned columns at that edge.
    pub fn set_column_pin(&mut self, column: impl Into<ColumnId>, pinned: Pinned) {
        self.state.write().set_pinned(column, pinned);
    }

    /// How far from its edge a pinned column sits: the widths of the pinned
    /// columns between it and the edge. `None` for a column that scrolls.
    ///
    /// A column's own fixed width counts directly; an auto-sized one counts by
    /// what it was last measured at ([`measure_columns`](GridHandle::measure_columns)).
    /// Before the first measurement that is zero, so a column sticks from the
    /// first frame and settles once the grid has been laid out.
    #[must_use]
    pub fn column_pin_offset(&self, column: &ColumnId) -> Option<f64> {
        let pin = self.column_pin(column);
        if !pin.is_pinned() {
            return None;
        }

        let state = self.state.read();
        let measured = self.measured_widths.read();
        let width_of = |column: &Column<T>| match column.spec().effective_width(&state) {
            ColumnWidth::Px(width) => f64::from(width),
            _ => measured
                .iter()
                .find(|(id, _)| id == column.id())
                .map_or(0.0, |(_, width)| *width),
        };

        let shown = self.visible_columns();
        let at = shown.iter().position(|entry| entry.id() == column)?;

        // Towards the start edge the columns before it count, towards the end
        // edge the ones after it — in both cases only the pinned ones.
        let neighbours: Vec<&Column<T>> = match pin {
            Pinned::Start => shown.get(..at).unwrap_or_default().iter().collect(),
            Pinned::End => shown.get(at + 1..).unwrap_or_default().iter().collect(),
            Pinned::None => Vec::new(),
        };
        let offset: f64 = neighbours
            .into_iter()
            .filter(|entry| entry.spec().effective_pin(&state) == pin)
            .map(width_of)
            .sum();
        // The identity of a float sum is negative zero, and the column at the
        // edge would be offset by "-0px".
        Some(if offset == 0.0 { 0.0 } else { offset })
    }

    // -- column widths -------------------------------------------------------

    /// The width to lay a column out with: a width chosen by resizing, clamped
    /// to the column's minimum, or else the column's own width.
    #[must_use]
    pub fn column_width(&self, column: &Column<T>) -> ColumnWidth {
        column.spec().effective_width(&self.state.read())
    }

    /// The visible columns' widths as a CSS `grid-template-columns` value, such
    /// as `"minmax(6rem, auto) 88px 1fr"`.
    ///
    /// An auto-sized column becomes `minmax(<min_width>px, auto)`, or
    /// `auto_track` if it sets no minimum. Laying every row out on this one
    /// track list, as CSS subgrid rows, keeps header and cells aligned without
    /// measuring anything.
    #[must_use]
    pub fn column_template(&self, auto_track: &str) -> String {
        self.visible_columns()
            .iter()
            .map(|column| match self.column_width(column) {
                ColumnWidth::Auto => match column.spec().min_width {
                    Some(min) => format!("minmax({min}px, auto)"),
                    None => auto_track.to_owned(),
                },
                ColumnWidth::Px(width) => format!("{width}px"),
                ColumnWidth::Fraction(fraction) => format!("{fraction}fr"),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The columns a user may show and hide, with their labels and whether
    /// each is visible now. Columns defined as hidden are the app's decision
    /// and are left out.
    #[must_use]
    pub fn pickable_columns(&self) -> Vec<(ColumnId, String, bool)> {
        self.columns
            .read()
            .iter()
            .filter(|column| column.spec().visible)
            .map(|column| {
                let id = column.id().clone();
                let visible = self.is_column_visible(&id);
                (id, column.label().to_owned(), visible)
            })
            .collect()
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

    /// Remembers a column header's element, so its width can be measured later.
    /// Called by the header cell when it mounts.
    pub fn register_column_element(&mut self, column: &ColumnId, element: Rc<MountedData>) {
        let mut elements = self.column_elements.write();
        match elements.iter_mut().find(|(id, _)| id == column) {
            Some(entry) => entry.1 = element,
            None => elements.push((column.clone(), element)),
        }
    }

    /// Measures every column header and records the widths.
    ///
    /// Called from the grid root's `onresize`, which is the one resize that
    /// reaches us: Dioxus observes only the element the listener sits on, and
    /// a header cell's own `onresize` never fires — `docs/VERIFICATION.md` §13.
    /// Measuring from here also means it happens after layout, which a
    /// measurement taken while mounting does not.
    pub fn measure_columns(&mut self) {
        let elements = self.column_elements.peek().clone();
        let mut grid = *self;
        spawn(async move {
            for (column, element) in elements {
                if let Ok(rect) = element.get_client_rect().await {
                    grid.record_column_width(&column, rect.width());
                }
            }
        });
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
                moved: false,
                repeat: self.pressed_handle.peek().as_ref() == Some(column),
            }));
        }
    }

    /// Follows the pointer during a resize. A no-op when none is in progress.
    pub fn update_column_resize(&mut self, client_x: f64) {
        let Some(resize) = self.resize.peek().clone() else {
            return;
        };
        if !resize.moved
            && client_x != resize.start_x
            && let Some(active) = self.resize.write().as_mut()
        {
            active.moved = true;
        }
        let width = resize.start_width + (client_x - resize.start_x);
        #[allow(clippy::cast_possible_truncation)]
        self.set_column_width(&resize.column, width as f32);
    }

    /// Ends the resize in progress, keeping the width it reached.
    pub fn end_column_resize(&mut self) {
        let Some(resize) = self.resize.peek().clone() else {
            return;
        };
        self.resize.set(None);
        if resize.moved {
            // A drag released over a header produces a click there.
            self.resize_click.set(true);
            self.pressed_handle.set(None);
        } else if resize.repeat {
            // Pressed twice without dragging: back to the defined width.
            //
            // Detected here rather than with `dblclick`, which WebKit does not
            // fire when press and release land on different elements — and the
            // release lands on the drag overlay. It also covers a double tap.
            self.pressed_handle.set(None);
            self.reset_column_width(&resize.column);
        } else {
            self.pressed_handle.set(Some(resize.column));
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
        if self.pressed_handle.peek().is_some() {
            self.pressed_handle.set(None);
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

    // -- remote loading --------------------------------------------------------

    /// Whether a remote grid is waiting for a response. Always `false` for a
    /// grid from [`use_grid`].
    ///
    /// The previous page stays on screen while loading, so the grid does not
    /// flicker empty between pages.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        *self.loading.read()
    }

    /// Why the most recent remote request failed, if it did. Cleared by the next
    /// successful response.
    #[must_use]
    pub fn load_error(&self) -> Option<String> {
        self.load_error.read().clone()
    }

    /// Sends the current request again, for instance after an error or when the
    /// data on the server has changed. Does nothing for a local grid.
    pub fn reload(&mut self) {
        *self.reload_nonce.write() += 1;
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
        let index = self.view.read().data_index(row)?;
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

    /// Moves the focus without pulling DOM focus to the cell, for when
    /// something inside it, such as an editor, takes DOM focus instead.
    pub(crate) fn set_focus_quietly(&mut self, focus: CellFocus) {
        if *self.focus.peek() != focus {
            self.focus.set(focus);
        }
        self.complete_focus_pull();
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
        match (self.focus_data_row(), self.rendered_range()) {
            // The header is never virtualized away.
            (None, _) | (_, None) => true,
            // Nor is the footer.
            (Some(body_row), _) if body_row >= self.view.read().len() => true,
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

    /// Records the height of a sticky footer; `0` once it is gone.
    pub fn record_footer_height(&mut self, height: f64) {
        if self.layout.peek().footer_height != height {
            self.layout.write().footer_height = height;
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
    /// sticky header and footer.
    #[must_use]
    pub fn body_viewport_height(&self) -> f64 {
        let layout = self.layout();
        (layout.viewport_height - layout.header_height - layout.footer_height).max(0.0)
    }

    /// Which body rows a virtualized body with this row height and overscan
    /// should render right now.
    #[must_use]
    pub fn virtual_range(&self, row_height: f64, overscan: usize) -> Range<usize> {
        let total = self.view.read().len();
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
        // The header and the footer stay in place; only body rows scroll.
        let Some(body_row) = self
            .focus()
            .row
            .checked_sub(1)
            .filter(|row| *row < self.view.peek().len())
        else {
            return;
        };
        let layout = *self.layout.peek();
        let body_viewport =
            (layout.viewport_height - layout.header_height - layout.footer_height).max(0.0);

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

    /// How many rows the keyboard can reach: the header row, the rows on the
    /// current page, of every kind, and the footer if one is mounted.
    ///
    /// The ARIA grid pattern treats the header as part of the grid, so
    /// [`CellFocus::row`] counts it: row `0` is the header and row `n` is the
    /// `n - 1`-th row of the page. That also lines it up with `aria-rowindex`,
    /// which is 1-based and includes header rows.
    #[must_use]
    pub fn focusable_row_count(&self) -> usize {
        self.view.read().len() + self.header_rows() + usize::from(*self.footer.read())
    }

    /// Whether the focus currently sits on one of the header rows.
    #[must_use]
    pub fn focus_is_header(&self) -> bool {
        self.focus().row < self.header_rows()
    }

    /// The row the focus is on as an index into the page's data rows, or `None`
    /// while it sits on a header row.
    ///
    /// The one place that knows how many rows the header takes, so that the
    /// rest does not have to.
    #[must_use]
    pub fn focus_data_row(&self) -> Option<usize> {
        self.focus().row.checked_sub(self.header_rows())
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
        let focus = self.focus();
        let mut moved = navigate(focus, key, rows, cols, page_rows);
        // Cells, not columns: a cell covering several columns is one stop, and
        // a key that would land inside the cell it started on carries on past
        // it. The row it lands on decides, so this comes after the move.
        moved.col = self.focus_row_spans(moved.row).step(focus.col, moved.col);
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
