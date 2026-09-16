//! Playground: the shipped `data_grid` component, driven by controls that let a
//! human — or Playwright — exercise every mode it supports.
//!
//! `playground/src/components/data_grid` is copied from `registry/data_grid` by
//! `scripts/sync-playground-component.sh`, so what runs here is exactly what
//! `dx components add data_grid` hands a user.

mod components;

use components::data_grid::DataGrid;
use dioxus::prelude::*;
use dioxus_datagrid::{Column, ColumnWidth, GridRow, SelectionMode};

const STYLE: Asset = asset!("/assets/playground.css");

fn main() {
    dioxus::launch(App);
}

#[derive(Clone, PartialEq)]
struct Employee {
    id: u32,
    name: String,
    email: String,
    department: String,
    age: u32,
}

impl GridRow for Employee {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

/// Twelve rows: enough for three pages of five, with ties on `department` and
/// `age` so multi-column sorting has something to resolve.
fn employees() -> Vec<Employee> {
    const PEOPLE: [(&str, &str, u32); 12] = [
        ("Zoe Bauer", "engineering", 30),
        ("adam Fischer", "sales", 25),
        ("Mia Kaufmann", "engineering", 30),
        ("Carol Weber", "support", 41),
        ("bob Schneider", "finance", 41),
        ("Ingrid Vogel", "engineering", 28),
        ("Hugo Brandt", "legal", 52),
        ("Lena Hoffmann", "support", 34),
        ("Nils Krause", "sales", 45),
        ("Ava Richter", "research", 23),
        ("Theo Lang", "finance", 38),
        ("Yara Nowak", "research", 31),
    ];

    PEOPLE
        .iter()
        .enumerate()
        .map(|(index, (name, department, age))| {
            let local = name.to_lowercase().replace(' ', ".");
            Employee {
                id: index as u32 + 1,
                name: (*name).to_owned(),
                email: format!("{local}@example.com"),
                department: (*department).to_owned(),
                age: *age,
            }
        })
        .collect()
}

/// Rows for the virtualized mode: deterministic, so a test can predict what row
/// n contains, and varied enough that sorting and searching do real work.
fn many_employees(count: u32) -> Vec<Employee> {
    const FIRST: [&str; 8] = ["Ada", "Ben", "Cleo", "Dan", "Eva", "Finn", "Greta", "Hans"];
    const LAST: [&str; 6] = ["Bauer", "Fischer", "Kaufmann", "Weber", "Vogel", "Lang"];
    const DEPARTMENTS: [&str; 6] = [
        "engineering",
        "sales",
        "support",
        "finance",
        "legal",
        "research",
    ];

    (1..=count)
        .map(|id| {
            let index = id as usize;
            let first = FIRST[index % FIRST.len()];
            let last = LAST[(index / FIRST.len()) % LAST.len()];
            Employee {
                id,
                name: format!("{first} {last} {id}"),
                email: format!(
                    "{}.{}.{id}@example.com",
                    first.to_lowercase(),
                    last.to_lowercase()
                ),
                department: DEPARTMENTS[index % DEPARTMENTS.len()].to_owned(),
                age: 20 + id % 45,
            }
        })
        .collect()
}

/// Row count for the virtualized mode, matching `PLAN.md` phase 4.
const MANY: u32 = 100_000;
/// Fixed row height for the virtualized mode. Tall enough for the component's
/// cell padding, so no row content is clipped.
const ROW_HEIGHT: f64 = 40.0;

fn columns() -> Vec<Column<Employee>> {
    vec![
        Column::new("name", "Name")
            .cell(|row: &Employee| rsx! { "{row.name}" })
            .sort_by_text(|row: &Employee| row.name.as_str())
            .filter_by(|row: &Employee| row.name.clone()),
        Column::new("email", "Email")
            .cell(|row: &Employee| rsx! { "{row.email}" })
            .sort_by_text(|row: &Employee| row.email.as_str())
            .filter_by(|row: &Employee| row.email.clone()),
        Column::new("department", "Department")
            .cell(|row: &Employee| rsx! { "{row.department}" })
            .sort_by_text(|row: &Employee| row.department.as_str())
            .filter_by(|row: &Employee| row.department.clone()),
        Column::new("age", "Age")
            .cell(|row: &Employee| rsx! { "{row.age}" })
            .sort_by_value(|row: &Employee| row.age)
            .width(ColumnWidth::Px(88.0)),
    ]
}

#[component]
fn App() -> Element {
    let mut rows = use_signal(employees);
    let cols = use_hook(columns);

    let mut selection = use_signal(|| SelectionMode::Multi);
    let mut paged = use_signal(|| true);
    let mut virtualized = use_signal(|| false);
    let mut selected = use_signal(Vec::<u32>::new);

    rsx! {
        document::Link { rel: "stylesheet", href: STYLE }

        main { class: "page",
            header {
                h1 { "dioxus-datagrid playground" }
                p { class: "hint",
                    "This renders the component that "
                    code { "dx components add data_grid" }
                    " installs."
                }
            }

            div { class: "controls",
                fieldset {
                    legend { "Selection" }
                    for (label , mode) in [
                        ("None", SelectionMode::None),
                        ("Single", SelectionMode::Single),
                        ("Multi", SelectionMode::Multi),
                    ] {
                        label { key: "{label}",
                            input {
                                r#type: "radio",
                                name: "selection",
                                "data-testid": "selection-{label.to_lowercase()}",
                                checked: selection() == mode,
                                // The grid clears its own selection when the mode
                                // changes and reports that through
                                // on_selection_change, so nothing to reset here.
                                onchange: move |_| selection.set(mode),
                            }
                            "{label}"
                        }
                    }
                }

                label { class: "toggle",
                    input {
                        r#type: "checkbox",
                        "data-testid": "toggle-paging",
                        checked: paged() && !virtualized(),
                        disabled: virtualized(),
                        onchange: move |event| paged.set(event.checked()),
                    }
                    "Paged (5 per page)"
                }

                label { class: "toggle",
                    input {
                        r#type: "checkbox",
                        "data-testid": "toggle-virtualized",
                        checked: virtualized(),
                        onchange: move |event| {
                            let on = event.checked();
                            virtualized.set(on);
                            rows.set(if on { many_employees(MANY) } else { employees() });
                        },
                    }
                    "Virtualized ({MANY} rows)"
                }

                output { "data-testid": "selected-keys", class: "selected",
                    "selected: [{selected().iter().map(u32::to_string).collect::<Vec<_>>().join(\", \")}]"
                }
            }

            DataGrid {
                data: rows,
                columns: cols,
                page_size: (paged() && !virtualized()).then_some(5),
                row_height: virtualized().then_some(ROW_HEIGHT),
                height: virtualized().then(|| "480px".to_owned()),
                selection: selection(),
                column_filters: true,
                search_placeholder: "Search all columns",
                on_selection_change: move |keys: Vec<u32>| {
                    let mut keys = keys;
                    keys.sort_unstable();
                    selected.set(keys);
                },
            }
        }
    }
}
