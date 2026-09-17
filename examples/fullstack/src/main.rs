//! A grid backed by a real server: a Dioxus server function querying SQLite.
//!
//! The client sends every sort, filter, search and page change as a
//! `GridQuery` to the `load_employees` server function, which translates it
//! into SQL (see `db.rs`) and answers with one `Page`.
//!
//! Run it with `dx serve --package example-fullstack`. The CLI sees the
//! `fullstack` feature and builds both halves: the client with `web`, the
//! server with `server`.

#[cfg(feature = "server")]
mod db;

use datagrid_core::{DataSource, GridQuery, GridRow, Page};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{
    GridBody, GridColumnFilter, GridHeader, GridPagination, GridRoot, GridSearch, GridStatus,
};
use dioxus_datagrid::{Column, ColumnWidth, GridOptions, use_grid_remote};
use serde::{Deserialize, Serialize};

const STYLE: Asset = asset!("/assets/fullstack.css");

fn main() {
    dioxus::launch(App);
}

/// Crosses the network, so it serializes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Employee {
    pub id: u32,
    pub name: String,
    pub department: String,
    pub city: String,
    pub salary: u32,
}

impl GridRow for Employee {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

/// The rows the server seeds its database with.
#[cfg_attr(not(feature = "server"), allow(dead_code))]
fn employees() -> Vec<Employee> {
    const FIRST: [&str; 8] = ["Ada", "Ben", "Cleo", "Dan", "Eva", "Finn", "Greta", "Hans"];
    const LAST: [&str; 6] = ["Bauer", "Fischer", "Kaufmann", "Weber", "Vogel", "Lang"];
    const DEPARTMENTS: [&str; 5] = ["engineering", "sales", "support", "finance", "legal"];
    const CITIES: [&str; 4] = ["Berlin", "Hamburg", "Frankfurt", "Munich"];

    (1..=5_000_u32)
        .map(|id| {
            let n = id as usize;
            Employee {
                id,
                name: format!("{} {}", FIRST[n % FIRST.len()], LAST[(n / 8) % LAST.len()]),
                department: DEPARTMENTS[(n * 7) % DEPARTMENTS.len()].to_owned(),
                city: CITIES[(n * 3) % CITIES.len()].to_owned(),
                salary: 40_000 + (id * 7919) % 60_000,
            }
        })
        .collect()
}

/// Answers a grid query from the database. The body runs only on the server;
/// on the client this function becomes an HTTP call to it.
#[post("/api/employees")]
async fn load_employees(query: GridQuery) -> Result<Page<Employee>, ServerFnError> {
    let database = db::DATABASE.as_ref().map_err(ServerFnError::new)?;
    let connection = database
        .lock()
        .map_err(|_| ServerFnError::new("database lock poisoned"))?;
    db::query_employees(&connection, &query).map_err(ServerFnError::new)
}

/// The grid's view of the server: one method, one server function call.
struct Api;

impl DataSource<Employee> for Api {
    type Error = ServerFnError;

    async fn fetch(&self, query: GridQuery) -> Result<Page<Employee>, ServerFnError> {
        load_employees(query).await
    }
}

#[component]
fn App() -> Element {
    // Rendering only. The sort and filter closures mark columns sortable or
    // filterable; the server decides what that means, in db.rs.
    let columns = use_hook(|| {
        vec![
            Column::new("name", "Name")
                .cell(|row: &Employee| rsx! { "{row.name}" })
                .sort_by_text(|row: &Employee| row.name.as_str())
                .filter_by(|row: &Employee| row.name.clone()),
            Column::new("department", "Department")
                .cell(|row: &Employee| rsx! { "{row.department}" })
                .sort_by_text(|row: &Employee| row.department.as_str())
                .filter_by(|row: &Employee| row.department.clone()),
            Column::new("city", "City")
                .cell(|row: &Employee| rsx! { "{row.city}" })
                .sort_by_text(|row: &Employee| row.city.as_str())
                .filter_by(|row: &Employee| row.city.clone()),
            Column::new("salary", "Salary")
                .cell(|row: &Employee| rsx! { "{row.salary} €" })
                .sort_by_value(|row: &Employee| row.salary)
                .width(ColumnWidth::Px(120.0)),
        ]
    });

    let grid = use_grid_remote(Api, columns, GridOptions::paged(20));

    rsx! {
        document::Link { rel: "stylesheet", href: STYLE }

        main { class: "page",
            h1 { "SQLite through a server function" }
            p { class: "hint",
                "Every sort, filter, search and page change runs a SQL query on the server."
            }

            div { class: "toolbar",
                GridSearch { grid, placeholder: "Search name, department or city" }
                GridStatus { grid, class: "status" }
            }
            div { class: "filters",
                label {
                    "Department "
                    GridColumnFilter { grid, column_index: 1 }
                }
                label {
                    "City "
                    GridColumnFilter { grid, column_index: 2 }
                }
            }

            GridRoot { grid, class: "grid",
                GridHeader { grid, resizable: true, class: "head" }
                GridBody { grid, class: "body" }
            }
            GridPagination { grid, class: "pagination" }
        }
    }
}
