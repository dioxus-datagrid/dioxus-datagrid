//! Grouping rows and aggregating columns, on the grid handle.

use crate::GridHandle;
use datagrid_core::{AggregateValue, CellFocus, ColumnId, GridRow, Group, GroupKey, ViewRow};
use dioxus::prelude::*;

impl<T: GridRow> GridHandle<T> {
    /// The columns rows are grouped by, outermost first.
    #[must_use]
    pub fn group_by(&self) -> Vec<ColumnId> {
        self.state.read().group_by.clone()
    }

    /// Whether the rows are grouped right now: by at least one column that
    /// exists and can be grouped by.
    #[must_use]
    pub fn is_grouped(&self) -> bool {
        self.view().read().group_levels > 0
    }

    /// Groups the rows by these columns, outermost first; an empty list stops
    /// grouping. Every group starts expanded.
    pub fn set_group_by(&mut self, columns: Vec<ColumnId>) {
        if self.state.peek().group_by != columns {
            self.state.write().set_group_by(columns);
        }
    }

    /// Groups by one more column, innermost, or moves it to `position` among
    /// the grouped columns. Ignores a column that cannot be grouped by.
    pub fn group_by_column(&mut self, column: impl Into<ColumnId>, position: Option<usize>) {
        let column = column.into();
        let groupable = self
            .columns
            .peek()
            .iter()
            .any(|entry| entry.id() == &column && entry.spec().is_groupable());
        if groupable {
            self.state.write().group_by_column(column, position);
        }
    }

    /// Stops grouping by a column.
    pub fn ungroup_column(&mut self, column: &ColumnId) {
        if self.state.peek().group_by.contains(column) {
            self.state.write().ungroup_column(column);
        }
    }

    /// The columns rows could be grouped by and are not yet, with their
    /// labels.
    #[must_use]
    pub fn groupable_columns(&self) -> Vec<(ColumnId, String)> {
        let grouped = self.state.read().group_by.clone();
        self.columns
            .read()
            .iter()
            .filter(|column| column.spec().is_groupable() && !grouped.contains(column.id()))
            .map(|column| (column.id().clone(), column.label().to_owned()))
            .collect()
    }

    /// A column's label, whether or not it is visible.
    #[must_use]
    pub fn column_label(&self, column: &ColumnId) -> Option<String> {
        self.columns
            .read()
            .iter()
            .find(|entry| entry.id() == column)
            .map(|entry| entry.label().to_owned())
    }

    /// Expands a collapsed group or collapses an expanded one.
    pub fn toggle_group(&mut self, key: &GroupKey) {
        self.state.write().toggle_group(key);
    }

    /// Expands or collapses one group.
    pub fn set_group_expanded(&mut self, key: &GroupKey, expanded: bool) {
        if self.state.peek().is_group_expanded(key) != expanded {
            self.state.write().set_group_expanded(key, expanded);
        }
    }

    /// Expands or collapses every group.
    pub fn set_all_groups_expanded(&mut self, expanded: bool) {
        self.state.write().set_all_groups_expanded(expanded);
    }

    /// The group whose header or footer sits at `row_index` on the current
    /// page.
    #[must_use]
    pub fn group_at(&self, row_index: usize) -> Option<Group> {
        self.view().read().group_at(row_index).cloned()
    }

    /// What kind of row sits at `row_index` on the current page.
    #[must_use]
    pub fn row_kind(&self, row_index: usize) -> Option<ViewRow> {
        self.view().read().row(row_index)
    }

    /// The aggregates over every filtered row, for a footer; empty when no
    /// column has one.
    #[must_use]
    pub fn totals(&self) -> Vec<AggregateValue> {
        self.view().read().totals.clone()
    }

    /// Whether any column has an aggregate.
    #[must_use]
    pub fn has_aggregates(&self) -> bool {
        self.columns
            .read()
            .iter()
            .any(|column| !column.spec().aggregates.is_empty())
    }

    /// Handles a key on the focused row if it heads a group, as the treegrid
    /// pattern has it: `ArrowRight` expands, `ArrowLeft` collapses or, on a
    /// collapsed group, moves to the group it is in, and `Enter` or `Space`
    /// toggle. Returns whether the key was used.
    pub fn group_key(&mut self, key: GroupKeyPress) -> bool {
        let focus = self.focus();
        let Some(row_index) = focus.row.checked_sub(1) else {
            return false;
        };
        let Some(ViewRow::GroupHeader(_)) = self.row_kind(row_index) else {
            return false;
        };
        let Some(group) = self.group_at(row_index) else {
            return false;
        };
        match key {
            GroupKeyPress::Expand if !group.expanded => self.set_group_expanded(&group.key, true),
            GroupKeyPress::Collapse if group.expanded => {
                self.set_group_expanded(&group.key, false);
            }
            GroupKeyPress::Collapse => {
                // Already collapsed: up to the group it is in, if that
                // group's header is on this page.
                let parent = group
                    .key
                    .parent()
                    .and_then(|parent| self.view().read().group_header_position(&parent));
                if let Some(position) = parent {
                    self.set_focus(CellFocus::new(position + 1, focus.col));
                }
            }
            GroupKeyPress::Toggle => self.toggle_group(&group.key),
            GroupKeyPress::Expand => {}
        }
        true
    }

    // -- dragging a column header onto a group panel ---------------------------

    /// Whether column headers can be dragged onto a group panel, because one
    /// is mounted.
    #[must_use]
    pub fn has_group_panel(&self) -> bool {
        *self.group_panel.read()
    }

    /// Records whether a group panel is mounted. Called by the panel.
    pub fn set_group_panel(&mut self, mounted: bool) {
        if *self.group_panel.peek() != mounted {
            self.group_panel.set(mounted);
        }
    }

    /// Starts dragging a column's header.
    pub fn start_column_drag(&mut self, column: ColumnId) {
        self.dragged_column.set(Some(column));
    }

    /// Ends dragging a column's header, dropped or not.
    pub fn end_column_drag(&mut self) {
        if self.dragged_column.peek().is_some() {
            self.dragged_column.set(None);
        }
    }

    /// The column whose header is being dragged.
    #[must_use]
    pub fn dragged_column(&self) -> Option<ColumnId> {
        self.dragged_column.read().clone()
    }

    /// Groups by the column being dragged, at `position` among the grouped
    /// columns or innermost, and ends the drag. Returns whether a column was
    /// being dragged.
    pub fn drop_dragged_column(&mut self, position: Option<usize>) -> bool {
        let Some(column) = self.dragged_column.peek().clone() else {
            return false;
        };
        self.dragged_column.set(None);
        self.group_by_column(column, position);
        true
    }

    // -- the footer ---------------------------------------------------------------

    /// Records whether a footer with the totals is mounted, so the keyboard
    /// can reach it. Called by the footer.
    pub fn set_footer(&mut self, mounted: bool) {
        if *self.footer.peek() != mounted {
            self.footer.set(mounted);
        }
    }

    /// Whether a footer with the totals is mounted.
    #[must_use]
    pub fn has_footer(&self) -> bool {
        *self.footer.read()
    }
}

/// A key pressed on a group's header; see [`GridHandle::group_key`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupKeyPress {
    /// `ArrowRight`.
    Expand,
    /// `ArrowLeft`.
    Collapse,
    /// `Enter` or `Space`.
    Toggle,
}
