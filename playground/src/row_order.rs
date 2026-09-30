//! Reordering rows in the playground: the grid says what moved, the rows are
//! the application's to change.
//!
//! As with the detail rows, an application reaches the grid from inside
//! `DataGrid` through the context it provides.

use crate::Employee;
use dioxus::prelude::*;
use dioxus_datagrid::{GridHandle, RowMove};

/// Lets the rows of the grid around it be dragged into another order. Renders
/// nothing itself.
#[component]
pub fn ReorderRows(rows: Signal<Vec<Employee>>) -> Element {
    let mut grid = use_context::<GridHandle<Employee>>();

    // Made once, so the grid does not take a new callback for a change.
    let on_move = use_hook(move || {
        Callback::new(move |moved: RowMove<Employee>| {
            let mut rows = rows;
            rows.with_mut(|rows| moved.apply(rows));
        })
    });

    grid.set_row_move(on_move);

    rsx! {}
}
