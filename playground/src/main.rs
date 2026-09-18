//! Playground: the shipped `data_grid` component, driven by controls that let a
//! human — or Playwright — exercise every mode it supports.
//!
//! `playground/src/components/data_grid` is copied from `registry/data_grid` by
//! `scripts/sync-playground-component.sh`, so what runs here is exactly what
//! `dx components add data_grid` hands a user.

mod components;

use chrono::NaiveDate;
use components::data_grid::DataGrid;
use dioxus::prelude::*;
use dioxus_datagrid::{
    CellFormat, Column, ColumnId, ColumnWidth, GridLocale, GridRow, GridState, SelectionMode,
};

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
    salary: f64,
    since: NaiveDate,
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
            let id = index as u32 + 1;
            Employee {
                id,
                name: (*name).to_owned(),
                email: format!("{local}@example.com"),
                department: (*department).to_owned(),
                age: *age,
                salary: salary_for(id),
                since: since_for(id),
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
                salary: salary_for(id),
                since: since_for(id),
            }
        })
        .collect()
}

/// A deterministic monthly salary with cents, so number formatting shows.
fn salary_for(id: u32) -> f64 {
    f64::from(2_800 + (id * 7_919) % 6_000) + f64::from(id % 4) * 0.25
}

/// A deterministic start date within twenty years of 2006.
fn since_for(id: u32) -> NaiveDate {
    let start = NaiveDate::from_ymd_opt(2006, 1, 2).unwrap_or_default();
    start + chrono::Days::new(u64::from((id * 2_111) % 7_300))
}

/// Row count for the virtualized mode, matching `PLAN.md` phase 4.
const MANY: u32 = 100_000;
/// Fixed row height for the virtualized mode. Tall enough for the component's
/// cell padding, so no row content is clipped.
const ROW_HEIGHT: f64 = 40.0;

/// Column labels in the two playground languages. Labels are the app's text,
/// not the grid's, so the locale does not translate them.
fn label(id: &str, german: bool) -> &'static str {
    match (id, german) {
        ("email", true) => "E-Mail",
        ("email", false) => "Email",
        ("department", true) => "Abteilung",
        ("department", false) => "Department",
        ("age", true) => "Alter",
        ("age", false) => "Age",
        ("salary", true) => "Gehalt",
        ("salary", false) => "Salary",
        ("since", true) => "Seit",
        ("since", false) => "Since",
        _ => "Name",
    }
}

fn columns(german: bool) -> Vec<Column<Employee>> {
    let label = |id| label(id, german);
    vec![
        Column::new("name", label("name"))
            .cell(|row: &Employee| rsx! { "{row.name}" })
            .sort_by_text(|row: &Employee| row.name.as_str())
            .filter_by(|row: &Employee| row.name.clone()),
        Column::new("email", label("email"))
            .cell(|row: &Employee| rsx! { "{row.email}" })
            .sort_by_text(|row: &Employee| row.email.as_str())
            .filter_by(|row: &Employee| row.email.clone()),
        Column::new("department", label("department"))
            .cell(|row: &Employee| rsx! { "{row.department}" })
            .sort_by_text(|row: &Employee| row.department.as_str())
            .filter_by(|row: &Employee| row.department.clone()),
        Column::new("age", label("age"))
            .value_of(|row: &Employee| row.age)
            .width(ColumnWidth::Px(88.0)),
        // No `.cell()` from here on: the grid shows the value in its format,
        // with separators and date order from the locale.
        Column::new("salary", label("salary"))
            .value_of(|row: &Employee| row.salary)
            .format(CellFormat::currency("€", 2)),
        Column::new("since", label("since"))
            .value_of(|row: &Employee| row.since)
            .format(CellFormat::Date),
    ]
}

/// The state when nothing is saved. The formatted columns start hidden, so the
/// grid opens with the four columns the tests expect; the column menu shows
/// them. Paged like the page-size control starts, because an initial state
/// takes precedence over `page_size`.
fn default_state() -> GridState {
    GridState {
        hidden_columns: vec![ColumnId::from("salary"), ColumnId::from("since")],
        ..GridState::paged(5)
    }
}

/// Where the playground keeps the grid state between visits.
const STORAGE_KEY: &str = "dioxus-datagrid-playground";

/// Reads the saved grid state from `localStorage`.
///
/// Persistence is application code, not library code: the library hands out a
/// serializable `GridState` and takes one back, and where it lives is up to the
/// app. `document::eval` works on web, desktop and mobile alike.
async fn load_state() -> Option<GridState> {
    let script = format!("return localStorage.getItem({STORAGE_KEY:?});");
    let saved: Option<String> = document::eval(&script).join().await.ok()?;
    // State from an older version with fields missing still loads; anything
    // unreadable is ignored rather than breaking the page.
    serde_json::from_str(&saved?).ok()
}

/// Writes the grid state to `localStorage`.
fn save_state(state: &GridState) {
    let Ok(json) = serde_json::to_string(state) else {
        return;
    };
    let script = format!("localStorage.setItem({STORAGE_KEY:?}, await dioxus.recv());");
    let _ = document::eval(&script).send(json);
}

/// Forgets the saved state and starts over.
fn reset_state() {
    let script = format!("localStorage.removeItem({STORAGE_KEY:?}); location.reload();");
    let _ = document::eval(&script);
}

#[component]
fn App() -> Element {
    let mut rows = use_signal(employees);
    let mut german = use_signal(|| false);
    let cols = use_memo(move || columns(german()));

    let mut selection = use_signal(|| SelectionMode::Multi);
    let mut paged = use_signal(|| true);
    let mut virtualized = use_signal(|| false);
    let mut overscan = use_signal(|| 20_usize);
    let mut selected = use_signal(Vec::<u32>::new);
    // Loaded before the grid renders: `initial_state` is read on the first
    // render only.
    let saved = use_resource(load_state);

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
                    for (label, mode) in [
                        ("None", SelectionMode::None),
                        ("Single", SelectionMode::Single),
                        ("Multi", SelectionMode::Multi),
                    ]
                    {
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

                fieldset {
                    legend { "Language" }
                    for (text , value) in [("English", false), ("Deutsch", true)] {
                        label { key: "{text}",
                            input {
                                r#type: "radio",
                                name: "language",
                                "data-testid": if value { "language-de" } else { "language-en" },
                                checked: german() == value,
                                onchange: move |_| german.set(value),
                            }
                            "{text}"
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

                // For tuning on a device: how many rows a fling can reveal
                // before the next render.
                label { class: "toggle",
                    "Overscan "
                    select {
                        "data-testid": "overscan",
                        disabled: !virtualized(),
                        onchange: move |event| {
                            if let Ok(value) = event.value().parse() {
                                overscan.set(value);
                            }
                        },
                        for value in [5_usize, 10, 20, 40] {
                            option {
                                key: "{value}",
                                value: "{value}",
                                selected: overscan() == value,
                                "{value}"
                            }
                        }
                    }
                }

                button {
                    r#type: "button",
                    "data-testid": "reset-state",
                    onclick: move |_| reset_state(),
                    "Reset saved state"
                }

                output { "data-testid": "selected-keys", class: "selected",
                    "selected: [{selected().iter().map(u32::to_string).collect::<Vec<_>>().join(\", \")}]"
                }
            }

            if let Some(initial_state) = saved.read().clone() {
                DataGrid {
                    data: rows,
                    columns: cols,
                    page_size: (paged() && !virtualized()).then_some(5),
                    row_height: virtualized().then_some(ROW_HEIGHT),
                    overscan: overscan(),
                    height: virtualized().then(|| "480px".to_owned()),
                    selection: selection(),
                    column_filters: true,
                    search_placeholder: if german() { "Alle Spalten durchsuchen" } else { "Search all columns" },
                    on_selection_change: move |keys: Vec<u32>| {
                        let mut keys = keys;
                        keys.sort_unstable();
                        selected.set(keys);
                    },
                    column_picker: true,
                    initial_state: initial_state.unwrap_or_else(default_state),
                    on_state_change: move |state: GridState| save_state(&state),
                    locale: if german() { GridLocale::german() } else { GridLocale::english() },
                    // Tells screen readers which language the grid speaks.
                    lang: if german() { "de" } else { "en" },
                }
            }
        }
    }
}
