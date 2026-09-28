//! The checkbox column: a checkbox in every row, and a "select all" above them.
//!
//! The grid draws these rather than the application, because only the grid knows
//! what is selected. A column asks for them with
//! [`ColumnSpec::checkbox`](datagrid_core::ColumnSpec::checkbox).
//!
//! Neither checkbox is a tab stop. The grid is one stop with a roving tabindex,
//! so the cell around a checkbox carries the keyboard: `Space` on a row toggles
//! it, `Space` on the header cell toggles them all.

use crate::GridHandle;
use datagrid_core::{CellFocus, GridRow as GridRowKey, SelectionMode};
use dioxus::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

/// Writes the header checkbox's third state, which no attribute can express.
///
/// `indeterminate` is a DOM property and HTML has no attribute for it; Dioxus
/// writes `checked`, `value` and `selected` as properties but has no entry for
/// this one (`docs/VERIFICATION.md` §16). So the state a "select all" needs most
/// — some rows selected, not all — is set from here, by the id the box carries.
const SET_INDETERMINATE: &str = r#"
    const [id, mixed] = await dioxus.recv();
    const box = document.getElementById(id);
    if (box) {
        box.indeterminate = mixed;
    }
    return true;
"#;

/// Numbers the header checkboxes, so that two grids on a page do not share an
/// id. Only ever read from the one thread a `VirtualDom` runs on.
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// The "select all" checkbox of a checkbox column's header.
///
/// Rendered by [`GridHeaderCell`](crate::primitives::GridHeaderCell) for a column
/// marked [`checkbox`](datagrid_core::ColumnSpec::checkbox); use it directly only
/// when building a header of your own.
///
/// Renders nothing unless rows can be selected in
/// [`SelectionMode::Multi`] — "all of them" is not something a single
/// selection can hold. Covers the rows **on show**: with paging, the current
/// page.
///
/// Carries `data-select-all` as `none`, `partial` or `all` for styling, and shows
/// the third state as a native mixed checkbox.
#[component]
pub fn GridSelectAll<T: GridRowKey + PartialEq + 'static>(grid: GridHandle<T>) -> Element {
    let mut grid = grid;
    let id = use_hook(|| format!("dg-select-all-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed)));

    // Read inside the effect, not captured from this render, so that the effect
    // re-runs whenever the selection or the rows on show change.
    use_effect({
        let id = id.clone();
        move || {
            let mixed = grid.visible_selection().is_partial();
            let id = id.clone();
            spawn(async move {
                let eval = document::eval(SET_INDETERMINATE);
                if eval.send((id, mixed)).is_ok() {
                    // Awaited so the task outlives the script it started.
                    let _ = eval.join::<bool>().await;
                }
            });
        }
    });

    if grid.selection_mode() != SelectionMode::Multi {
        return rsx! {};
    }
    let extent = grid.visible_selection();
    let locale = grid.locale();
    let label = locale.read().select_all.to_string();

    rsx! {
        input {
            id,
            r#type: "checkbox",
            tabindex: "-1",
            checked: extent.is_all(),
            aria_label: label,
            "data-select-all": extent.as_str(),
            onclick: move |event: Event<MouseData>| {
                // The cell around it toggles too, for the rest of its area.
                event.stop_propagation();
                grid.toggle_select_all();
            },
        }
    }
}

/// The checkbox of one row in a checkbox column.
///
/// Rendered by [`GridCell`](crate::primitives::GridCell) for a column marked
/// [`checkbox`](datagrid_core::ColumnSpec::checkbox); use it directly only when
/// building cells of your own. Renders nothing where rows cannot be selected.
///
/// A click toggles the row, `Shift`+click reaches from the anchor to it — the
/// same as on the cell around it.
#[component]
pub fn GridSelectCheckbox<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position within the current page, zero-based.
    row_index: usize,
    /// Position among the visible columns, zero-based.
    column_index: usize,
) -> Element {
    let mut grid = grid;
    if grid.selection_mode() == SelectionMode::None {
        return rsx! {};
    }
    let Some(key) = grid.key_at(row_index) else {
        return rsx! {};
    };
    let selected = grid.is_selected(&key);
    let locale = grid.locale();
    let label = locale.read().select_row.to_string();
    let at = CellFocus::new(row_index + grid.header_rows(), column_index);

    rsx! {
        input {
            r#type: "checkbox",
            tabindex: "-1",
            checked: selected,
            aria_label: label,
            onclick: move |event: Event<MouseData>| {
                // Handled here, so the cell does not also take the click: the
                // box is the more specific target.
                event.stop_propagation();
                // The click gave DOM focus to the box; the grid has to be told
                // where that is, or the next arrow key starts somewhere else.
                grid.set_focus(at);
                if event.modifiers().shift() {
                    grid.extend_select(key.clone());
                } else {
                    grid.toggle_select(key.clone());
                }
            },
        }
    }
}
