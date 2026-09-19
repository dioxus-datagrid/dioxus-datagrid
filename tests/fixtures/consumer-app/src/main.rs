//! Consumer fixture for the registry smoke test.
//!
//! `scripts/registry-smoke.sh` runs `dx components add` for `data_grid`,
//! `data_grid_editor` and `data_grid_group_panel` against this crate and then
//! checks that it compiles. The code below is what the components'
//! `docs.md` tells a user to write, so the smoke test also catches documentation
//! that no longer matches the component.

mod components;

use components::data_grid::DataGrid;
use components::data_grid_editor::DataGridEditor;
use components::data_grid_group_panel::DataGridGroupPanel;
use dioxus::prelude::*;
use dioxus_datagrid::{Aggregate, Column, Delete, EditMode, GridRow, Save, SelectionMode};

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
    let mut users = use_signal(|| {
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
                .filter_by(|user: &User| user.name.clone())
                .editable(|user: &mut User, name: String| user.name = name)
                .validate(|user: &User| {
                    if user.name.trim().is_empty() {
                        Err("Enter a name".into())
                    } else {
                        Ok(())
                    }
                }),
            Column::new("age", "Age")
                .cell(|user: &User| rsx! { "{user.age}" })
                .sort_by_value(|user: &User| user.age)
                .editable(|user: &mut User, age: u32| user.age = age)
                .aggregate(Aggregate::Average),
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
            DataGridGroupPanel::<User> {}
            DataGridEditor {
                mode: EditMode::Row,
                on_save: move |save: Save<User>| {
                    let row = save.row().clone();
                    users.with_mut(|users| {
                        if let Some(user) = users.iter_mut().find(|user| user.id == row.id) {
                            *user = row;
                        }
                    });
                },
                on_delete: move |delete: Delete<User>| {
                    let gone: Vec<u32> = delete.rows().iter().map(|user| user.id).collect();
                    users.retain(|user| !gone.contains(&user.id));
                },
            }
        }
    }
}
