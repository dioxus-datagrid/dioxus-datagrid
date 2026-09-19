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

use datagrid_core::{ColumnId, DataSource, DistinctValues, GridQuery, GridRow, Page};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{
    GridBody, GridColumnFilter, GridEditStatus, GridFilterMenu, GridHeader, GridPagination,
    GridRoot, GridSearch, GridStatus,
};
use dioxus_datagrid::{
    CellFormat, Column, ColumnWidth, EditMode, Editing, GridOptions, Save, ValueKind,
    use_grid_remote,
};
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

/// The departments and cities an employee can have.
const DEPARTMENTS: [&str; 5] = ["engineering", "sales", "support", "finance", "legal"];
const CITIES: [&str; 4] = ["Berlin", "Hamburg", "Frankfurt", "Munich"];

/// The rows the server seeds its database with.
#[cfg_attr(not(feature = "server"), allow(dead_code))]
fn employees() -> Vec<Employee> {
    const FIRST: [&str; 8] = ["Ada", "Ben", "Cleo", "Dan", "Eva", "Finn", "Greta", "Hans"];
    const LAST: [&str; 6] = ["Bauer", "Fischer", "Kaufmann", "Weber", "Vogel", "Lang"];
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

/// Answers a filter menu's value list from the database.
#[post("/api/employees/values")]
async fn load_values(
    column: ColumnId,
    query: GridQuery,
    limit: usize,
) -> Result<DistinctValues, ServerFnError> {
    let database = db::DATABASE.as_ref().map_err(ServerFnError::new)?;
    let connection = database
        .lock()
        .map_err(|_| ServerFnError::new("database lock poisoned"))?;
    db::distinct_values(&connection, &column, &query, limit).map_err(ServerFnError::new)
}

/// Saves an edited employee. The server checks it again: the client cannot
/// know every rule, and must not be trusted with them.
#[post("/api/employees/save")]
async fn save_employee(employee: Employee) -> Result<(), ServerFnError> {
    let database = db::DATABASE.as_ref().map_err(ServerFnError::new)?;
    let connection = database
        .lock()
        .map_err(|_| ServerFnError::new("database lock poisoned"))?;
    db::update_employee(&connection, &employee).map_err(ServerFnError::new)
}

/// The grid's view of the server: each method one server function call.
struct Api;

impl DataSource<Employee> for Api {
    type Error = ServerFnError;

    async fn fetch(&self, query: GridQuery) -> Result<Page<Employee>, ServerFnError> {
        load_employees(query).await
    }

    async fn distinct_values(
        &self,
        column: ColumnId,
        query: GridQuery,
        limit: usize,
    ) -> Result<DistinctValues, ServerFnError> {
        load_values(column, query, limit).await
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
                .filter_by(|row: &Employee| row.name.clone())
                .editable(|row: &mut Employee, name: String| row.name = name),
            Column::new("department", "Department")
                .cell(|row: &Employee| rsx! { "{row.department}" })
                .sort_by_text(|row: &Employee| row.department.as_str())
                .filter_by(|row: &Employee| row.department.clone())
                .editable(|row: &mut Employee, department: String| row.department = department)
                .choices(DEPARTMENTS),
            Column::new("city", "City")
                .cell(|row: &Employee| rsx! { "{row.city}" })
                .sort_by_text(|row: &Employee| row.city.as_str())
                .filter_by(|row: &Employee| row.city.clone())
                .editable(|row: &mut Employee, city: String| row.city = city)
                .choices(CITIES),
            // No `.cell()`: the value, formatted. The kind is declared because
            // a remote grid has no rows to read it from before the first page.
            Column::new("salary", "Salary")
                .value_of(|row: &Employee| row.salary)
                .format(CellFormat::currency("€", 0))
                .kind(ValueKind::Number)
                .width(ColumnWidth::Px(120.0))
                // Any whole number the client can read; the salary band is the
                // server's rule, checked in `save_employee`.
                .editable(|row: &mut Employee, salary: u32| row.salary = salary),
        ]
    });

    let mut grid = use_grid_remote(Api, columns, GridOptions::paged(20));
    // Saving is a server call. Until it answers the grid shows the edit; if
    // the server refuses, the grid takes it back and shows why.
    let editing = use_hook(|| Editing {
        mode: EditMode::Cell,
        on_save: Some(EventHandler::new(|save: Save<Employee>| async move {
            if let Err(error) = save_employee(save.row().clone()).await {
                save.fail(error.to_string());
            }
        })),
        ..Editing::default()
    });
    grid.set_editing(editing);

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
                GridEditStatus { grid, class: "edit-status" }
            }
            // The bar takes `>50000` or `40000..60000` as well as plain text;
            // the menus build conditions or value lists. Both go to the server.
            div { class: "filters",
                for (index , label) in ["Name", "Department", "City", "Salary"].into_iter().enumerate() {
                    div { key: "{label}", class: "filter",
                        label {
                            "{label} "
                            GridColumnFilter { grid, column_index: index }
                        }
                        GridFilterMenu { grid, column_index: index, class: "filter-menu" }
                    }
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
