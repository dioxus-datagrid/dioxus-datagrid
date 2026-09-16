//! Unstyled primitives implementing the WAI-ARIA data grid pattern.
//!
//! Every primitive takes the [`GridHandle`] and spreads any extra attributes
//! onto its element, so callers bring their own classes and styling. See
//! `docs/ACCESSIBILITY.md` for the roles, attributes and key bindings.
//!
//! These are deliberately not re-exported at the crate root: the row component
//! is called `GridRow`, which would collide with the
//! [`GridRow`](datagrid_core::GridRow) trait you implement on your row type.

use crate::GridHandle;
use datagrid_core::{
    CellFocus, GridRow as GridRowKey, NavKey, SelectionMode, SortDirection, offset_of, total_height,
};
use dioxus::prelude::*;
use std::rc::Rc;

/// Translates a key press into a navigation key, if it is one.
fn nav_key(event: &KeyboardData) -> Option<NavKey> {
    let ctrl = event.modifiers().ctrl() || event.modifiers().meta();
    Some(match event.key() {
        Key::ArrowUp => NavKey::Up,
        Key::ArrowDown => NavKey::Down,
        Key::ArrowLeft => NavKey::Left,
        Key::ArrowRight => NavKey::Right,
        Key::Home if ctrl => NavKey::CtrlHome,
        Key::End if ctrl => NavKey::CtrlEnd,
        Key::Home => NavKey::Home,
        Key::End => NavKey::End,
        Key::PageUp => NavKey::PageUp,
        Key::PageDown => NavKey::PageDown,
        _ => return None,
    })
}

/// The grid container: `role="grid"` plus the keyboard handling.
///
/// Owns the key bindings for the whole grid, because the roving tabindex keeps
/// focus on a descendant cell and the events bubble up to here.
#[component]
pub fn GridRoot<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let mut grid = grid;
    // `aria-rowcount` spans every page, not just the rendered one, and counts
    // the header row — that is the whole point of the attribute when the grid
    // is paged.
    let row_count = grid.filtered_len() + 1;
    let column_count = grid.visible_column_count();

    let onkeydown = move |event: Event<KeyboardData>| {
        let data = event.data();
        let shift = data.modifiers().shift();

        if let Some(key) = nav_key(&data) {
            if shift && grid.selection_mode() == SelectionMode::Multi && !grid.focus_is_header() {
                // Shift+Arrow extends the selection as it moves.
                grid.move_focus(key);
                if let Some(target) = grid.focus().row.checked_sub(1) {
                    if let Some(row_key) = grid.key_at(target) {
                        grid.extend_select(row_key);
                    }
                }
            } else {
                grid.move_focus(key);
            }
            event.prevent_default();
            return;
        }

        let focus = grid.focus();
        match data.key() {
            // On a header, both Enter and Space sort. Shift makes it additive so
            // a second column can join the sort.
            Key::Enter | Key::Character(_) if focus.row == 0 => {
                if data.key() != Key::Enter && data.key() != Key::Character(" ".into()) {
                    return;
                }
                let columns = grid.visible_columns();
                if let Some(column) = columns.get(focus.col) {
                    if column.is_sortable() {
                        grid.toggle_sort(column.id().clone(), shift);
                        event.prevent_default();
                    }
                }
            }
            // On a body row, Space selects and Shift+Space extends.
            Key::Character(ref character) if character == " " => {
                if grid.selection_mode() == SelectionMode::None {
                    return;
                }
                if let Some(target) = focus.row.checked_sub(1) {
                    if let Some(row_key) = grid.key_at(target) {
                        if shift {
                            grid.extend_select(row_key);
                        } else {
                            grid.toggle_select(row_key);
                        }
                        event.prevent_default();
                    }
                }
            }
            _ => {}
        }
    };

    // In a virtualized grid the focused row can scroll out of the DOM. If it had
    // DOM focus, the browser drops focus to the page, and the next arrow key would
    // go nowhere. Catch that and park focus on the root, which carries the key
    // bindings and, when the user navigates, scrolls back to the focused row.
    //
    // Not while a focus move is pending: that move is about to bring its cell into
    // the DOM and hand it focus, and parking now would snatch focus straight back.
    use_effect(move || {
        if grid.focus_within() && !grid.focus_pending() && !grid.focus_is_rendered() {
            grid.focus_root();
        }
    });

    // The root is the tab stop only while the focused cell is not in the DOM;
    // otherwise that cell is, and the root must not add a second one. It stays
    // programmatically focusable either way.
    let root_tabindex = if grid.focus_is_rendered() { "-1" } else { "0" };

    rsx! {
        div {
            role: "grid",
            aria_rowcount: "{row_count}",
            aria_colcount: "{column_count}",
            aria_multiselectable: matches!(grid.selection_mode(), SelectionMode::Multi).then_some("true"),
            tabindex: root_tabindex,
            onmounted: move |event| grid.set_root(event.data()),
            onscroll: move |event| {
                let data = event.data();
                grid.record_scroll(data.scroll_top(), data.scroll_left(), f64::from(data.client_height()));
            },
            onresize: move |event| {
                if let Ok(size) = event.data().get_content_box_size() {
                    grid.record_viewport_height(size.height);
                }
            },
            onfocusin: move |_| grid.set_focus_within(true),
            onfocusout: move |_| grid.set_focus_within(false),
            onfocus: move |_| {
                // Our own parking move is not the user entering the grid.
                if grid.take_internal_root_focus() {
                    return;
                }
                // Tabbed in while the focused row was scrolled away: bring it
                // back and hand focus to its cell.
                grid.reveal_focus();
                grid.request_focus_pull();
            },
            onkeydown,
            ..attributes,
            {children}
        }
    }
}

/// The header row group, rendering one [`GridHeaderCell`] per visible column.
#[component]
pub fn GridHeader<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let columns = grid.visible_columns();

    rsx! {
        div {
            role: "rowgroup",
            // A virtualized body needs to know how much of the viewport the
            // sticky header covers.
            onresize: move |event| {
                if let Ok(size) = event.data().get_border_box_size() {
                    grid.record_header_height(size.height);
                }
            },
            ..attributes,
            div { role: "row", aria_rowindex: "1",
                for (index , column) in columns.into_iter().enumerate() {
                    GridHeaderCell {
                        key: "{column.id()}",
                        grid,
                        column_index: index,
                    }
                }
            }
        }
    }
}

/// One column header: `role="columnheader"`, with `aria-sort` when sortable.
#[component]
pub fn GridHeaderCell<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position among the visible columns, zero-based.
    column_index: usize,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let columns = grid.visible_columns();
    let Some(column) = columns.get(column_index).cloned() else {
        return rsx! {};
    };

    let id = column.id().clone();
    let sortable = column.is_sortable();
    let direction = grid.sort_direction(&id);
    let priority = grid.sort_priority(&id);

    // Only the sorted column carries aria-sort; "none" on a sortable column that
    // is not sorted tells assistive technology that sorting is available.
    let aria_sort = match (sortable, direction) {
        (true, Some(SortDirection::Asc)) => Some("ascending"),
        (true, Some(SortDirection::Desc)) => Some("descending"),
        (true, None) => Some("none"),
        (false, _) => None,
    };

    let focused = grid.focus() == CellFocus::new(0, column_index);
    let onmounted = use_focus_pull(grid, CellFocus::new(0, column_index));

    let onclick = move |event: MouseEvent| {
        if !sortable {
            return;
        }
        grid.set_focus(CellFocus::new(0, column_index));
        grid.toggle_sort(id.clone(), event.modifiers().shift());
    };

    rsx! {
        div {
            role: "columnheader",
            aria_colindex: "{column_index + 1}",
            aria_sort,
            tabindex: if focused { "0" } else { "-1" },
            "data-sortable": "{sortable}",
            "data-sorted": match direction {
                Some(SortDirection::Asc) => "ascending",
                Some(SortDirection::Desc) => "descending",
                None => "none",
            },
            "data-sort-priority": priority.map(|value| value.to_string()),
            onmounted,
            onclick,
            ..attributes,
            {column.render_header()}
        }
    }
}

/// The body row group, rendering one [`GridRow`] per row on the current page.
#[component]
pub fn GridBody<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let row_count = grid.view().read().indices.len();

    rsx! {
        div { role: "rowgroup", ..attributes,
            for row_index in 0..row_count {
                GridRow { key: "{row_index}", grid, row_index }
            }
        }
    }
}

/// A body row group that renders only the rows in and near the viewport.
///
/// A drop-in replacement for [`GridBody`] for large data sets. Rows outside the
/// window are represented by padding above and below, so the scrollbar still
/// reflects every row.
///
/// # Requirements
///
/// - **Every row is exactly `row_height` pixels tall.** The primitive sets the
///   height on each row; content that does not fit is clipped. Variable row
///   heights are not supported.
/// - **[`GridRoot`] is the scroll container.** Give it a height and
///   `overflow: auto`. It reports scroll position and size to the grid.
/// - **The header is sticky**, or absent. Rows beneath a sticky header are
///   treated as hidden; a header that scrolls away with the rows would make the
///   window lag behind by the header's height.
///
/// Keyboard navigation scrolls the focused row into view, and `PageUp` and
/// `PageDown` move by one viewport. `aria-rowindex` stays correct for every
/// rendered row because it counts from the full view, not the window.
#[component]
pub fn VirtualGridBody<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// The fixed height of every row, in CSS pixels.
    row_height: f64,
    /// Extra rows rendered above and below the viewport, so fast scrolling does
    /// not expose blank space before the next render.
    #[props(default = 5)]
    overscan: usize,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;

    // Registered so keyboard navigation can page by viewport, scroll rows into
    // view, and tell whether the focused row is in the DOM. The setter compares
    // first, since an unconditional write during render would loop.
    grid.set_virtual_body(Some((row_height, overscan)));
    use_drop(move || grid.set_virtual_body(None));

    // A memo, so a scroll event that stays within the same rows — most of them —
    // does not re-render the body.
    let range = use_memo(use_reactive!(
        |row_height, overscan| grid.virtual_range(row_height, overscan)
    ));

    let total = grid.view().read().indices.len();
    let window = range();
    let (start, end) = (window.start, window.end);
    let padding_top = offset_of(start, row_height);
    let padding_bottom = total_height(total.saturating_sub(end), row_height);

    rsx! {
        div {
            role: "rowgroup",
            style: "padding-top: {padding_top}px; padding-bottom: {padding_bottom}px;",
            "data-virtual-start": "{start}",
            "data-virtual-end": "{end}",
            ..attributes,
            for row_index in start..end {
                GridRow {
                    key: "{row_index}",
                    grid,
                    row_index,
                    style: "height: {row_height}px; box-sizing: border-box; overflow: hidden;",
                }
            }
        }
    }
}

/// One row: `role="row"`, with `aria-rowindex` and `aria-selected`.
#[component]
pub fn GridRow<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position within the current page, zero-based.
    row_index: usize,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let columns = grid.visible_columns();
    let key = grid.key_at(row_index);

    // aria-rowindex is 1-based over the whole filtered set and counts the header
    // row, so page 2 of a 25-row page starts at 27.
    let page_offset = grid
        .state()
        .page
        .map_or(0, |page| page.index.saturating_mul(page.size));
    let aria_row_index = page_offset + row_index + 2;

    let selectable = grid.selection_mode() != SelectionMode::None;
    let selected = key.as_ref().is_some_and(|key| grid.is_selected(key));

    rsx! {
        div {
            role: "row",
            aria_rowindex: "{aria_row_index}",
            aria_selected: selectable.then_some(if selected { "true" } else { "false" }),
            "data-selected": "{selected}",
            ..attributes,
            for column_index in 0..columns.len() {
                GridCell {
                    key: "{column_index}",
                    grid,
                    row_index,
                    column_index,
                }
            }
        }
    }
}

/// One cell: `role="gridcell"`, carrying the roving tabindex.
#[component]
pub fn GridCell<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position within the current page, zero-based.
    row_index: usize,
    /// Position among the visible columns, zero-based.
    column_index: usize,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let columns = grid.visible_columns();
    let Some(column) = columns.get(column_index).cloned() else {
        return rsx! {};
    };

    // Focus coordinates count the header as row 0.
    let focus_row = row_index + 1;
    let focused = grid.focus() == CellFocus::new(focus_row, column_index);
    let onmounted = use_focus_pull(grid, CellFocus::new(focus_row, column_index));

    let content = {
        let data = grid.data();
        let rows = data.read();
        let index = grid.view().read().indices.get(row_index).copied();
        match index.and_then(|index| rows.get(index)) {
            Some(row) => column.render_cell(row),
            None => rsx! {},
        }
    };

    let onclick = move |_| {
        grid.set_focus(CellFocus::new(focus_row, column_index));
        if grid.selection_mode() != SelectionMode::None {
            if let Some(key) = grid.key_at(row_index) {
                grid.select(key);
            }
        }
    };

    rsx! {
        div {
            role: "gridcell",
            aria_colindex: "{column_index + 1}",
            tabindex: if focused { "0" } else { "-1" },
            onmounted,
            onclick,
            ..attributes,
            {content}
        }
    }
}

/// Pulls DOM focus onto the cell at `at` while a focus move is pending.
///
/// The roving tabindex alone only decides what `Tab` reaches; after an arrow key
/// the browser still has focus on the previous cell, so the newly focused cell
/// has to claim it. It does so only while the grid reports a pending move, which
/// keeps ordinary re-renders — and rows remounting as a virtualized grid scrolls
/// — from yanking focus away from, say, the search box.
///
/// Returns the `onmounted` handler for the cell. A cell that a focus move
/// scrolled into the rendered window mounts after the move, so it has to claim
/// focus on mount as well as from the effect.
fn use_focus_pull<T: GridRowKey + 'static>(
    grid: GridHandle<T>,
    at: CellFocus,
) -> impl FnMut(MountedEvent) + 'static {
    let mut element = use_signal(|| None::<Rc<MountedData>>);

    use_effect(move || {
        // Read, so the effect re-runs whenever the focus or the pending flag
        // changes. The coordinate comes from the signal rather than a value
        // captured at first render.
        let should_take = grid.focus() == at && grid.focus_pending();
        let _ = grid.focus_nonce();
        if !should_take {
            return;
        }
        // Peeked, not read: subscribing to the element would re-run this on
        // every mount.
        if let Some(target) = element.peek().clone() {
            let mut grid = grid;
            grid.complete_focus_pull();
            spawn(async move {
                let _ = target.set_focus(true).await;
            });
        }
    });

    move |event: MountedEvent| {
        let target = event.data();
        element.set(Some(target.clone()));
        let mut grid = grid;
        if grid.focus() == at && grid.focus_pending() {
            grid.complete_focus_pull();
            spawn(async move {
                let _ = target.set_focus(true).await;
            });
        }
    }
}

/// Page controls for a paged grid. Renders nothing when paging is off.
#[component]
pub fn GridPagination<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let Some(page) = grid.page() else {
        return rsx! {};
    };
    let page_count = grid.page_count();
    let last = page_count.saturating_sub(1);

    rsx! {
        nav {
            aria_label: "Pagination",
            "data-page": "{page}",
            "data-page-count": "{page_count}",
            ..attributes,
            button {
                r#type: "button",
                disabled: page == 0,
                aria_label: "Previous page",
                onclick: move |_| grid.previous_page(),
                "Previous"
            }
            span {
                role: "status",
                aria_live: "polite",
                "Page {page + 1} of {page_count.max(1)}"
            }
            button {
                r#type: "button",
                disabled: page >= last,
                aria_label: "Next page",
                onclick: move |_| grid.next_page(),
                "Next"
            }
        }
    }
}

/// A search box bound to the grid's global search.
#[component]
pub fn GridSearch<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Placeholder text for the input.
    #[props(default = String::from("Search"))]
    placeholder: String,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let value = grid.search().unwrap_or_default();

    rsx! {
        input {
            r#type: "search",
            role: "searchbox",
            aria_label: "{placeholder}",
            placeholder: "{placeholder}",
            value: "{value}",
            oninput: move |event| grid.set_search(event.value()),
            ..attributes,
        }
    }
}

/// A text input bound to one column's filter.
#[component]
pub fn GridColumnFilter<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position among the visible columns, zero-based.
    column_index: usize,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let columns = grid.visible_columns();
    let Some(column) = columns.get(column_index).cloned() else {
        return rsx! {};
    };

    let id = column.id().clone();
    let label = format!("Filter {}", column.label());
    let value = grid.filter(&id).unwrap_or_default();

    rsx! {
        input {
            r#type: "text",
            aria_label: "{label}",
            value: "{value}",
            oninput: move |event| grid.set_filter(id.clone(), event.value()),
            ..attributes,
        }
    }
}
