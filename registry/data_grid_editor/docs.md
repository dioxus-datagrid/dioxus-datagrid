# data_grid_editor

Editing for [`data_grid`](../data_grid/docs.md): one cell at a time, a whole row inline, a row in a
form dialog, or cells collected into a batch. Adding and deleting rows, validation at the cell, and
a save that fails puts the old value back.

Install `data_grid` first, then this:

```bash
dx components add data_grid --git https://github.com/dioxus-datagrid/dioxus-datagrid
dx components add data_grid_editor --git https://github.com/dioxus-datagrid/dioxus-datagrid
```

The logic lives in the [`dioxus-datagrid`][crate] crate; this file is the styled shell.

[crate]: https://github.com/dioxus-datagrid/dioxus-datagrid

## Usage

Say how each column is edited, then put `DataGridEditor` inside the `DataGrid`:

```rust
use dioxus::prelude::*;
use dioxus_datagrid::{Column, Delete, EditMode, GridRow, Save};

use crate::components::data_grid::DataGrid;
use crate::components::data_grid_editor::DataGridEditor;

#[component]
fn Users() -> Element {
    let mut users = use_signal(load_users);

    let columns = use_hook(|| {
        vec![
            Column::new("name", "Name")
                .value_text(|user: &User| user.name.as_str())
                .editable(|user: &mut User, name: String| user.name = name)
                .validate(|user: &User| {
                    if user.name.trim().is_empty() {
                        Err("Enter a name".into())
                    } else {
                        Ok(())
                    }
                }),
            // Typed: "abc" is refused with "Enter a number" before the setter runs.
            Column::new("age", "Age")
                .value_of(|user: &User| user.age)
                .editable(|user: &mut User, age: u32| user.age = age),
            Column::new("team", "Team")
                .value_text(|user: &User| user.team.as_str())
                .editable(|user: &mut User, team: String| user.team = team)
                .choices(["Red", "Blue", "Green"]),
        ]
    });

    rsx! {
        DataGrid { data: users, columns,
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
```

The grid never writes your data. Each callback gets a token with the rows; store them, and call
`fail` on the token if that did not work:

```rust
on_save: move |save: Save<User>| async move {
    if let Err(error) = api::update(save.row()).await {
        // The grid shows the row as it was, and the message.
        save.fail(error.to_string());
    }
},
```

While the save runs, the grid already shows the edited row.

## Props

| Prop | Default | Meaning |
|---|---|---|
| `mode` | `EditMode::Cell` | `Cell`, `Row`, `Dialog` or `Batch`. |
| `on_save` | — | Saves an edited row. |
| `on_create` | — | Saves a new row. |
| `on_delete` | — | Deletes rows. Without it there is no delete button. |
| `on_batch_save` | — | Saves a batch, with `EditMode::Batch`: changed, added and deleted rows at once. |
| `new_row` | — | The row a new-row form starts from. Without it there is no add button. Give it a key no other row has. |
| `validate_row` | — | Checks a whole row, after each column's own checks. |
| `confirm_delete` | `true` | Whether deleting asks first. |
| `toolbar` | `true` | Whether to show the Add, Edit, Delete and Save buttons. |

## Columns

| Method | Meaning |
|---|---|
| `.editable(\|row, value: V\| …)` | Makes the column editable. `V` is the field's type — `String`, `bool`, any integer or float, `NaiveDate`, `NaiveDateTime`, or an `Option` of one, which is the only one that accepts an empty cell. |
| `.edit(\|row, value: Option<Value>\| -> Result<(), EditError>)` | The same, taking the value as it is and able to refuse it. |
| `.validate(\|row\| -> Result<(), String>)` | Checks the row after this column changed; the message shows at the cell. |
| `.choices([...])` | Offers these values as a list. |
| `.editor(\|editor: CellEditor\| rsx! { … })` | Your own editor. Wire `editor.onkeydown` and `editor.onmounted` to keep keys and focus. |

A column without `.editable` stays read-only and carries `aria-readonly`.

## Modes

- **Cell:** `Enter` or `F2` edits the focused cell. `Enter` saves and moves down, `Tab` saves and
  edits the next editable cell, `Escape` cancels.
- **Row:** `Enter` or `F2` edits the whole row. `Tab` moves between its editors, `Enter` saves,
  `Escape` cancels.
- **Dialog:** `Enter` or `F2` opens a form with every editable column. Focus stays in the form until
  it closes.
- **Batch:** like Cell, but changes are collected, marked in their cells and counted, and saved or
  discarded together.

In every mode `Delete` deletes the focused row, or the selection if the focused row is part of it,
and *Add* opens the form for a new row. New rows appear in the grid once they are saved.

## Styling

The editor shares `data_grid`'s theme variables. Cells being edited carry `data-editing`, changed
cells in a batch `data-changed`, rows marked deleted `data-deleted`, and rows being saved
`data-saving`. The dialogs carry `data-edit-dialog` and `data-delete-confirm`.
