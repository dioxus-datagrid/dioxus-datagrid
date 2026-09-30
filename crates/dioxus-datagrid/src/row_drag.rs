//! Dragging rows into another order, and moving them with the keyboard.
//!
//! The grid never owns the rows, so it never reorders them: it reports the move
//! and the application does it — [`move_row`](datagrid_core::move_row) is the
//! same move against a `Vec`.
//!
//! The gesture is built from pointer events rather than HTML drag and drop,
//! which WebKit does not deliver for this (see `docs/DECISIONS.md` ADR-0037),
//! and there is a keyboard route for the same thing: `Alt+Shift+ArrowUp` and
//! `Alt+Shift+ArrowDown`.

use crate::GridHandle;
use datagrid_core::{CellFocus, GridRow as GridRowKey};
use dioxus::prelude::*;
use std::fmt;

/// A row that was dragged, or moved with the keyboard, to another place.
///
/// `from` and `to` are indices into the rows the grid was given, not positions
/// on the page, and `to` is where the row sits **afterwards**. Applying it is
/// one call:
///
/// ```ignore
/// on_row_move: move |moved: RowMove<Task>| {
///     tasks.with_mut(|tasks| moved.apply(tasks));
/// }
/// ```
pub struct RowMove<T> {
    /// The row that moved.
    pub row: T,
    /// Where it was.
    pub from: usize,
    /// Where it now is.
    pub to: usize,
}

impl<T> RowMove<T> {
    /// Does the move to `rows`, and says whether anything moved.
    pub fn apply(&self, rows: &mut Vec<T>) -> bool {
        datagrid_core::move_row(rows, self.from, self.to)
    }
}

impl<T: fmt::Debug> fmt::Debug for RowMove<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RowMove")
            .field("row", &self.row)
            .field("from", &self.from)
            .field("to", &self.to)
            .finish()
    }
}

/// A drag in progress: which row is being dragged, and where it would land.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct RowDrag {
    /// The dragged row's index into the rows the grid was given.
    pub(crate) from: usize,
    /// The row it is over now, as an index into those rows; the dragged row
    /// itself while it has not been dragged anywhere yet.
    pub(crate) to: usize,
}

/// Where a row would land relative to the row it is dragged over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropSide {
    /// Above it: the dragged row came from below.
    Before,
    /// Below it: the dragged row came from above.
    After,
}

impl DropSide {
    /// `"before"` or `"after"`, for a data attribute.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }
}

impl<T: GridRowKey + PartialEq> GridHandle<T> {
    /// Sets what a moved row does, which is what lets rows be reordered at all.
    ///
    /// Call it on every render, as a component of the grid does; it only stores
    /// the callback when it changes.
    pub fn set_row_move(&mut self, on_move: Callback<RowMove<T>>) {
        if *self.row_move.peek() != Some(on_move) {
            self.row_move.set(Some(on_move));
        }
    }

    /// Takes the reordering away, cancelling a drag in progress.
    pub fn clear_row_move(&mut self) {
        if self.row_move.peek().is_some() {
            self.row_move.set(None);
            self.cancel_row_drag();
        }
    }

    /// Whether rows may be dragged into another order as the grid stands.
    ///
    /// Only where the order on screen is the rows' own: a sorted or grouped
    /// grid shows them in an order of its making, and moving a row inside it
    /// would mean nothing to the rows underneath. See `docs/DECISIONS.md`
    /// ADR-0037.
    #[must_use]
    pub fn can_reorder_rows(&self) -> bool {
        self.row_move.read().is_some() && self.state.read().sort.is_empty() && !self.is_grouped()
    }

    /// The row being dragged, as a position on the current page.
    #[must_use]
    pub fn dragged_row(&self) -> Option<usize> {
        let drag = (*self.row_drag.read())?;
        self.page_position(drag.from)
    }

    /// Where the dragged row would land, as a position on the current page and
    /// the side of that row it would land on.
    #[must_use]
    pub fn row_drop_target(&self) -> Option<(usize, DropSide)> {
        let drag = (*self.row_drag.read())?;
        if drag.to == drag.from {
            return None;
        }
        let side = if drag.to < drag.from {
            DropSide::Before
        } else {
            DropSide::After
        };
        Some((self.page_position(drag.to)?, side))
    }

    /// Where a row of the data sits on the current page, if it is on it.
    fn page_position(&self, index: usize) -> Option<usize> {
        self.view()
            .read()
            .data_rows()
            .find(|&(_, data)| data == index)
            .map(|(position, _)| position)
    }

    /// Starts dragging the row at `row_index` on the current page.
    pub fn start_row_drag(&mut self, row_index: usize) {
        if !self.can_reorder_rows() {
            return;
        }
        let Some(from) = self.view().read().data_index(row_index) else {
            return;
        };
        self.row_drag.set(Some(RowDrag { from, to: from }));
    }

    /// Says the pointer is now over the row at `row_index` on the current page.
    pub fn drag_row_over(&mut self, row_index: usize) {
        if self.row_drag.peek().is_none() {
            return;
        }
        let Some(to) = self.view().read().data_index(row_index) else {
            return;
        };
        if let Some(drag) = self.row_drag.peek().as_ref()
            && drag.to == to
        {
            return;
        }
        if let Some(drag) = self.row_drag.write().as_mut() {
            drag.to = to;
        }
    }

    /// Ends a drag where it is, reporting the move. Returns whether a row moved.
    pub fn finish_row_drag(&mut self) -> bool {
        let Some(drag) = self.row_drag.write().take() else {
            return false;
        };
        self.report_row_move(drag.from, drag.to)
    }

    /// Ends a drag without moving anything.
    pub fn cancel_row_drag(&mut self) {
        if self.row_drag.peek().is_some() {
            self.row_drag.set(None);
        }
    }

    /// Moves the row at `row_index` on the current page one row up (`-1`) or
    /// down (`1`), which is what `Alt+Shift+ArrowUp` and `Alt+Shift+ArrowDown`
    /// do. Returns whether it moved.
    ///
    /// Counted in rows the user can see: with a filter on, a row moves past the
    /// row above it on screen, not past the rows between them that are hidden.
    pub fn move_row_by(&mut self, row_index: usize, step: isize) -> bool {
        if !self.can_reorder_rows() {
            return false;
        }
        let rows: Vec<(usize, usize)> = self.view().read().data_rows().collect();
        let Some(place) = rows.iter().position(|&(position, _)| position == row_index) else {
            return false;
        };
        let Some(target) = place.checked_add_signed(step) else {
            return false;
        };
        let (Some(&(_, from)), Some(&(target_position, to))) = (rows.get(place), rows.get(target))
        else {
            return false;
        };
        if !self.report_row_move(from, to) {
            return false;
        }
        // The focus follows the row, so that pressing again moves the same one.
        let column = self.focus().col;
        self.set_focus(CellFocus::new(target_position + self.header_rows(), column));
        self.request_focus_pull();
        true
    }

    /// Hands a move to the application.
    fn report_row_move(&mut self, from: usize, to: usize) -> bool {
        if from == to {
            return false;
        }
        let Some(on_move) = *self.row_move.peek() else {
            return false;
        };
        let Some(row) = self.data.peek().get(from).cloned() else {
            return false;
        };
        on_move.call(RowMove { row, from, to });
        true
    }
}

/// The handle that drags a row into another place.
///
/// Rendered by [`GridCell`](crate::primitives::GridCell) for a column marked
/// [`drag_handle`](datagrid_core::ColumnSpec::drag_handle); use it directly only
/// when building cells of your own. Renders nothing where rows cannot be
/// reordered.
///
/// Not a tab stop — the grid is one, and the keyboard route is
/// `Alt+Shift+ArrowUp` and `Alt+Shift+ArrowDown` on any cell of the row, which
/// the handle announces through `aria-keyshortcuts`.
///
/// **A touch does not drag.** A touch pointer is captured by the element it
/// starts on, so the rows it passes over never hear about it, and the grid
/// would have to measure them to know where the finger is. Until it does, a
/// touch on the handle scrolls the page as a touch anywhere else does, and the
/// keyboard is the way to move a row (`docs/DECISIONS.md` ADR-0037).
#[component]
pub fn GridRowHandle<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position within the current page, zero-based.
    row_index: usize,
) -> Element {
    let mut grid = grid;
    if !grid.can_reorder_rows() {
        return rsx! {};
    }
    let locale = grid.locale();
    let label = locale.read().drag_row.to_string();
    let dragging = grid.dragged_row() == Some(row_index);

    rsx! {
        span {
            role: "button",
            tabindex: "-1",
            aria_label: label,
            "aria-keyshortcuts": "Alt+Shift+ArrowUp Alt+Shift+ArrowDown",
            "data-row-handle": "",
            "data-dragging": dragging.then_some("true"),
            // The pointer, not HTML drag and drop, which WebKit delivers no
            // drop for. A touch is left alone: it would be captured here, and
            // the rows under the finger would never hear of it.
            onpointerdown: move |event: Event<PointerData>| {
                if event.data().pointer_type() == "touch" {
                    return;
                }
                event.stop_propagation();
                grid.start_row_drag(row_index);
            },
        }
    }
}
