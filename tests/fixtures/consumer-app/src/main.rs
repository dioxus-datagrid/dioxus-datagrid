//! Consumer fixture for the registry smoke test.
//!
//! `scripts/registry-smoke.sh` runs `dx components add data_grid` against this
//! crate and then checks that it compiles. The code below is what the component's
//! `docs.md` tells a user to write, so the smoke test also catches documentation
//! that no longer matches the component.

mod components;

use components::data_grid::DataGrid;
use dioxus::prelude::*;
use dioxus_datagrid::{Column, GridRow, SelectionMode};

fn main() {
    dioxus::launch(App);
}

#[derive(Clone, PartialEq)]
struct User {
    id: u32,
    name: String,
    age: u32,
}

impl GridRow for User {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

#[component]
fn App() -> Element {
    let users = use_signal(|| {
        vec![
            User { id: 1, name: "Zoe".into(), age: 30 },
            User { id: 2, name: "adam".into(), age: 25 },
        ]
    });

    let columns = use_hook(|| {
        vec![
            Column::new("name", "Name")
                .cell(|user: &User| rsx! { "{user.name}" })
                .sort_by_text(|user: &User| user.name.as_str())
                .filter_by(|user: &User| user.name.clone()),
            Column::new("age", "Age")
                .cell(|user: &User| rsx! { "{user.age}" })
                .sort_by_value(|user: &User| user.age),
        ]
    });

    rsx! {
        DataGrid {
            data: users,
            columns,
            page_size: 25,
            selection: SelectionMode::Multi,
            on_selection_change: move |keys: Vec<u32>| {
                let _ = keys;
            },
        }
    }
}
