//! Row reordering for `data_grid`, composed from what the `dioxus-datagrid`
//! crate already draws.
//!
//! This file is yours once `dx components add` copies it in. The gesture — the
//! handle, the drag, the keyboard route and the line that shows where a row
//! would land — lives in the crate; this component only says what a move does.

use dioxus::prelude::*;
use dioxus_datagrid::{GridHandle, GridRow, RowMove};

/// Props for [`DataGridReorder`].
#[derive(Props, Clone, PartialEq)]
pub struct DataGridReorderProps<T: GridRow + PartialEq + 'static> {
    /// Takes a row that was moved. The grid reports the move; the rows are
    /// yours to change, and [`RowMove::apply`] does it to a `Vec`.
    pub on_move: Callback<RowMove<T>>,
}

/// Lets the rows of the `DataGrid` around it be dragged into another order,
/// through a [`Column::drag_handle`](dioxus_datagrid::Column::drag_handle)
/// column. Renders nothing of its own.
///
/// Rows can only be reordered where the order on screen is their own: a sorted
/// or grouped grid draws no handles.
///
/// ```rust,ignore
/// DataGrid { data: tasks, columns,
///     DataGridReorder {
///         on_move: move |moved: RowMove<Task>| {
///             tasks.with_mut(|tasks| moved.apply(tasks));
///         },
///     }
/// }
/// ```
#[component]
pub fn DataGridReorder<T: GridRow + PartialEq + 'static>(
    props: DataGridReorderProps<T>,
) -> Element {
    // Put there by `DataGrid`; outside one there is nothing to reorder.
    let grid = try_use_context::<GridHandle<T>>();
    // Taking the component away takes the handles with it.
    use_drop(move || {
        if let Some(mut grid) = grid {
            grid.clear_row_move();
        }
    });
    let Some(mut grid) = grid else {
        return rsx! {};
    };

    grid.set_row_move(props.on_move);

    rsx! {}
}
