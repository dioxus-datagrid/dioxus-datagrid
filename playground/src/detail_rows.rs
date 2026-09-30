//! Detail rows in the playground: what a row shows when it is opened.
//!
//! An application reaches the grid from inside `DataGrid` through the context
//! it provides, which is all a detail row needs — there is no registry
//! component for it yet, and this is what one would do.

use crate::Employee;
use dioxus::prelude::*;
use dioxus_datagrid::{DetailRows, GridHandle};

/// Gives the grid around it detail rows. Renders nothing itself.
#[component]
pub fn EmployeeDetails(
    /// Only people of thirty or more have something to show, so that a row
    /// without a detail is in the way of anything that tries them.
    #[props(default)]
    some_rows: bool,
) -> Element {
    let mut grid = use_context::<GridHandle<Employee>>();

    // Made once: a callback made on every render would be a different callback
    // every time, and the grid would take it for a change.
    let render = use_hook(|| {
        Callback::new(|employee: Employee| {
            rsx! {
                div { class: "employee-detail", "data-testid": "detail-{employee.id}",
                    // Not a heading: the grid sits in a page whose headings are
                    // its own, and a row's detail is not a section of it.
                    p { class: "employee-detail-name", "{employee.name}" }
                    dl {
                        dt { "Email" }
                        dd { "{employee.email}" }
                        dt { "Department" }
                        dd { "{employee.department}" }
                    }
                    // Something to reach with the keyboard, to show that a
                    // detail is an ordinary part of the page.
                    button { "data-testid": "detail-action-{employee.id}", "Write to {employee.name}" }
                }
            }
        })
    });
    let has_detail = use_hook(|| Callback::new(|employee: Employee| employee.age >= 30));

    grid.set_detail_rows(DetailRows {
        render,
        has_detail: some_rows.then_some(has_detail),
    });

    rsx! {}
}
