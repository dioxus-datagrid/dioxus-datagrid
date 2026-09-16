# data_grid

A typed data grid: sorting, filtering, global search, paging, selection and full
keyboard navigation.

The heavy lifting lives in the [`dioxus-datagrid`][crate] crate, which
`dx components add` puts in your `Cargo.toml`. This file is just the styled
shell — change the markup and the classes however you like, and bug fixes to the
grid logic still reach you through the crate.

[crate]: https://github.com/dioxus-datagrid/dioxus-datagrid

## Usage

```rust
use dioxus::prelude::*;
use dioxus_datagrid::{Column, GridRow, SelectionMode};

use crate::components::data_grid::DataGrid;

#[derive(Clone, PartialEq)]
struct User {
    id: u32,
    name: String,
    email: String,
    age: u32,
}

// Rows need a stable identity so selection survives sorting and paging.
impl GridRow for User {
    type Key = u32;
    fn key(&self) -> u32 {
        self.id
    }
}

#[component]
fn Users() -> Element {
    let users = use_signal(load_users);

    // `use_hook` so the closures are built once, not on every render.
    let columns = use_hook(|| {
        vec![
            Column::new("name", "Name")
                .cell(|user: &User| rsx! { "{user.name}" })
                .sort_by_text(|user: &User| user.name.as_str())
                .filter_by(|user: &User| user.name.clone()),
            Column::new("email", "Email")
                .cell(|user: &User| rsx! { "{user.email}" })
                .sort_by_text(|user: &User| user.email.as_str())
                .filter_by(|user: &User| user.email.clone()),
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
                tracing::info!("selected {keys:?}");
            },
        }
    }
}
```

## Props

| Prop | Default | Meaning |
|---|---|---|
| `data` | — | The rows to show. |
| `columns` | — | The columns to show them in. |
| `page_size` | `None` | Rows per page; omit to show all rows. |
| `selection` | `SelectionMode::None` | `None`, `Single` or `Multi`. |
| `searchable` | `true` | Whether to show the search box. |
| `search_placeholder` | `"Search"` | Placeholder for the search box. |
| `empty_message` | `"No matching rows"` | Shown when nothing matches. |
| `on_selection_change` | — | Fires with the selected keys whenever they change. |

## Columns

A column only does what you give it a closure for:

- `.cell(...)` — how the cell renders. Without it the column renders empty.
- `.sort_by_text(...)` — sortable by text borrowed from the row.
- `.sort_by_value(...)` — sortable by a number, bool, or `Option` of one.
- `.filter_by(...)` — takes part in column filters and in the global search.
- `.width(ColumnWidth::Px(120.0))` — a fixed track instead of the default
  `minmax(6rem, auto)`. `Fraction` gives it a share of the free space.

Sorting text is case-insensitive by default; `.collation(TextCollation::CaseSensitive)`
changes that per column.

## Keyboard

The grid is one tab stop. Inside it, arrow keys move between cells, `Home`/`End`
jump within a row and `Ctrl+Home`/`Ctrl+End` to the corners. On a header,
`Enter` sorts and `Shift+Enter` adds that column to a multi-column sort. On a
row, `Space` selects and `Shift+Space` or `Shift+Arrow` extends the selection.

## Styling

Every colour comes from the dx-components theme variables, so the grid matches
the official components and follows light and dark mode without extra work. The
component links both stylesheets itself.

Class names are prefixed `dg-`. State is exposed as data attributes you can
style against: `data-sortable`, `data-sorted`, `data-sort-priority` on headers,
and `data-selected` on rows.

> This file assumes the default `components_dir` of `src/components`, because it
> loads its stylesheet from `/src/components/data_grid/style.css`. If you
> configure a different directory in `Dioxus.toml`, adjust the `asset!` path at
> the top of `component.rs`.
