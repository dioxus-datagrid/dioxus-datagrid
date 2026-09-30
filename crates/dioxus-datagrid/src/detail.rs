//! Detail rows: what a row shows when it is opened, and the button that opens
//! it.
//!
//! A detail row is a row of the view like any other (`ViewRow::Detail`), so it
//! counts in `aria-rowcount`, takes a place on the page, and follows its row
//! through sorting and filtering. What it holds is the application's: any
//! `Element`, a nested grid included.

use crate::GridHandle;
use datagrid_core::{CellFocus, GridRow as GridRowKey};
use dioxus::prelude::*;
use std::fmt;

/// What a grid's detail rows show, and which rows have one.
///
/// Handed to [`GridHandle::set_detail_rows`]. Without it no row has a detail
/// and an [expander column](datagrid_core::ColumnSpec::expander) draws no
/// buttons.
pub struct DetailRows<T: 'static> {
    /// Renders the detail of a row.
    pub render: Callback<T, Element>,
    /// Whether a row has a detail at all. Every row has one without it.
    pub has_detail: Option<Callback<T, bool>>,
}

impl<T> Clone for DetailRows<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for DetailRows<T> {}

impl<T> PartialEq for DetailRows<T> {
    fn eq(&self, other: &Self) -> bool {
        self.render == other.render && self.has_detail == other.has_detail
    }
}

impl<T> fmt::Debug for DetailRows<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DetailRows")
            .field("has_detail", &self.has_detail.is_some())
            .finish()
    }
}

impl<T: GridRowKey + PartialEq> GridHandle<T> {
    /// Sets what a row's detail shows, and which rows have one.
    ///
    /// Call it on every render, as the registry's component does; it only
    /// stores the callbacks when they change.
    ///
    /// ```ignore
    /// grid.set_detail_rows(DetailRows {
    ///     render: Callback::new(|order: Order| rsx! { OrderLines { order } }),
    ///     has_detail: Some(Callback::new(|order: Order| !order.lines.is_empty())),
    /// });
    /// ```
    pub fn set_detail_rows(&mut self, detail: DetailRows<T>) {
        if *self.detail_config.peek() != Some(detail) {
            self.detail_config.set(Some(detail));
        }
    }

    /// Takes the detail rows away, closing any that are open.
    pub fn clear_detail_rows(&mut self) {
        if self.detail_config.peek().is_some() {
            self.detail_config.set(None);
            self.collapse_all_details();
        }
    }

    /// Whether the grid shows detail rows at all.
    #[must_use]
    pub fn has_detail_rows(&self) -> bool {
        self.detail_config.read().is_some()
    }

    /// Whether the row at `row_index` on the current page has a detail to show.
    #[must_use]
    pub fn row_has_detail(&self, row_index: usize) -> bool {
        let Some(detail) = *self.detail_config.read() else {
            return false;
        };
        let Some(has_detail) = detail.has_detail else {
            return self.key_at(row_index).is_some();
        };
        self.with_row(row_index, |row: &T| has_detail.call(row.clone()))
            .unwrap_or(false)
    }

    /// Whether the detail of the row at `row_index` is open.
    #[must_use]
    pub fn is_detail_expanded(&self, row_index: usize) -> bool {
        self.key_at(row_index)
            .is_some_and(|key| self.expanded_details.read().contains(&key))
    }

    /// Whether detail rows can be opened as the grid stands.
    ///
    /// A virtualized grid places its rows by counting equal heights, and a
    /// detail row is as tall as its content; a remote grid is paged by a server
    /// that knows nothing of a row the client inserted. Both would put rows in
    /// the wrong place, so both refuse rather than mislead. See
    /// `docs/DECISIONS.md` ADR-0036.
    #[must_use]
    pub fn can_expand_details(&self) -> bool {
        self.has_detail_rows() && !self.remote && self.row_height().is_none()
    }

    /// Opens or closes the detail of the row at `row_index`, and says whether
    /// anything happened.
    ///
    /// Does nothing for a row without a detail, or where
    /// [`can_expand_details`](Self::can_expand_details) is false.
    pub fn toggle_detail(&mut self, row_index: usize) -> bool {
        if !self.can_expand_details() || !self.row_has_detail(row_index) {
            return false;
        }
        let Some(key) = self.key_at(row_index) else {
            return false;
        };
        let mut open = self.expanded_details.write();
        if !open.remove(&key) {
            open.insert(key);
        }
        true
    }

    /// Closes every open detail.
    pub fn collapse_all_details(&mut self) {
        if !self.expanded_details.peek().is_empty() {
            self.expanded_details.write().clear();
        }
    }

    /// How many details are open, counting rows that are not on this page.
    #[must_use]
    pub fn expanded_detail_count(&self) -> usize {
        self.expanded_details.read().len()
    }

    /// The id of the detail row of the row at `row_index`, for `aria-controls`.
    #[must_use]
    pub fn detail_id(&self, row_index: usize) -> Option<String> {
        let index = self.view().read().data_index(row_index)?;
        Some(self.detail_id_at(index))
    }

    /// The same id, asked by the index into the data — which is what a detail
    /// row carries, and what still works when its row is on the page before.
    pub(crate) fn detail_id_at(&self, index: usize) -> String {
        self.element_id(&format!("detail-{index}"))
    }

    /// Renders the detail of the row at `index` in the data.
    pub(crate) fn render_detail_at(&self, index: usize) -> Element {
        let Some(detail) = *self.detail_config.read() else {
            return rsx! {};
        };
        self.with_data_row(index, |row: &T| detail.render.call(row.clone()))
            .unwrap_or_else(|| rsx! {})
    }
}

/// The button that opens and closes a row's detail.
///
/// Rendered by [`GridCell`](crate::primitives::GridCell) for a column marked
/// [`expander`](datagrid_core::ColumnSpec::expander); use it directly only when
/// building cells of your own. Renders nothing for a row without a detail, or in
/// a grid that cannot open them.
///
/// Not a tab stop — the grid is one, and the cell around the button carries the
/// keyboard, where `Enter` toggles the detail. Carries `data-expander` as
/// `true` or `false` for styling, and `aria-controls` while the detail is
/// there to point at.
#[component]
pub fn GridDetailToggle<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position within the current page, zero-based.
    row_index: usize,
    /// Position among the visible columns, zero-based.
    column_index: usize,
) -> Element {
    let mut grid = grid;
    if !grid.can_expand_details() || !grid.row_has_detail(row_index) {
        return rsx! {};
    }
    let expanded = grid.is_detail_expanded(row_index);
    let locale = grid.locale();
    let locale = locale.read();
    let label = if expanded {
        locale.collapse_row.to_string()
    } else {
        locale.expand_row.to_string()
    };
    let at = CellFocus::new(row_index + grid.header_rows(), column_index);

    rsx! {
        button {
            r#type: "button",
            tabindex: "-1",
            aria_expanded: if expanded { "true" } else { "false" },
            // Only while the row it names is rendered: a reference to an
            // element that is not there says nothing.
            aria_controls: expanded.then(|| grid.detail_id(row_index)).flatten(),
            aria_label: label,
            "data-expander": if expanded { "true" } else { "false" },
            onclick: move |event: Event<MouseData>| {
                // The cell around it toggles too, for the rest of its area.
                event.stop_propagation();
                grid.set_focus(at);
                grid.toggle_detail(row_index);
            },
        }
    }
}

/// The row below an opened row: one cell across every column, holding whatever
/// the application renders for that row.
///
/// [`GridRow`](crate::primitives::GridRow) renders this for a
/// [`ViewRow::Detail`](datagrid_core::ViewRow::Detail) by itself. Renders
/// `data-detail-row`, and carries the id the opening button points at.
#[component]
pub fn GridDetailRow<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position within the current page, zero-based.
    row_index: usize,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let grid = grid;
    let header_rows = grid.header_rows();
    let focus_row = row_index + header_rows;
    // The one cell stands for every column of its row.
    let onmounted =
        crate::primitives::use_focus_pull_where(grid, move |focus| focus.row == focus_row);
    let aria_row_index = grid.view().read().row_offset + row_index + header_rows + 1;
    let column_count = grid.visible_column_count();
    let focused = grid.focus().row == focus_row;

    // Which row this is the detail of: taken from the view row rather than
    // from the row above, which may be on the page before.
    let Some(master) = grid
        .view()
        .read()
        .row(row_index)
        .and_then(datagrid_core::ViewRow::detail_index)
    else {
        return rsx! {};
    };
    let id = grid.detail_id_at(master);
    let group_levels = grid.view().read().group_levels;

    rsx! {
        div {
            role: "row",
            id,
            aria_rowindex: "{aria_row_index}",
            // In a treegrid, a detail sits where its row's children would.
            aria_level: (group_levels > 0).then(|| (group_levels + 2).to_string()),
            "data-detail-row": "",
            ..attributes,
            div {
                role: "gridcell",
                aria_colindex: "1",
                aria_colspan: "{column_count}",
                tabindex: if focused { "0" } else { "-1" },
                "data-detail-cell": "",
                onmounted,
                {grid.render_detail_at(master)}
            }
        }
    }
}
