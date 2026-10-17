//! The context menu: what a right click, or `Shift+F10`, offers on a cell or a
//! column header.
//!
//! The grid decides what is on offer for a target — the actions of a column
//! menu on a header, copy, select, edit, delete and details on a cell — and an
//! application adds its own entries with a callback. [`GridContextMenu`] draws
//! the menu at the pointer, or at the focused cell when the keyboard opened it.
//! It follows the WAI-ARIA menu pattern, like the column menu, and is built from
//! plain parts (ADR-0025).

use crate::column_menu::{ColumnAction, column_menu_entries};
use crate::copy::write_clipboard;
use crate::{GridHandle, filter_menu::fit_shift};
use datagrid_core::{CellFocus, GridRow as GridRowKey, SelectionMode, to_tsv};
use dioxus::prelude::*;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

fn next_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// What a context menu is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ContextTarget {
    /// A column header: the column's position among the visible columns.
    Header {
        /// Zero-based position among the visible columns.
        column_index: usize,
    },
    /// A data cell: its row on the current page and its column among the
    /// visible ones.
    Cell {
        /// Zero-based position within the current page.
        row_index: usize,
        /// Zero-based position among the visible columns.
        column_index: usize,
    },
}

/// An open context menu: what it is about, and where it opens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextMenuState {
    /// What it is about.
    pub target: ContextTarget,
    /// The viewport coordinate it opens at: the pointer, or the focused cell's
    /// corner.
    pub x: f64,
    /// See [`x`](ContextMenuState::x).
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
enum Run {
    Column(ColumnAction),
    CopyCell,
    CopyRow,
    ToggleSelect,
    ToggleDetail,
    Edit,
    Delete,
    Custom(Callback<ContextTarget>),
}

/// One entry of a context menu.
///
/// The grid makes the standard ones; an application makes its own with
/// [`ContextEntry::new`].
#[derive(Clone, Debug, PartialEq)]
pub struct ContextEntry {
    /// A stable name, `data-context-action` on the entry: what it does, not what
    /// it is called in this language.
    pub id: String,
    /// Its text.
    pub label: String,
    /// Whether it is shown but cannot be chosen.
    pub disabled: bool,
    /// `Some` for an entry that is one of a choice, so the menu can say which
    /// one holds; `None` for a plain action.
    pub checked: Option<bool>,
    /// Whether a line separates it from the entry before.
    pub separated: bool,
    run: Run,
}

impl ContextEntry {
    /// An entry of your own: `on_select` is called with the target when it is
    /// chosen.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        on_select: Callback<ContextTarget>,
    ) -> Self {
        Self::of(id, label, Run::Custom(on_select))
    }

    fn of(id: impl Into<String>, label: impl Into<String>, run: Run) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
            checked: None,
            separated: false,
            run,
        }
    }

    /// Shows the entry but does not let it be chosen.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Draws a line between this entry and the one before it.
    #[must_use]
    pub fn separated(mut self) -> Self {
        self.separated = true;
        self
    }

    fn separated_if(mut self, separated: bool) -> Self {
        self.separated = separated;
        self
    }
}

impl<T: GridRowKey + PartialEq + 'static> GridHandle<T> {
    /// Whether a [`GridContextMenu`] is mounted, which decides whether a right
    /// click on a cell is the grid's or the browser's.
    #[must_use]
    pub fn has_context_menu(&self) -> bool {
        *self.context_mounted.read()
    }

    /// The context menu that is open, if one is.
    #[must_use]
    pub fn context_menu(&self) -> Option<ContextMenuState> {
        *self.context_menu.read()
    }

    /// What a context menu on `target` offers: the grid's own entries for it,
    /// then the application's.
    ///
    /// Entries that would do nothing are left out, as in the column menu: a
    /// delete in a grid that cannot delete, details on a row that has none.
    #[must_use]
    pub fn context_menu_entries(&self, target: ContextTarget) -> Vec<ContextEntry> {
        let mut entries = match target {
            ContextTarget::Header { column_index } => column_menu_entries(self, column_index)
                .into_iter()
                .map(|entry| {
                    let mut made = ContextEntry::of(
                        format!("column-{}", entry.action.as_str()),
                        entry.label,
                        Run::Column(entry.action),
                    );
                    made.checked = entry.checked;
                    made
                })
                .collect(),
            ContextTarget::Cell {
                row_index,
                column_index,
            } => self.cell_context_entries(row_index, column_index),
        };
        if let Some(extra) = *self.context_extras.peek() {
            let mut own = extra.call(target);
            if let Some(first) = own.first_mut()
                && !entries.is_empty()
            {
                first.separated = true;
            }
            entries.append(&mut own);
        }
        entries
    }

    fn cell_context_entries(&self, row_index: usize, column_index: usize) -> Vec<ContextEntry> {
        let locale = self.locale();
        let locale = locale.read();
        let columns = self.visible_columns();
        let Some(column) = columns.get(column_index) else {
            return Vec::new();
        };
        let key = self.key_at(row_index);
        let mut entries: Vec<ContextEntry> = Vec::new();

        // Copying, for a column that holds a value: markup has no text to copy.
        let is_chrome = column.spec().is_checkbox()
            || column.spec().is_expander()
            || column.spec().is_drag_handle();
        if column.spec().value.is_some() && !is_chrome {
            entries.push(ContextEntry::of(
                "copy",
                locale.context_copy.to_string(),
                Run::CopyCell,
            ));
        }
        entries.push(ContextEntry::of(
            "copy-row",
            locale.context_copy_row.to_string(),
            Run::CopyRow,
        ));

        // Row actions, in a group of their own.
        let mut rows: Vec<ContextEntry> = Vec::new();
        if self.selection_mode() != SelectionMode::None
            && let Some(key) = &key
        {
            let selected = self.is_selected(key);
            rows.push(ContextEntry::of(
                "select-row",
                if selected {
                    locale.context_deselect_row.to_string()
                } else {
                    locale.select_row.to_string()
                },
                Run::ToggleSelect,
            ));
        }
        if self.can_expand_details() && self.row_has_detail(row_index) {
            rows.push(ContextEntry::of(
                "details",
                if self.is_detail_expanded(row_index) {
                    locale.collapse_row.to_string()
                } else {
                    locale.expand_row.to_string()
                },
                Run::ToggleDetail,
            ));
        }
        if self.edit_mode().is_some() && column.is_editable() {
            rows.push(ContextEntry::of(
                "edit",
                locale.edit_edit.to_string(),
                Run::Edit,
            ));
        }
        if self.can_delete() {
            rows.push(ContextEntry::of(
                "delete",
                locale.edit_delete.to_string(),
                Run::Delete,
            ));
        }
        for (index, entry) in rows.into_iter().enumerate() {
            entries.push(entry.separated_if(index == 0));
        }
        entries
    }

    /// Opens a context menu on `target` at the viewport coordinate (`x`, `y`),
    /// and moves the focus to the cell it is about. Returns whether there was
    /// anything to open: a click that has nothing on offer should be left to the
    /// browser.
    pub fn open_context_menu(&mut self, target: ContextTarget, x: f64, y: f64) -> bool {
        if self.context_menu_entries(target).is_empty() {
            return false;
        }
        let focus = match target {
            ContextTarget::Header { column_index } => {
                CellFocus::new(self.header_rows().saturating_sub(1), column_index)
            }
            ContextTarget::Cell {
                row_index,
                column_index,
            } => CellFocus::new(row_index + self.header_rows(), column_index),
        };
        // Quietly: the menu takes the DOM focus, and a cell asked to pull it would
        // take it straight back. Closing asks for it again.
        self.set_focus_quietly(focus);
        self.context_menu
            .set(Some(ContextMenuState { target, x, y }));
        true
    }

    /// Closes the context menu, and puts the focus back on the cell it was
    /// about.
    pub fn close_context_menu(&mut self) {
        if self.context_menu.peek().is_some() {
            self.context_menu.set(None);
            self.request_focus_pull();
        }
    }

    /// Carries `entry` out on `target`, then closes the menu.
    pub fn run_context_entry(&mut self, entry: &ContextEntry, target: ContextTarget) {
        if entry.disabled {
            return;
        }
        match (&entry.run, target) {
            (Run::Column(action), ContextTarget::Header { column_index }) => {
                if let Some(column) = self.visible_columns().get(column_index) {
                    action.apply(self, column.id().clone());
                }
            }
            (
                Run::CopyCell,
                ContextTarget::Cell {
                    row_index,
                    column_index,
                },
            ) => write_clipboard(self.cell_text(row_index, column_index)),
            (Run::CopyRow, ContextTarget::Cell { row_index, .. }) => {
                let columns = self.visible_columns();
                let cells: Vec<String> = columns
                    .iter()
                    .enumerate()
                    .filter(|(_, column)| {
                        !(column.spec().is_checkbox()
                            || column.spec().is_expander()
                            || column.spec().is_drag_handle())
                    })
                    .map(|(index, _)| self.cell_text(row_index, index))
                    .collect();
                write_clipboard(to_tsv([cells]));
            }
            (Run::ToggleSelect, ContextTarget::Cell { row_index, .. }) => {
                if let Some(key) = self.key_at(row_index) {
                    self.toggle_select(key);
                }
            }
            (Run::ToggleDetail, ContextTarget::Cell { row_index, .. }) => {
                self.toggle_detail(row_index);
            }
            (
                Run::Edit,
                ContextTarget::Cell {
                    row_index,
                    column_index,
                },
            ) => {
                self.start_edit(row_index, column_index);
            }
            (Run::Delete, ContextTarget::Cell { .. }) => {
                // The focus is on the cell the menu is about, so this is the
                // selection if the row is in it, and the row alone otherwise.
                let keys = self.delete_candidates();
                self.request_delete(&keys);
            }
            (Run::Custom(callback), target) => callback.call(target),
            _ => {}
        }
        self.close_context_menu();
    }
}

/// The context menu of the grid: a menu at the pointer, or at the focused cell,
/// for a right click or `Shift+F10` on a cell or a column header.
///
/// Mount it inside [`GridRoot`](crate::primitives::GridRoot) — or anywhere the
/// grid is in scope — and cells and headers start to open it. Where a target has
/// nothing on offer the browser's own menu is left alone.
///
/// Follows the WAI-ARIA menu pattern: a `menu` of `menuitem`s (`menuitemradio`
/// where the entries are a choice), `ArrowDown` and `ArrowUp` move between them
/// and wrap, `Home` and `End` jump to the ends, `Escape`, `Tab` or a click
/// outside close it and put the focus back on the cell.
///
/// Unstyled: the parts carry `data-context-menu-*` attributes, and each entry
/// names what it does in `data-context-action`. The panel is `position: fixed`
/// at the pointer and is moved back inside the viewport once it is measured; the
/// click-outside layer covers the page, so give the panel a higher `z-index`.
#[component]
pub fn GridContextMenu<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Entries of your own, for a target, added after the grid's.
    #[props(default)]
    entries: Option<Callback<ContextTarget, Vec<ContextEntry>>>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let mut items = use_signal(Vec::<Rc<MountedData>>::new);
    let mut active = use_signal(|| 0_usize);
    let mut backdrop = use_signal(|| None::<Rc<MountedData>>);
    let mut panel = use_signal(|| None::<Rc<MountedData>>);
    let mut shift = use_signal(|| (0.0_f64, 0.0_f64));
    let panel_id = use_hook(|| format!("dg-context-menu-{}", next_id()));

    // Cells and headers open the menu only where one is mounted; otherwise the
    // right click is the browser's.
    if !*grid.context_mounted.peek() {
        grid.context_mounted.set(true);
    }
    if *grid.context_extras.peek() != entries {
        grid.context_extras.set(entries);
    }
    use_drop(move || {
        grid.context_mounted.set(false);
        grid.context_extras.set(None);
        grid.context_menu.set(None);
    });

    // Focus goes to the first entry when the menu opens, as a menu does.
    use_effect(move || {
        let open = grid.context_menu().is_some();
        if !open {
            return;
        }
        active.set(0);
        shift.set((0.0, 0.0));
        if let Some(item) = items.read().first().cloned() {
            spawn(async move {
                let _ = item.set_focus(true).await;
            });
        }
    });

    let Some(state) = grid.context_menu() else {
        return rsx! {};
    };
    let target = state.target;
    let list = grid.context_menu_entries(target);
    if list.is_empty() {
        return rsx! {};
    }
    let count = list.len();
    let label = grid.locale().read().context_menu.to_string();

    let mut close = move || {
        items.write().clear();
        grid.close_context_menu();
    };
    let mut focus_item = move |index: usize| {
        active.set(index);
        if let Some(item) = items.peek().get(index).cloned() {
            spawn(async move {
                let _ = item.set_focus(true).await;
            });
        }
    };

    let (dx, dy) = shift();
    let style = format!(
        "position: fixed; left: {}px; top: {}px; translate: {dx}px {dy}px;",
        state.x, state.y
    );

    rsx! {
        div { "data-context-menu": "", ..attributes,
            div {
                "data-context-menu-backdrop": "",
                aria_hidden: "true",
                style: "position: fixed; inset: 0;",
                onmounted: move |event| backdrop.set(Some(event.data())),
                onclick: move |event: MouseEvent| {
                    event.stop_propagation();
                    close();
                },
                // A right click on the layer is another request for a menu, and
                // the page under it should have it.
                oncontextmenu: move |event: MouseEvent| {
                    event.prevent_default();
                    close();
                },
            }
            div {
                id: "{panel_id}",
                role: "menu",
                aria_label: "{label}",
                "data-context-menu-panel": "",
                style,
                onmounted: move |event| panel.set(Some(event.data())),
                // Measured whenever the panel changes size: moved back inside the
                // viewport on either axis, which the backdrop stands for.
                onresize: move |_| {
                    spawn(async move {
                        let (Some(viewport), Some(own)) = (backdrop.peek().clone(), panel.peek().clone()) else {
                            return;
                        };
                        let (Ok(viewport), Ok(own)) = (viewport.get_client_rect().await, own.get_client_rect().await) else {
                            return;
                        };
                        let (cx, cy) = *shift.peek();
                        let nx = fit_shift(own.min_x() - cx, own.width(), viewport.min_x(), viewport.max_x());
                        let ny = fit_shift(own.min_y() - cy, own.height(), viewport.min_y(), viewport.max_y());
                        if (nx - cx).abs() > 0.5 || (ny - cy).abs() > 0.5 {
                            shift.set((nx, ny));
                        }
                    });
                },
                onkeydown: move |event: KeyboardEvent| {
                    let at = active();
                    let next = match event.key() {
                        Key::Escape | Key::Tab => {
                            event.prevent_default();
                            event.stop_propagation();
                            close();
                            return;
                        }
                        Key::ArrowDown => (at + 1) % count,
                        Key::ArrowUp => (at + count - 1) % count,
                        Key::Home => 0,
                        Key::End => count - 1,
                        _ => return,
                    };
                    event.prevent_default();
                    event.stop_propagation();
                    focus_item(next);
                },
                for (index , entry) in list.into_iter().enumerate() {
                    button {
                        key: "{entry.id}",
                        r#type: "button",
                        role: if entry.checked.is_some() { "menuitemradio" } else { "menuitem" },
                        aria_checked: entry.checked.map(|on| on.to_string()),
                        aria_disabled: entry.disabled.then_some("true"),
                        tabindex: if index == active() { "0" } else { "-1" },
                        "data-context-menu-item": "",
                        "data-context-action": "{entry.id}",
                        // A line between groups of entries, drawn by the style.
                        "data-separated": (entry.separated && index > 0).then_some("true"),
                        onmounted: move |event| {
                            let mut items = items.write();
                            if items.len() <= index {
                                items.resize(index + 1, event.data());
                            }
                            if let Some(slot) = items.get_mut(index) {
                                *slot = event.data();
                            }
                        },
                        onclick: {
                            let entry = entry.clone();
                            move |event: MouseEvent| {
                                event.stop_propagation();
                                grid.run_context_entry(&entry, target);
                            }
                        },
                        "{entry.label}"
                    }
                }
            }
        }
    }
}

/// Whether a key press asks for a context menu: the menu key, or `Shift+F10`.
pub(crate) fn is_context_key(event: &KeyboardData) -> bool {
    event.key() == Key::ContextMenu
        || (event.key() == Key::F10 && event.modifiers().shift() && !event.modifiers().ctrl())
}

/// Opens the menu for `target` at the corner of the cell `element`, as the
/// keyboard asks for it. Returns whether there is a menu to open, which is known
/// at once, before the cell has been measured.
pub(crate) fn open_from_keyboard<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    target: ContextTarget,
    element: Option<Rc<MountedData>>,
) -> bool {
    if !grid.has_context_menu() || grid.context_menu_entries(target).is_empty() {
        return false;
    }
    let mut grid = grid;
    spawn(async move {
        // The start of the cell, a line below its top: where a menu opened by a
        // pointer at the cell would be.
        let (x, y) = match element {
            Some(element) => match element.get_client_rect().await {
                Ok(rect) => (rect.min_x(), rect.max_y()),
                Err(_) => (0.0, 0.0),
            },
            None => (0.0, 0.0),
        };
        grid.open_context_menu(target, x, y);
    });
    true
}
