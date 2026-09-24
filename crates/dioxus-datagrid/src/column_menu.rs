//! The menu at a column header: sort, group, pin, hide, fit.

use crate::GridHandle;
use datagrid_core::{CellFocus, ColumnId, GridRow as GridRowKey, Pinned, SortDirection};
use dioxus::prelude::*;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Gives each menu on the page its own id, so button and panel can point at
/// each other.
fn next_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// What one entry of a column menu does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ColumnAction {
    /// Sorts by the column, or with `None` takes it out of the sort.
    Sort(Option<SortDirection>),
    /// Groups the rows by the column.
    Group,
    /// Stops grouping by the column.
    Ungroup,
    /// Holds the column at an edge, or with [`Pinned::None`] lets it scroll.
    Pin(Pinned),
    /// Hides the column.
    Hide,
    /// Forgets a width set by resizing, so the column sizes to its content.
    FitWidth,
}

impl ColumnAction {
    /// What an entry renders as `data-column-menu-action`: a stable name to
    /// style by, and to address an entry by what it does rather than by the
    /// language it happens to be written in.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sort(Some(SortDirection::Asc)) => "sort-asc",
            Self::Sort(Some(SortDirection::Desc)) => "sort-desc",
            Self::Sort(None) => "sort-clear",
            Self::Group => "group",
            Self::Ungroup => "ungroup",
            Self::Pin(Pinned::Start) => "pin-start",
            Self::Pin(Pinned::End) => "pin-end",
            Self::Pin(Pinned::None) => "unpin",
            Self::Hide => "hide",
            Self::FitWidth => "fit-width",
        }
    }

    /// Carries the action out on a grid.
    pub fn apply<T: GridRowKey + 'static>(self, grid: &mut GridHandle<T>, column: ColumnId) {
        match self {
            Self::Sort(Some(direction)) => grid.set_sort(column, direction),
            Self::Sort(None) => grid.clear_sort(&column),
            Self::Group => grid.group_by_column(column, None),
            Self::Ungroup => grid.ungroup_column(&column),
            Self::Pin(edge) => grid.set_column_pin(column, edge),
            Self::Hide => grid.set_column_hidden(column, true),
            Self::FitWidth => grid.reset_column_width(&column),
        }
    }
}

/// One entry of a column menu: what it does, what it is called, and — for the
/// entries that are a choice between states — whether it is the state that
/// holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnMenuEntry {
    /// What choosing it does.
    pub action: ColumnAction,
    /// Its text, in the grid's language.
    pub label: String,
    /// `Some` for the entries of a choice — the sort direction, the edge — so
    /// the menu can say which one holds. `None` for a plain action.
    pub checked: Option<bool>,
}

/// What a column offers right now, in the order a menu should show it.
///
/// Entries that would do nothing are left out: sorting for a column that cannot
/// be sorted, grouping for one that cannot be grouped, clearing a sort that is
/// not set, unpinning a column that is not pinned, hiding the last visible
/// column, and fitting a column that has no width to forget.
///
/// [`GridColumnMenu`] renders this; it is public so a menu of your own can, too.
#[must_use]
pub fn column_menu_entries<T: GridRowKey + PartialEq + 'static>(
    grid: &GridHandle<T>,
    column_index: usize,
) -> Vec<ColumnMenuEntry> {
    let columns = grid.visible_columns();
    let Some(column) = columns.get(column_index) else {
        return Vec::new();
    };

    let id = column.id().clone();
    let locale = grid.locale();
    let locale = locale.read();

    let sorted = grid.sort_direction(&id);
    let grouped = grid.group_by().contains(&id);
    let pin = grid.column_pin(&id);
    let mut entries: Vec<ColumnMenuEntry> = Vec::new();

    if column.is_sortable() {
        for (direction, label) in [
            (Some(SortDirection::Asc), locale.sort_ascending.to_string()),
            (
                Some(SortDirection::Desc),
                locale.sort_descending.to_string(),
            ),
            (None, locale.sort_clear.to_string()),
        ] {
            // Nothing to clear when nothing is sorted.
            if direction.is_none() && sorted.is_none() {
                continue;
            }
            entries.push(ColumnMenuEntry {
                action: ColumnAction::Sort(direction),
                label,
                // "Clear sort" is an action, not a state to be in.
                checked: direction.map(|wanted| sorted == Some(wanted)),
            });
        }
    }

    if column.spec().is_groupable() {
        entries.push(if grouped {
            ColumnMenuEntry {
                action: ColumnAction::Ungroup,
                label: locale.group_remove(column.label()),
                checked: None,
            }
        } else {
            ColumnMenuEntry {
                action: ColumnAction::Group,
                label: locale.group_by_column.to_string(),
                checked: None,
            }
        });
    }

    for (edge, label) in [
        (Pinned::Start, locale.pin_start.to_string()),
        (Pinned::End, locale.pin_end.to_string()),
        (Pinned::None, locale.pin_none.to_string()),
    ] {
        if edge == Pinned::None && !pin.is_pinned() {
            continue;
        }
        entries.push(ColumnMenuEntry {
            action: ColumnAction::Pin(edge),
            label,
            checked: Some(pin == edge),
        });
    }

    // The grid refuses to hide its last column, so do not offer it.
    if grid.visible_column_count() > 1 {
        entries.push(ColumnMenuEntry {
            action: ColumnAction::Hide,
            label: locale.hide_column.to_string(),
            checked: None,
        });
    }

    if grid.state().column_width(&id).is_some() {
        entries.push(ColumnMenuEntry {
            action: ColumnAction::FitWidth,
            label: locale.fit_width.to_string(),
            checked: None,
        });
    }

    entries
}

/// A menu of everything that can be done to one column: sorting, grouping,
/// pinning, hiding, and sizing it back to its content.
///
/// Follows the WAI-ARIA menu pattern. The button reports `aria-haspopup` and
/// `aria-expanded`; the panel is a `menu` of `menuitem`s, with `menuitemradio`
/// where the entries are a choice between states, so a screen reader announces
/// which one holds. `ArrowDown` and `ArrowUp` move between entries and wrap,
/// `Home` and `End` jump to the ends, `Escape` or a click outside closes the
/// menu and returns focus to the button.
///
/// Everything is unstyled: the parts carry `data-column-menu-*` attributes to
/// style them by, and each entry names what it does in
/// `data-column-menu-action`. The click-outside layer is `position: fixed` and
/// covers the page, so give the panel a higher `z-index` and position it.
///
/// What it offers comes from [`column_menu_entries`]; a column with nothing to
/// offer renders nothing at all.
#[component]
pub fn GridColumnMenu<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position among the visible columns, zero-based.
    column_index: usize,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let mut trigger = use_signal(|| None::<Rc<MountedData>>);
    let mut items = use_signal(Vec::<Rc<MountedData>>::new);
    let mut active = use_signal(|| 0_usize);
    let panel_id = use_hook(|| format!("dg-column-menu-{}", next_id()));

    let columns = grid.visible_columns();
    let Some(column) = columns.get(column_index).cloned() else {
        return rsx! {};
    };
    let entries = column_menu_entries(&grid, column_index);
    if entries.is_empty() {
        return rsx! {};
    }
    let count = entries.len();

    let id: ColumnId = column.id().clone();
    let menu_id = id.clone();
    let label = grid.locale().read().column_menu(column.label());

    // Closing hands the focus back to the header cell rather than to the
    // button: the button is not a tab stop, so focus parked on it would leave
    // the grid's arrow keys with nothing to move.
    let mut close = move || {
        grid.close_column_menu();
        items.write().clear();
        let row = grid.header_rows().saturating_sub(1);
        grid.set_focus(CellFocus::new(row, column_index));
    };

    // Opened from the keyboard, the button never had focus, so the menu puts it
    // on its first entry once the panel is there. Opened by a click, the same
    // thing happens, which is what a menu does anyway.
    use_effect(move || {
        if !grid.is_column_menu_open(&menu_id) {
            return;
        }
        let at = *active.peek();
        if let Some(item) = items.read().get(at).cloned() {
            spawn(async move {
                let _ = item.set_focus(true).await;
            });
        }
    });

    // Moves the highlight and takes DOM focus with it, as a menu does.
    let mut focus_item = move |index: usize| {
        active.set(index);
        if let Some(item) = items.peek().get(index).cloned() {
            spawn(async move {
                let _ = item.set_focus(true).await;
            });
        }
    };

    // Takes the column rather than capturing it: a closure holding a `ColumnId`
    // is not `Copy`, and every entry needs its own copy of this one.
    let mut run = move |action: ColumnAction, column: ColumnId| {
        action.apply(&mut grid, column);
        close();
    };

    let is_open = grid.is_column_menu_open(&id);

    rsx! {
        div { "data-column-menu": "", ..attributes,
            button {
                r#type: "button",
                // Not a tab stop: the grid is one tab stop and this button sits
                // inside a cell of it. The keyboard opens the menu with
                // `Alt+ArrowDown` on the header, which the header cell handles.
                tabindex: "-1",
                "data-column-menu-trigger": "",
                aria_label: "{label}",
                aria_haspopup: "menu",
                aria_expanded: "{is_open}",
                aria_controls: is_open.then(|| panel_id.clone()),
                onmounted: move |event| trigger.set(Some(event.data())),
                onclick: {
                    let id = id.clone();
                    move |event: MouseEvent| {
                        // The button sits inside the header cell, whose own click
                        // sorts the column. Opening a menu is not that.
                        event.stop_propagation();
                        if is_open {
                            close();
                        } else {
                            active.set(0);
                            grid.open_column_menu(id.clone());
                        }
                    }
                },
                span { aria_hidden: "true", "⋮" }
            }
            if is_open {
                div {
                    "data-column-menu-backdrop": "",
                    aria_hidden: "true",
                    style: "position: fixed; inset: 0;",
                    onclick: move |event: MouseEvent| {
                        event.stop_propagation();
                        close();
                    },
                }
                div {
                    id: "{panel_id}",
                    role: "menu",
                    aria_label: "{label}",
                    "data-column-menu-panel": "",
                    onkeydown: move |event: KeyboardEvent| {
                        let at = active();
                        let next = match event.key() {
                            Key::Escape => {
                                event.prevent_default();
                                event.stop_propagation();
                                close();
                                return;
                            }
                            // Wraps, which is what a menu does and a grid does not.
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
                    for (index , entry) in entries.into_iter().enumerate() {
                        button {
                            key: "{entry.label}",
                            r#type: "button",
                            role: if entry.checked.is_some() { "menuitemradio" } else { "menuitem" },
                            aria_checked: entry.checked.map(|on| on.to_string()),
                            tabindex: if index == active() { "0" } else { "-1" },
                            "data-column-menu-item": "",
                            "data-column-menu-action": "{entry.action.as_str()}",
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
                                let id = id.clone();
                                move |event: MouseEvent| {
                                    event.stop_propagation();
                                    run(entry.action, id.clone());
                                }
                            },
                            "{entry.label}"
                        }
                    }
                }
            }
        }
    }
}
