//! Unstyled primitives implementing the WAI-ARIA data grid pattern.
//!
//! Every primitive takes the [`GridHandle`] and spreads any extra attributes
//! onto its element, so callers bring their own classes and styling. See
//! `docs/ACCESSIBILITY.md` for the roles, attributes and key bindings.
//!
//! These are deliberately not re-exported at the crate root: the row component
//! is called `GridRow`, which would collide with the
//! [`GridRow`](datagrid_core::GridRow) trait you implement on your row type.

pub use crate::column_menu::{GridColumnMenu, column_menu_entries};
pub use crate::edit_ui::{
    GridCellEditor, GridDeleteConfirm, GridEditDialog, GridEditStatus, GridEditToolbar,
};
pub use crate::filter_menu::{DEFAULT_VALUE_LIMIT, GridFilterMenu};
use crate::group::GroupKeyPress;
pub use crate::group_ui::{
    AggregateSource, GridAggregateCell, GridFooter, GridGroupFooterRow, GridGroupPanel,
    GridGroupRow,
};
use crate::{COLUMN_RESIZE_STEP, GridHandle};
use datagrid_core::{
    CellFocus, GridRow as GridRowKey, GroupSpan, NavKey, Pinned, SelectionMode, SortDirection,
    ViewRow, offset_of, total_height,
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
    // What the clipboard holds reaches a page as a paste event, which the
    // document hears rather than the grid: one listener per grid, for as long as
    // the grid is there (`docs/DECISIONS.md` ADR-0033).
    crate::paste::use_paste_listener(grid);
    // `aria-rowcount` spans every page, not just the rendered one, and counts
    // the header rows — that is the whole point of the attribute when the grid
    // is paged.
    let row_count =
        grid.view().read().row_count + grid.header_rows() + usize::from(grid.has_footer());
    let column_count = grid.visible_column_count();
    // Groups make rows nest, which is what a treegrid is for.
    let role = if grid.is_grouped() {
        "treegrid"
    } else {
        "grid"
    };

    let onkeydown = move |event: Event<KeyboardData>| {
        // An editor handles its own keys; the grid's would fight them.
        if grid.is_editing() {
            return;
        }
        let data = event.data();
        let shift = data.modifiers().shift();

        // On a group's header, the arrows across expand and collapse it.
        let plain = !shift && !data.modifiers().ctrl() && !data.modifiers().alt();
        let group_press = match data.key() {
            Key::ArrowRight if plain => Some(GroupKeyPress::Expand),
            Key::ArrowLeft if plain => Some(GroupKeyPress::Collapse),
            Key::Enter if plain => Some(GroupKeyPress::Toggle),
            Key::Character(ref character) if plain && character == " " => {
                Some(GroupKeyPress::Toggle)
            }
            _ => None,
        };
        if group_press.is_some_and(|press| grid.group_key(press)) {
            event.prevent_default();
            return;
        }

        if let Some(key) = nav_key(&data) {
            let cells = grid.cell_selection_mode();
            grid.move_focus(key);
            let in_body = !grid.focus_is_header();

            // The cell selection follows the focus: `Shift` grows the rectangle
            // from its anchor, anything else starts over at the cell reached.
            if cells.is_enabled() && in_body {
                if shift && cells.is_range() {
                    grid.extend_cell_selection(grid.focus());
                } else {
                    grid.select_cell(grid.focus());
                }
            }

            // Rows get `Shift+Arrow` only where the cells did not take it: a
            // grid that selects rectangles is one its user reads as a sheet.
            if shift
                && !cells.is_range()
                && grid.selection_mode() == SelectionMode::Multi
                && in_body
                && let Some(target) = grid.focus_data_row()
                && let Some(row_key) = grid.key_at(target)
            {
                grid.extend_select(row_key);
            }
            event.prevent_default();
            return;
        }

        // Copy, before the single-character keys below: `c` with a modifier is
        // not the `c` that types into a cell. `Cmd` on macOS, `Ctrl` elsewhere.
        if (data.modifiers().ctrl() || data.modifiers().meta())
            && data.key() == Key::Character("c".into())
        {
            grid.copy_selection();
            event.prevent_default();
            return;
        }

        let focus = grid.focus();
        match data.key() {
            // On a header, both Enter and Space sort. Shift makes it additive so
            // a second column can join the sort.
            Key::Enter | Key::Character(_) if grid.focus_is_header() => {
                if data.key() != Key::Enter && data.key() != Key::Character(" ".into()) {
                    return;
                }
                let columns = grid.visible_columns();
                if let Some(column) = columns.get(focus.col)
                    && column.is_sortable()
                {
                    grid.toggle_sort(column.id().clone(), shift);
                    event.prevent_default();
                }
            }
            // On a body cell, Enter or F2 starts editing it, if it can be.
            Key::Enter | Key::F2 if !grid.focus_is_header() => {
                if grid
                    .focus_data_row()
                    .is_some_and(|row| grid.start_edit(row, focus.col))
                {
                    event.prevent_default();
                }
            }
            // Delete asks to delete the focused row, or the selection it is
            // part of.
            Key::Delete
                if grid.can_delete()
                    && grid
                        .focus_data_row()
                        .is_some_and(|row| grid.key_at(row).is_some()) =>
            {
                let keys = grid.delete_candidates();
                grid.request_delete(&keys);
                event.prevent_default();
            }
            // On a body row, Space selects and Shift+Space extends.
            Key::Character(ref character) if character == " " => {
                if grid.selection_mode() == SelectionMode::None {
                    return;
                }
                if let Some(target) = grid.focus_data_row()
                    && let Some(row_key) = grid.key_at(target)
                {
                    if shift {
                        grid.extend_select(row_key);
                    } else {
                        grid.toggle_select(row_key);
                    }
                    event.prevent_default();
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

    // Column widths, measured after the DOM is committed rather than while the
    // headers mount: at mount the stylesheet may not have been applied and a
    // header still spans the container. Re-runs whenever the view changes,
    // which covers the columns changing; `onresize` covers the window.
    use_effect(move || {
        let _ = grid.view().read().len();
        let _ = grid.visible_column_count();
        grid.measure_columns();
    });

    // The root is the tab stop only while the focused cell is not in the DOM;
    // otherwise that cell is, and the root must not add a second one. It stays
    // programmatically focusable either way.
    let root_tabindex = if grid.focus_is_rendered() { "-1" } else { "0" };

    rsx! {
        div {
            role,
            aria_rowcount: "{row_count}",
            aria_colcount: "{column_count}",
            aria_multiselectable: (matches!(grid.selection_mode(), SelectionMode::Multi)
                || grid.cell_selection_mode().is_range())
                .then_some("true"),
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
                // The grid's own resize is the one that fires, so the columns
                // are measured from here rather than each from its own header.
                grid.measure_columns();
            },
            // A remote grid keeps showing the previous page while the next loads;
            // aria-busy tells assistive technology the content is about to change.
            aria_busy: grid.is_loading().then_some("true"),
            "data-resizing": grid.resizing_column().is_some().then_some("true"),
            // Column resizing: a ColumnResizeHandle starts the drag, the root
            // follows it, because the pointer leaves a narrow handle at once.
            onpointermove: move |event| {
                if grid.resizing_column().is_none() {
                    return;
                }
                // Released outside the grid, where the pointerup never reached us.
                if event.data().held_buttons().is_empty() {
                    grid.end_column_resize();
                    return;
                }
                grid.update_column_resize(event.data().client_coordinates().x);
            },
            onpointerdown: move |_| grid.forget_resize_click(),
            onpointerup: move |_| grid.end_column_resize(),
            onpointercancel: move |_| grid.end_column_resize(),
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
            // While a column is resized, a transparent layer over the whole
            // window catches the pointer, so the drag keeps going when it leaves
            // the grid — widening the last column always does. It is a child of
            // the root, so its events bubble to the handlers above. This stands in
            // for pointer capture, which Dioxus does not expose (ADR-0018).
            if grid.resizing_column().is_some() {
                div {
                    "data-resize-overlay": "",
                    aria_hidden: "true",
                    style: "position: fixed; inset: 0; z-index: 2147483647; cursor: col-resize; touch-action: none;",
                    // A fast double press can press again before the overlay is
                    // gone. That press belongs to the handle's gesture, so it must
                    // not reach the root and forget which handle was pressed.
                    onpointerdown: move |event: PointerEvent| event.stop_propagation(),
                }
            }
        }
    }
}

/// The header row group, rendering one [`GridHeaderCell`] per visible column.
#[component]
pub fn GridHeader<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Renders a [`ColumnResizeHandle`] in every resizable column's header.
    #[props(default)]
    resizable: bool,
    /// Lets headers be dragged onto one another, and moved with
    /// `Alt+Shift+ArrowLeft` / `Alt+Shift+ArrowRight`, to change the column
    /// order.
    #[props(default)]
    reorderable: bool,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let columns = grid.visible_columns();
    // One row per level of column groups, then the columns themselves.
    let groups = grid.group_header_rows();
    let leaf_index = groups.len() + 1;

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
            for (level , spans) in groups.into_iter().enumerate() {
                div {
                    key: "group-{level}",
                    role: "row",
                    aria_rowindex: "{level + 1}",
                    "data-group-header-row": "{level}",
                    for span in spans {
                        GridGroupHeaderCell {
                            key: "{level}-{span.start}",
                            grid,
                            level,
                            span,
                        }
                    }
                }
            }
            div { role: "row", aria_rowindex: "{leaf_index}",
                for (index , column) in columns.into_iter().enumerate() {
                    GridHeaderCell {
                        key: "{column.id()}",
                        grid,
                        column_index: index,
                        resizable,
                        reorderable,
                    }
                }
            }
        }
    }
}

/// One cell of a group header row: a label over the columns it covers.
///
/// Rendered by [`GridHeader`] for every level of column groups, derived from the
/// columns' own groups. A cell with no label stands above a column that has no
/// group at that level, so that every header row covers every column and
/// `aria-colindex` keeps counting straight.
///
/// Carries `aria-colspan` when it covers more than one column, and
/// `data-group-header` for styling. It holds the roving tabindex for any of its
/// columns, so arrowing up from a column header lands on the group above it —
/// and, being one cell, it is one stop: arrowing across goes to the next group.
#[component]
pub fn GridGroupHeaderCell<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Which group header row this is, outermost first.
    level: usize,
    /// The columns it covers, and what to call them.
    span: GroupSpan,
) -> Element {
    let mut grid = grid;
    let covered = span.columns();
    let start = span.start;

    let focus = grid.focus();
    let focused = focus.row == level && span.covers(focus.col);
    let width = span.span;
    let onmounted = use_focus_pull_where(grid, move |focus| {
        focus.row == level && focus.col >= start && focus.col < start + width
    });

    // A group over pinned columns has to be held too, or it would scroll away
    // from the columns it names. Only when they agree: a group that straddles
    // the edge of the pinned block belongs to neither side.
    let columns = grid.visible_columns();
    let pins: Vec<Pinned> = covered
        .clone()
        .filter_map(|index| columns.get(index))
        .map(|column| grid.column_pin(column.id()))
        .collect();
    let pin = match pins.first() {
        Some(first) if pins.iter().all(|pin| pin == first) => *first,
        _ => Pinned::None,
    };
    let pin_offset = columns
        .get(start)
        .filter(|_| pin.is_pinned())
        .and_then(|column| grid.column_pin_offset(column.id()));

    rsx! {
        div {
            role: "columnheader",
            aria_colindex: "{start + 1}",
            aria_colspan: (span.span > 1).then(|| span.span.to_string()),
            tabindex: if focused { "0" } else { "-1" },
            "data-group-header": "{level}",
            "data-pinned": pin.as_str(),
            style: format!(
                "grid-column: span {};{}",
                span.span,
                pin_offset.map_or(String::new(), |offset| format!(" --dg-pin-offset: {offset}px;")),
            ),
            onmounted,
            onclick: move |_| grid.set_focus(CellFocus::new(level, start)),
            {span.label.clone().unwrap_or_default()}
        }
    }
}

/// One column header: `role="columnheader"`, with `aria-sort` when sortable.
#[component]
pub fn GridHeaderCell<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position among the visible columns, zero-based.
    column_index: usize,
    /// Renders a [`ColumnResizeHandle`] if the column is resizable, and enables
    /// `Alt+ArrowLeft` / `Alt+ArrowRight` to resize from the keyboard.
    #[props(default)]
    resizable: bool,
    /// Lets the column be dragged onto another header to change the column
    /// order, and moved with `Alt+Shift+ArrowLeft` / `Alt+Shift+ArrowRight`.
    #[props(default)]
    reorderable: bool,
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

    let show_handle = resizable && column.spec().resizable;
    let resize_id = id.clone();
    let drag_id = id.clone();
    let over_id = id.clone();
    let menu_id = id.clone();
    let drop_id = id.clone();
    let move_id = id.clone();
    // Dragged onto a group panel, a header groups by its column; dragged onto
    // another header, it changes the column order.
    let groupable =
        grid.has_group_panel() && column.spec().is_groupable() && !grid.group_by().contains(&id);
    let draggable = groupable || reorderable;
    let dragging = grid.dragged_column().is_some_and(|dragged| dragged == id);
    let pin = grid.column_pin(&id);
    let pin_offset = grid.column_pin_offset(&id);
    // The column headers are the last of the header rows; any above them are
    // the group headers.
    let leaf_row = grid.header_rows().saturating_sub(1);

    let shortcuts = match (show_handle, reorderable) {
        (true, true) => {
            Some("Alt+ArrowLeft Alt+ArrowRight Alt+Shift+ArrowLeft Alt+Shift+ArrowRight")
        }
        (true, false) => Some("Alt+ArrowLeft Alt+ArrowRight"),
        (false, true) => Some("Alt+Shift+ArrowLeft Alt+Shift+ArrowRight"),
        (false, false) => None,
    };

    // The keyboard alternatives to dragging. Handled here rather than on the
    // root so they only exist where the affordance does, and stopped from
    // bubbling so the root does not also treat the arrow as navigation.
    let onkeydown = move |event: KeyboardEvent| {
        let data = event.data();
        if !data.modifiers().alt() {
            return;
        }
        // Only the keys this cell claims; everything else bubbles on.
        if !matches!(
            data.key(),
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowDown
        ) {
            return;
        }

        // The key reached this cell, so this cell has DOM focus — but the grid
        // only learns that from its own navigation. Without saying so here, a
        // cell focused any other way (a script, the browser restoring focus)
        // would move a column and then lose focus to the re-render, swallowing
        // the next key.
        grid.set_focus(CellFocus::new(leaf_row, column_index));

        // Alt+Down opens the column menu, where there is one: the button sits
        // inside the cell and is not a tab stop, so this is the way in.
        if data.key() == Key::ArrowDown && grid.has_column_menu() {
            grid.open_column_menu(menu_id.clone());
            event.prevent_default();
            event.stop_propagation();
            return;
        }

        let step = if data.key() == Key::ArrowLeft { -1 } else { 1 };

        // Shift moves the column, Alt alone resizes it. Reordering is checked
        // first: it is the more specific gesture, and a column can be movable
        // without being resizable.
        let handled = if data.modifiers().shift() {
            reorderable && grid.move_column_by(&move_id, step)
        } else if show_handle {
            grid.resize_column_by(&resize_id, step as f32 * COLUMN_RESIZE_STEP);
            true
        } else {
            false
        };

        if handled {
            event.prevent_default();
            event.stop_propagation();
        }
    };

    let ondragover = move |event: DragEvent| {
        // Accepting the drop is what allows one at all, and only a column that
        // could actually move should look like a target.
        if reorderable
            && grid
                .dragged_column()
                .is_some_and(|dragged| dragged != over_id)
        {
            event.prevent_default();
        }
    };
    let ondrop = move |event: DragEvent| {
        if reorderable && grid.drop_dragged_column_at(&drop_id) {
            event.prevent_default();
        }
    };

    let focused = grid.focus() == CellFocus::new(leaf_row, column_index);
    let mut pull_focus = use_focus_pull(grid, CellFocus::new(leaf_row, column_index));
    let measured_id = id.clone();
    let onmounted = move |event: MountedEvent| {
        // Handed to the grid rather than measured here: at mount the stylesheet
        // may not have been applied, so the header still spans the container.
        // The grid measures it once its own resize fires, after layout.
        grid.register_column_element(&measured_id, event.data());
        pull_focus(event);
    };

    let onclick = move |event: MouseEvent| {
        // The click that ends a resize drag lands here when the pointer is
        // released over the header; it is not a request to sort.
        if grid.take_resize_click() || !sortable {
            return;
        }
        grid.set_focus(CellFocus::new(leaf_row, column_index));
        grid.toggle_sort(id.clone(), event.modifiers().shift());
    };

    rsx! {
        div {
            role: "columnheader",
            // Named explicitly, so that a menu button or any other control
            // inside the cell does not end up in the column's name.
            aria_label: column.label(),
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
            "data-align": column.spec().effective_align().as_str(),
            "data-filtered": grid.is_filtered(column.id()).then_some("true"),
            "aria-keyshortcuts": shortcuts,
            "data-dragging": dragging.then_some("true"),
            "data-pinned": pin.as_str(),
            style: pin_offset.map(|offset| format!("--dg-pin-offset: {offset}px;")),
            draggable: draggable.then_some("true"),
            ondragstart: move |event: DragEvent| {
                if !draggable {
                    return;
                }
                // Firefox starts no drag without data.
                let _ = event.data_transfer().set_data("text/plain", drag_id.as_str());
                grid.start_column_drag(drag_id.clone());
            },
            ondragend: move |_| grid.end_column_drag(),
            ondragover,
            ondrop,
            onmounted,
            onclick,
            onkeydown,
            ..attributes,
            {column.render_header()}
            // Rendered only where a column menu add-on is mounted, so the core
            // component need not know that one exists.
            if grid.has_column_menu() {
                GridColumnMenu { grid, column_index }
            }
            if show_handle {
                ColumnResizeHandle { grid, column_index }
            }
        }
    }
}

/// A drag handle that resizes its column.
///
/// Rendered by [`GridHeaderCell`] when `resizable` is set; use it directly only
/// when building a header of your own. Place it inside the header cell, usually
/// absolutely positioned along its end edge.
///
/// Only the drag starts here. The pointer is followed by [`GridRoot`], because
/// Dioxus has no pointer capture and a handle a few pixels wide would lose the
/// pointer after the first move. A resize therefore ends when the pointer leaves
/// the grid with the button released.
///
/// # Requirements
///
/// - **`touch-action: none`** on the handle, or a touch drag scrolls the page
///   instead of resizing.
///
/// Carries `aria-hidden`: it is a pointer affordance only. Keyboard users resize
/// with `Alt+ArrowLeft` / `Alt+ArrowRight` on the header cell, which announces
/// those keys through `aria-keyshortcuts`. Pressing the handle twice without
/// dragging — a double-click or double tap — resets the width.
///
/// Renders `data-resize-handle` for styling, and `data-resizing` while dragging.
#[component]
pub fn ColumnResizeHandle<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position among the visible columns, zero-based.
    column_index: usize,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let columns = grid.visible_columns();
    let Some(column) = columns.get(column_index) else {
        return rsx! {};
    };
    let id = column.id().clone();
    let active = grid.resizing_column().as_ref() == Some(&id);

    rsx! {
        div {
            aria_hidden: "true",
            "data-resize-handle": "",
            "data-resizing": active.then_some("true"),
            onpointerdown: move |event: PointerEvent| {
                // No text selection and no focus change while dragging, and the
                // header underneath must not see a press it would treat as its own.
                event.prevent_default();
                event.stop_propagation();
                grid.start_column_resize(&id, event.client_coordinates().x);
            },
            onclick: move |event: MouseEvent| event.stop_propagation(),
            // Resizing a header that can be dragged to group must not drag it.
            ondragstart: move |event: DragEvent| {
                event.prevent_default();
                event.stop_propagation();
            },
            ..attributes,
        }
    }
}

/// The body row group, rendering one [`GridRow`] per row on the current page.
#[component]
pub fn GridBody<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let row_count = grid.view().read().len();

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

    let total = grid.view().read().len();
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
                    // `clip` rather than `hidden`: both keep a tall cell inside
                    // the fixed row height, but `hidden` makes the row a scroll
                    // container, and a pinned cell would then stick to the row
                    // instead of to the grid and scroll away with the rest.
                    style: "height: {row_height}px; box-sizing: border-box; overflow: clip;",
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
    // A grouped view has rows of its own among the data rows; whatever walks
    // the rows by position gets the right one for each.
    match grid.row_kind(row_index) {
        Some(ViewRow::GroupHeader(_)) => {
            return rsx! {
                GridGroupRow { grid, row_index, attributes }
            };
        }
        Some(ViewRow::GroupFooter(_)) => {
            return rsx! {
                GridGroupFooterRow { grid, row_index, attributes }
            };
        }
        _ => {}
    }

    let columns = grid.visible_columns();
    let key = grid.key_at(row_index);
    let header_rows = grid.header_rows();

    // A cell may cover several columns, so what the row renders is its cells,
    // not its columns; the covered ones draw nothing.
    let spans = grid.row_spans(row_index);
    let cells: Vec<(usize, usize)> = (0..columns.len())
        .filter(|column| spans.is_anchor(*column))
        .map(|column| (column, spans.width(column)))
        .collect();

    // aria-rowindex is 1-based over every row of the view and counts the header
    // rows, so page 2 of a 25-row page starts at 27 under a one-row header.
    let (aria_row_index, group_levels) = {
        let view = grid.view();
        let view = view.read();
        (
            view.row_offset + row_index + header_rows + 1,
            view.group_levels,
        )
    };
    // In a treegrid, data rows sit one level below the innermost groups.
    let aria_level = (group_levels > 0).then(|| (group_levels + 1).to_string());

    let selectable = grid.selection_mode() != SelectionMode::None;
    let selected = key.as_ref().is_some_and(|key| grid.is_selected(key));
    let editing = grid
        .edit_target()
        .is_some_and(|target| !target.form && target.row_index == Some(row_index));

    rsx! {
        div {
            role: "row",
            aria_rowindex: "{aria_row_index}",
            aria_level,
            aria_selected: selectable.then_some(if selected { "true" } else { "false" }),
            "data-selected": "{selected}",
            "data-editing": editing.then_some("true"),
            "data-deleted": grid.is_row_deleted(row_index).then_some("true"),
            "data-saving": grid.is_row_saving(row_index).then_some("true"),
            ..attributes,
            for (column_index , span) in cells {
                GridCell {
                    key: "{column_index}",
                    grid,
                    row_index,
                    column_index,
                    span,
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
    /// How many columns the cell covers. The columns it covers render no cell
    /// of their own; [`GridRow`] works that out with
    /// [`GridHandle::row_spans`](crate::GridHandle::row_spans) and a loop of
    /// your own should do the same.
    #[props(default = 1)]
    span: usize,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let columns = grid.visible_columns();
    let Some(column) = columns.get(column_index).cloned() else {
        return rsx! {};
    };

    // Focus coordinates count the header as row 0.
    let focus_row = row_index + grid.header_rows();
    let span = span.max(1);
    // A cell covering several columns holds the focus for any of them, the way
    // a group's header cell holds it for its whole row.
    let covers = move |focus: CellFocus| {
        focus.row == focus_row && focus.col >= column_index && focus.col < column_index + span
    };
    let focused = covers(grid.focus());
    let onmounted = use_focus_pull_where(grid, covers);

    let editable = column.is_editable();
    let editing = editable
        && grid
            .edit_target()
            .is_some_and(|target| target.edits_cell(row_index, column.id()));

    // The row as the grid shows it: with unsaved and in-flight edits.
    let (content, tooltip) = if editing {
        (
            rsx! {
                GridCellEditor { grid, column_index }
            },
            None,
        )
    } else {
        let locale = grid.locale();
        let locale = locale.read();
        grid.with_row(row_index, |row| {
            (
                column.render_cell(row, &locale),
                column.tooltip(row, &locale),
            )
        })
        .unwrap_or_else(|| (rsx! {}, None))
    };
    let align = column.spec().effective_align().as_str();
    let overflow = column.spec().overflow.as_str();
    let pin = grid.column_pin(column.id());
    let pin_offset = grid.column_pin_offset(column.id());
    // Only worth saying in a grid that edits at all.
    let read_only = !editable && grid.edit_mode().is_some();
    let changed = grid.is_cell_changed(row_index, column_index);
    let cells = grid.cell_selection_mode();
    // The whole cell counts as selected, whichever of the columns it covers the
    // rectangle reaches: a cell over several columns is one cell.
    let cell_selected = cells.is_enabled()
        && (column_index..column_index + span)
            .any(|column| grid.is_cell_selected(CellFocus::new(focus_row, column)));

    let onclick = move |event: Event<MouseData>| {
        // A click into the editor is for the editor, which has focus already.
        if editing {
            return;
        }
        let at = CellFocus::new(focus_row, column_index);
        grid.set_focus(at);
        // Shift-click reaches from the anchor to here, as in a sheet.
        if event.modifiers().shift() {
            grid.extend_cell_selection(at);
        } else {
            grid.select_cell(at);
        }
        if grid.selection_mode() != SelectionMode::None
            && let Some(key) = grid.key_at(row_index)
        {
            grid.select(key);
        }
    };

    // A row is a subgrid, so a cell that covers several columns says so in the
    // layout as well as to the reader.
    let mut style = String::new();
    if span > 1 {
        style.push_str(&format!("grid-column: span {span};"));
    }
    if let Some(offset) = pin_offset {
        style.push_str(&format!(" --dg-pin-offset: {offset}px;"));
    }

    rsx! {
        div {
            role: "gridcell",
            aria_colindex: "{column_index + 1}",
            aria_colspan: (span > 1).then(|| span.to_string()),
            aria_selected: cells
                .is_enabled()
                .then_some(if cell_selected { "true" } else { "false" }),
            aria_readonly: read_only.then_some("true"),
            tabindex: if focused { "0" } else { "-1" },
            "data-align": align,
            "data-overflow": overflow,
            "data-editing": editing.then_some("true"),
            "data-changed": changed.then_some("true"),
            "data-cell-selected": cell_selected.then_some("true"),
            "data-pinned": pin.as_str(),
            style: (!style.is_empty()).then_some(style),
            title: tooltip,
            onmounted,
            onclick,
            ondoubleclick: move |_| {
                if !grid.is_editing() {
                    grid.start_edit(row_index, column_index);
                }
            },
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
pub(crate) fn use_focus_pull<T: GridRowKey + 'static>(
    grid: GridHandle<T>,
    at: CellFocus,
) -> impl FnMut(MountedEvent) + 'static {
    use_focus_pull_where(grid, move |focus| focus == at)
}

/// [`use_focus_pull`] for an element that stands for every cell `holds`
/// accepts, such as the one cell of a group's header, which holds the focus
/// whatever its column.
pub(crate) fn use_focus_pull_where<T: GridRowKey + 'static>(
    grid: GridHandle<T>,
    holds: impl Fn(CellFocus) -> bool + Copy + 'static,
) -> impl FnMut(MountedEvent) + 'static {
    let mut element = use_signal(|| None::<Rc<MountedData>>);

    use_effect(move || {
        // Read, so the effect re-runs whenever the focus or the pending flag
        // changes. The coordinate comes from the signal rather than a value
        // captured at first render.
        let should_take = holds(grid.focus()) && grid.focus_pending();
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
        if holds(grid.focus()) && grid.focus_pending() {
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
    let locale = grid.locale();
    let locale = locale.read();
    let position = locale.page_of(page + 1, page_count.max(1));

    rsx! {
        nav {
            aria_label: "{locale.pagination}",
            "data-page": "{page}",
            "data-page-count": "{page_count}",
            ..attributes,
            button {
                r#type: "button",
                disabled: page == 0,
                aria_label: "{locale.previous_page}",
                onclick: move |_| grid.previous_page(),
                "{locale.previous}"
            }
            span {
                role: "status",
                aria_live: "polite",
                "{position}"
            }
            button {
                r#type: "button",
                disabled: page >= last,
                aria_label: "{locale.next_page}",
                onclick: move |_| grid.next_page(),
                "{locale.next}"
            }
        }
    }
}

/// Loading and error state of a remote grid, as a live region.
///
/// Renders its container always, because a live region must already be in the
/// page for a screen reader to announce what appears in it. Inside, it shows
/// `loading_label` while a request is in flight, or the error with a retry
/// button when the last request failed. Both labels default to the grid's
/// [locale](GridHandle::locale). For a grid from
/// [`use_grid`](crate::use_grid) it stays empty.
///
/// Renders `data-state` as `idle`, `loading` or `error` for styling.
#[component]
pub fn GridStatus<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Announced while a request is in flight.
    #[props(default)]
    loading_label: Option<String>,
    /// Label of the button that repeats a failed request.
    #[props(default)]
    retry_label: Option<String>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let error = grid.load_error();
    let loading = grid.is_loading();
    let locale = grid.locale();
    let locale = locale.read();
    let loading_label = loading_label.unwrap_or_else(|| locale.loading.to_string());
    let retry_label = retry_label.unwrap_or_else(|| locale.retry.to_string());
    let state = match (&error, loading) {
        (Some(_), _) => "error",
        (None, true) => "loading",
        (None, false) => "idle",
    };

    rsx! {
        div {
            role: "status",
            aria_live: "polite",
            "data-state": state,
            ..attributes,
            if let Some(error) = error {
                span { "{error}" }
                button {
                    r#type: "button",
                    onclick: move |_| grid.reload(),
                    "{retry_label}"
                }
            } else if loading {
                "{loading_label}"
            }
        }
    }
}

/// A search box bound to the grid's global search.
#[component]
pub fn GridSearch<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Placeholder and accessible name of the input. Defaults to the grid's
    /// [locale](GridHandle::locale).
    #[props(default)]
    placeholder: Option<String>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let value = grid.search().unwrap_or_default();
    let placeholder = placeholder.unwrap_or_else(|| grid.locale().read().search.to_string());

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
    let label = grid.locale().read().filter_column(column.label());
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
