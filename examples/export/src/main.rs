//! Exporting a grid: download its rows as CSV, print them, and see what the
//! writers are given.
//!
//! An application's way, with two dependencies and no registry component: the
//! grid comes from `use_grid`, the table an export writes from
//! `GridHandle::export_table`, and `datagrid-export` writes it. Everything the
//! `data_grid_export` component does is a few lines here.
//!
//! Run it with `dx serve --package example-export` for web, or add
//! `--platform desktop` for a desktop window.

use datagrid_export::{CsvOptions, CsvValues, HtmlOptions, XlsxOptions, to_csv, to_html, to_xlsx};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{GridBody, GridFooter, GridHeader, GridRoot, GridSearch};
use dioxus_datagrid::{
    Aggregate, CellAlign, CellFormat, Column, ExportScope, GridOptions, GridRow, GridState,
    print_html, save_bytes, save_file, use_grid,
};

const STYLE: Asset = asset!("/assets/grid.css");

fn main() {
    dioxus::launch(App);
}

#[derive(Clone, PartialEq)]
struct Person {
    id: u32,
    name: String,
    department: String,
    age: u32,
    salary: f64,
}

impl GridRow for Person {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

/// A fixed data set with a few awkward values: a comma, a quotation mark, and a
/// name that a spreadsheet would take for a formula.
fn people() -> Vec<Person> {
    const PEOPLE: [(&str, &str, u32); 12] = [
        ("Zoe Bauer", "engineering", 30),
        ("adam Fischer", "sales", 25),
        ("Mia \"Mimi\" Kaufmann", "engineering", 30),
        ("Carol Weber, Jr.", "support", 41),
        ("bob Schneider", "finance", 41),
        ("Ingrid Vogel", "engineering", 28),
        ("=Hugo Brandt", "legal", 52),
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
            let id = index as u32 + 1;
            Person {
                id,
                name: (*name).to_owned(),
                department: (*department).to_owned(),
                age: *age,
                salary: 3000.0 + f64::from(id * 371 % 40) * 100.0 + 0.5,
            }
        })
        .collect()
}

#[component]
fn App() -> Element {
    let people = use_signal(people);

    let columns = use_hook(|| {
        vec![
            Column::new("name", "Name")
                .cell(|person: &Person| rsx! { "{person.name}" })
                .sort_by_text(|person: &Person| person.name.as_str())
                .filter_by(|person: &Person| person.name.clone())
                .aggregate(Aggregate::Count),
            Column::new("department", "Department")
                .cell(|person: &Person| rsx! { "{person.department}" })
                .sort_by_text(|person: &Person| person.department.as_str())
                .filter_by(|person: &Person| person.department.clone()),
            Column::new("age", "Age")
                .cell(|person: &Person| rsx! { "{person.age}" })
                .sort_by_value(|person: &Person| person.age)
                .align(CellAlign::End)
                .aggregate(Aggregate::Average),
            // The format is what the grid shows, and what an export can carry:
            // a CSV holds the number, a printout the text.
            Column::new("salary", "Salary")
                .value_of(|person: &Person| person.salary)
                .sortable(true)
                .format(CellFormat::Currency {
                    symbol: "€".into(),
                    decimals: 2,
                })
                .align(CellAlign::End)
                .aggregate(Aggregate::Sum),
        ]
    });

    // Grouped from the start, so that the export has groups and aggregates to say.
    let mut state = GridState::new();
    state.group_by_column("department", None);
    let grid = use_grid(people, columns, GridOptions::default().initial_state(state));

    let mut for_excel = use_signal(|| false);
    let mut shown = use_signal(|| false);

    // What the writers are given. Everything below works from this table alone,
    // which is also what a server would build from its own rows.
    let csv_options = move || CsvOptions {
        // Excel reads a file without a byte order mark in the system code page,
        // and wants a semicolon where the decimal comma is.
        bom: for_excel(),
        delimiter: if for_excel() { ';' } else { ',' },
        values: if shown() {
            CsvValues::Shown
        } else {
            CsvValues::Raw
        },
        ..CsvOptions::default()
    };
    let csv = move || {
        grid.export_table(ExportScope::Filtered)
            .map(|table| to_csv(&table, &csv_options()))
            .unwrap_or_default()
    };

    rsx! {
        document::Link { rel: "stylesheet", href: STYLE }

        main { class: "page",
            h1 { "Export" }
            p { class: "hint",
                "Filter, sort or search, then download what the grid shows, or print it. "
                "The file holds every filtered row, not only the page on screen."
            }

            div { class: "toolbar",
                GridSearch { grid, class: "search", placeholder: "Search all columns" }
                span { class: "count", "{grid.filtered_len()} of {people.read().len()} rows" }
            }

            GridRoot { grid, class: "dg",
                GridHeader { grid, class: "dg-head" }
                GridBody { grid, class: "dg-body" }
                GridFooter { grid, class: "dg-foot" }
            }

            div { class: "export-bar", role: "group", aria_label: "Export",
                button {
                    r#type: "button",
                    onclick: move |_| save_file("people.csv", "text/csv;charset=utf-8", csv()),
                    "Download CSV"
                }
                button {
                    r#type: "button",
                    onclick: move |_| {
                        // A workbook, not a text file: numbers stay numbers, the salary
                        // keeps its currency format, the departments are an outline.
                        if let Some(table) = grid.export_table(ExportScope::Filtered)
                            && let Ok(workbook) = to_xlsx(&table, &XlsxOptions::default())
                        {
                            save_bytes(
                                "people.xlsx",
                                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                                &workbook,
                            );
                        }
                    },
                    "Download Excel"
                }
                button {
                    r#type: "button",
                    onclick: move |_| {
                        if let Some(table) = grid.export_table(ExportScope::Filtered) {
                            let options = HtmlOptions {
                                caption: Some("People".to_owned()),
                                style: true,
                                ..HtmlOptions::default()
                            };
                            print_html(to_html(&table, &options));
                        }
                    },
                    "Print"
                }
                label {
                    input {
                        r#type: "checkbox",
                        checked: for_excel(),
                        onchange: move |event| for_excel.set(event.checked()),
                    }
                    " CSV for Excel (semicolon, byte order mark)"
                }
                label {
                    input {
                        r#type: "checkbox",
                        checked: shown(),
                        onchange: move |event| shown.set(event.checked()),
                    }
                    " The text shown, not the values"
                }
            }

            h2 { "What the file holds" }
            pre { class: "preview", aria_label: "CSV preview", "{csv()}" }
        }
    }
}
