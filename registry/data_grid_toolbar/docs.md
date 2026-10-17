# data_grid_toolbar

A toolbar for [`data_grid`](../data_grid/docs.md): one row above the table with the actions people reach for.

## Usage

```rust
use crate::components::data_grid::DataGrid;
use crate::components::data_grid_editor::DataGridEditor;
use crate::components::data_grid_toolbar::DataGridToolbar;

rsx! {
    // The toolbar brings its own search box and column picker.
    DataGrid { data: people, columns, searchable: false, column_picker: false,
        DataGridEditor { toolbar: false, on_save, on_delete, new_row }
        DataGridToolbar::<Person> {
            button { onclick: move |_| refresh(), "Refresh" }
        }
    }
}
```

## What is in it

From the start of the row: the **search box**, the **add, edit and delete buttons**, then **your own
entries**. At the end: the **row count** (with how many are selected) and the **column picker**.

| Prop | Default | |
| --- | --- | --- |
| `search` | `true` | The search box. |
| `actions` | `true` | Add, edit and delete. They appear only where a `DataGridEditor` made the grid editable, and only the ones it can do (no `on_delete`, no delete button). |
| `count` | `true` | Rows, and how many of them are selected. |
| `column_picker` | `true` | Show and hide columns. |
| `search_placeholder` | the locale's | |

Anything between the braces is yours: a button, a `DataGridExport`, a link. A plain `button` directly
inside is styled like the rest.

## One of each

`DataGrid` has a search box and a column picker of its own (`searchable`, `column_picker`), and
`DataGridEditor` has its buttons (`toolbar`). Switch those off when you use this component, or they are on
screen twice. They stay, so a grid without a toolbar component is as it was.

## Accessibility

The row is a `group` named after the locale's `toolbar` text, not a `toolbar`: that role promises arrow keys
between its controls, and a search box and a disclosure inside it would fight them. Every control is a tab
stop of its own.
