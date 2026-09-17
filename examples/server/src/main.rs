//! Server-side sorting, filtering and paging with simulated latency.
//!
//! The "server" runs in-process: it answers each `GridQuery` after a delay, so
//! the grid behaves as it would against a slow network. The request log shows
//! what the grid sends. Things to try:
//!
//! - Type a search quickly. Only one request goes out once typing pauses.
//! - Type one letter, wait for the request, then type a second. A one-letter
//!   search is the slowest, so its request is still running when the next one
//!   starts — and is cancelled instead of overwriting the newer result.
//! - "Fail next request", then page: the error appears with a retry button.
//!
//! Run it with `dx serve --package example-server`.

use datagrid_core::{
    ColumnSpec, DataSource, GridQuery, GridRow, GridState, Page, PageState, compute_view,
};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{
    GridBody, GridHeader, GridPagination, GridRoot, GridSearch, GridStatus,
};
use dioxus_datagrid::{Column, ColumnWidth, GridOptions, use_grid_remote};
use std::rc::Rc;
use std::time::Duration;

const STYLE: Asset = asset!("/assets/server.css");
const ROWS: u32 = 5_000;

fn main() {
    dioxus::launch(App);
}

#[derive(Clone, Debug, PartialEq)]
struct Employee {
    id: u32,
    name: String,
    department: String,
    city: String,
    salary: u32,
}

impl GridRow for Employee {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

/// The table the simulated server owns.
fn employees() -> Vec<Employee> {
    const FIRST: [&str; 8] = ["Ada", "Ben", "Cleo", "Dan", "Eva", "Finn", "Greta", "Hans"];
    const LAST: [&str; 6] = ["Bauer", "Fischer", "Kaufmann", "Weber", "Vogel", "Lang"];
    const DEPARTMENTS: [&str; 5] = ["engineering", "sales", "support", "finance", "legal"];
    const CITIES: [&str; 4] = ["Berlin", "Hamburg", "Köln", "München"];

    (1..=ROWS)
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

/// How the server sorts and filters: the same core logic the grid uses locally.
fn server_columns() -> Vec<ColumnSpec<Employee>> {
    vec![
        ColumnSpec::new("name")
            .sort_by_text(|row: &Employee| row.name.as_str())
            .filter_by(|row: &Employee| row.name.clone()),
        ColumnSpec::new("department")
            .sort_by_text(|row: &Employee| row.department.as_str())
            .filter_by(|row: &Employee| row.department.clone()),
        ColumnSpec::new("city")
            .sort_by_text(|row: &Employee| row.city.as_str())
            .filter_by(|row: &Employee| row.city.clone()),
        ColumnSpec::new("salary").sort_by_value(|row: &Employee| row.salary),
    ]
}

/// What became of a request, as far as the server can tell.
#[derive(Clone, Copy, PartialEq)]
enum Outcome {
    Pending,
    Answered,
    Failed,
    /// The grid dropped the request before the answer arrived.
    Cancelled,
}

#[derive(Clone, PartialEq)]
struct Request {
    id: usize,
    summary: String,
    latency: u64,
    outcome: Outcome,
}

/// An in-process server with adjustable latency.
struct SimulatedServer {
    rows: Rc<Vec<Employee>>,
    columns: Rc<Vec<ColumnSpec<Employee>>>,
    latency: Signal<u64>,
    fail_next: Signal<bool>,
    log: Signal<Vec<Request>>,
}

/// Marks a request cancelled if its future is dropped before it finishes.
struct Watch {
    log: Signal<Vec<Request>>,
    id: usize,
    finished: bool,
}

impl Watch {
    fn finish(mut self, outcome: Outcome) {
        self.finished = true;
        set_outcome(self.log, self.id, outcome);
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        if !self.finished {
            set_outcome(self.log, self.id, Outcome::Cancelled);
        }
    }
}

fn set_outcome(mut log: Signal<Vec<Request>>, id: usize, outcome: Outcome) {
    if let Some(request) = log.write().iter_mut().find(|request| request.id == id) {
        request.outcome = outcome;
    }
}

fn summarize(query: &GridQuery) -> String {
    let mut parts = vec![format!("page {}", query.page + 1)];
    if let Some(search) = &query.search {
        parts.push(format!("search \"{search}\""));
    }
    for (column, text) in &query.column_filters {
        parts.push(format!("{column} ~ \"{text}\""));
    }
    for sort in &query.sort {
        parts.push(format!("sort {} {:?}", sort.column, sort.direction));
    }
    parts.join(", ")
}

async fn sleep(duration: Duration) {
    #[cfg(not(target_family = "wasm"))]
    tokio::time::sleep(duration).await;
    #[cfg(target_family = "wasm")]
    gloo_timers::future::sleep(duration).await;
}

impl DataSource<Employee> for SimulatedServer {
    type Error = String;

    async fn fetch(&self, query: GridQuery) -> Result<Page<Employee>, String> {
        // Short searches match the most rows and, here, take the longest. That
        // makes an out-of-order answer easy to provoke.
        let term_length = query.search.as_ref().map_or(3, |term| term.chars().count());
        let latency = *self.latency.peek() * (4 - term_length.clamp(1, 3) as u64);

        let mut log = self.log;
        let id = log.peek().len() + 1;
        log.write().push(Request {
            id,
            summary: summarize(&query),
            latency,
            outcome: Outcome::Pending,
        });
        let watch = Watch {
            log,
            id,
            finished: false,
        };

        sleep(Duration::from_millis(latency)).await;

        let mut fail_next = self.fail_next;
        if *fail_next.peek() {
            fail_next.set(false);
            watch.finish(Outcome::Failed);
            return Err("The server did not respond. (Simulated.)".to_owned());
        }

        let state = GridState {
            sort: query.sort.clone(),
            column_filters: query.column_filters.clone(),
            search: query.search.clone(),
            page: Some(PageState {
                index: query.page,
                size: query.page_size,
            }),
            ..GridState::new()
        };
        let view = compute_view(&self.rows, &self.columns, &state);
        let rows = view
            .indices
            .iter()
            .filter_map(|&index| self.rows.get(index).cloned())
            .collect();

        watch.finish(Outcome::Answered);
        Ok(Page::new(rows, view.filtered_len))
    }
}

#[component]
fn App() -> Element {
    let latency = use_signal(|| 400_u64);
    let fail_next = use_signal(|| false);
    let log = use_signal(Vec::<Request>::new);

    let server = SimulatedServer {
        rows: use_hook(|| Rc::new(employees())),
        columns: use_hook(|| Rc::new(server_columns())),
        latency,
        fail_next,
        log,
    };

    // The client columns only render. The sort and filter closures mark a
    // column sortable or filterable; the server does the actual work.
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

    let grid = use_grid_remote(server, columns, GridOptions::paged(15));

    rsx! {
        document::Link { rel: "stylesheet", href: STYLE }

        main { class: "page",
            h1 { "Server-side data" }
            p { class: "hint",
                "{ROWS} rows on a simulated server. Sorting, search and paging are answered after a delay."
            }

            div { class: "controls",
                LatencyControl { latency }
                FailButton { fail_next }
            }

            div { class: "toolbar",
                GridSearch { grid, placeholder: "Search name, department or city" }
                GridStatus { grid, class: "status" }
            }

            GridRoot { grid, class: "grid",
                GridHeader { grid, resizable: true, class: "head" }
                GridBody { grid, class: "body" }
            }
            GridPagination { grid, class: "pagination" }

            RequestLog { log }
        }
    }
}

#[component]
fn LatencyControl(latency: Signal<u64>) -> Element {
    rsx! {
        label {
            "Latency "
            input {
                r#type: "range",
                min: "0",
                max: "2000",
                step: "100",
                value: "{latency}",
                oninput: move |event| {
                    if let Ok(value) = event.value().parse() {
                        latency.set(value);
                    }
                },
            }
            " {latency} ms"
        }
    }
}

#[component]
fn FailButton(fail_next: Signal<bool>) -> Element {
    rsx! {
        button {
            r#type: "button",
            disabled: fail_next(),
            onclick: move |_| fail_next.set(true),
            if fail_next() {
                "Next request will fail"
            } else {
                "Fail next request"
            }
        }
    }
}

#[component]
fn RequestLog(log: Signal<Vec<Request>>) -> Element {
    rsx! {
        section { class: "log",
            h2 { "Requests" }
            ol { reversed: true,
                for request in log.read().iter().rev().take(12) {
                    li {
                        key: "{request.id}",
                        "data-outcome": match request.outcome {
                            Outcome::Pending => "pending",
                            Outcome::Answered => "answered",
                            Outcome::Failed => "failed",
                            Outcome::Cancelled => "cancelled",
                        },
                        span { class: "summary", "{request.summary}" }
                        span { class: "meta",
                            "{request.latency} ms · "
                            match request.outcome {
                                Outcome::Pending => "waiting",
                                Outcome::Answered => "answered",
                                Outcome::Failed => "failed",
                                Outcome::Cancelled => "cancelled, a newer request replaced it",
                            }
                        }
                    }
                }
            }
        }
    }
}
