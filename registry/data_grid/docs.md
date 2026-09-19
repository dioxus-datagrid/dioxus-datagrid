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
| `search_placeholder` | from `locale` | Placeholder for the search box. |
| `empty_message` | from `locale` | Shown when nothing matches. |
| `locale` | `GridLocale::english()` | Every text the grid writes and how it formats numbers and dates. |
| `on_selection_change` | — | Fires with the selected keys whenever they change. |
| `column_filters` | `false` | Shows a filter input for every filterable column. |
| `filter_menu` | `false` | Shows a filter menu for every filterable column: conditions, or a value list. |
| `row_height` | `None` | Virtualizes the grid: only rows in view are rendered, each exactly this many pixels tall. |
| `height` | `None` | A CSS height, such as `"480px"`, that the grid scrolls within. |
| `overscan` | `20` | With `row_height`: extra rows rendered above and below the visible ones. |
| `resizable_columns` | `true` | Lets the user resize columns by dragging a header's edge. |
| `column_picker` | `false` | Shows a menu for showing and hiding columns. |
| `column_picker_label` | from `locale` | Label of that menu. |
| `initial_state` | `None` | A `GridState` to start from, read on the first render. |
| `on_state_change` | — | Fires with the whole `GridState` whenever it changes. |
| children | — | Add-ons such as `DataGridEditor` or `DataGridGroupPanel`, placed above the grid. |

## Editing

Install [`data_grid_editor`](../data_grid_editor/docs.md) and put a `DataGridEditor` inside the
`DataGrid`: cells, rows, a form dialog or batches, with adding, deleting and validation.

## Totals and grouping

A column with `.aggregate(Aggregate::Sum)` — or `Average`, `Min`, `Max`, `Count`, or
`Aggregate::custom(...)` — gets its result in a footer row under the grid, which stays in view
while the rows scroll. The keyboard reaches it after the last row.

To group rows, install [`data_grid_group_panel`](../data_grid_group_panel/docs.md) and put a
`DataGridGroupPanel::<Row>` inside the `DataGrid`, or start grouped with `initial_state`. Group
headers expand and collapse; a column that should not be offered for grouping takes
`.groupable(false)`.

## Filtering

With `column_filters`, each filterable column gets a text box. Plain text
searches text columns and means "equals" on numbers, dates and booleans; a
leading operator compares: `>100`, `<=2026-01-01`, `!=Berlin`, `10..20`.

With `filter_menu`, each also gets a menu: one or two conditions joined by
*and* or *or*, with the operators that suit the column, or a list of the
column's values with counts to tick, as in a spreadsheet. A column is
filterable when it has a value or `filter_by` text.

```rust
DataGrid { data: users, columns, column_filters: true, filter_menu: true }
```

Filters are part of the grid state, so they persist with `on_state_change`
like sort and page do.

## Formats and languages

A column with a value and no `.cell()` shows the value itself, formatted:

```rust
Column::new("salary", "Salary")
    .value_of(|user: &User| user.salary)
    .format(CellFormat::currency("€", 2)),
Column::new("joined", "Joined")
    .value_of(|user: &User| user.joined) // a chrono::NaiveDate, with the `chrono` feature
    .format(CellFormat::Date),
```

`CellFormat` has `number`, `currency`, `percent`, `Date`, `DateTime` and
`date_pattern`. Numeric formats align the column at the end; `.align()`
overrides that, and `.overflow(CellOverflow::TruncateWithTooltip)` shows cut-off
text as a tooltip.

Separators, symbol placement, date order and every text the grid writes come
from `locale`. English is the default and German ships with the crate:

```rust
DataGrid { data: users, columns, locale: GridLocale::german() }
```

For another language, start from one of them and change the fields —
`GridLocale { search: "Rechercher".into(), ..GridLocale::german() }`. Column
labels are yours and are not translated.

## Large data sets

For tens of thousands of rows, set `row_height` and `height` and leave out `page_size`:

```rust
DataGrid {
    data: measurements,
    columns,
    row_height: 40.0,
    height: "70vh",
}
```

The grid then renders only the rows in view plus a few above and below, while
the scrollbar and `aria-rowcount` still reflect every row. Keyboard navigation
scrolls to the focused row, and `PageUp` / `PageDown` move by one screen.

Every row is exactly `row_height` pixels tall and taller content is clipped, so
pick a height that fits your cells. With the default styling, 40 pixels fits one
line of text.

On touch devices a fast fling can outrun rendering. The default `overscan` of 20
keeps rows ready for that; on the Android emulator 5 showed blank rows and 40
made scrolling sluggish. Lower it if your cells are expensive to render.

## Resizing and hiding columns

Drag the edge of a header to resize its column; pressing the edge twice
without dragging (a double-click or double tap) restores the column's own width.
From the keyboard, focus a header and press `Alt+ArrowLeft` or
`Alt+ArrowRight`. A column never gets narrower than its `.min_width(...)`, or
48 pixels without one. `.resizable(false)` opts a column out.

With `column_picker` a menu lists the columns with a checkbox each. The last
visible column cannot be hidden. Columns defined with `.hidden()` are left out:
hiding those is the app's decision.

## Saving the grid's state

Sort, filters, search, page, column widths and hidden columns together form a
`GridState`. With the crate's `serde` feature it serializes, so an app can keep
it wherever it likes:

```rust
// In Cargo.toml: dioxus-datagrid = { version = "0.4", features = ["serde"] }
let saved = use_resource(load_state_from_storage);

rsx! {
    // initial_state is read once, so wait until the saved state has loaded.
    if let Some(initial_state) = saved.read().clone() {
        DataGrid {
            data: users,
            columns,
            initial_state,
            on_state_change: move |state: GridState| save_state_to_storage(&state),
        }
    }
}
```

State saved by an older version still loads: missing fields fall back to their
defaults, and ids of columns that no longer exist are ignored. The playground
keeps its state in `localStorage` this way.

## Columns

A column only does what you give it a closure for:

- `.cell(...)` — how the cell renders. Without it the column renders empty.
- `.sort_by_text(...)` — sortable by text borrowed from the row.
- `.sort_by_value(...)` — sortable by a number, bool, or `Option` of one.
- `.filter_by(...)` — takes part in column filters and in the global search.
- `.width(ColumnWidth::Px(120.0))` — a fixed track instead of the default
  `minmax(6rem, auto)`. `Fraction` gives it a share of the free space.
- `.min_width(80.0)` — the narrowest the user can resize it to.
- `.resizable(false)` — no resize handle for this column.
- `.hidden()` — not shown, and not offered in the column menu.
- `.aggregate(...)` — a total for the footer and every group footer.
- `.groupable(false)` — rows cannot be grouped by it.

Sorting text is case-insensitive by default; `.collation(TextCollation::CaseSensitive)`
changes that per column.

## Keyboard

The grid is one tab stop. Inside it, arrow keys move between cells, `Home`/`End`
jump within a row and `Ctrl+Home`/`Ctrl+End` to the corners. On a header,
`Enter` sorts and `Shift+Enter` adds that column to a multi-column sort. On a
row, `Space` selects and `Shift+Space` or `Shift+Arrow` extends the selection.
On a group header, `ArrowRight` expands, `ArrowLeft` collapses, and `Enter` or
`Space` toggle.

## Styling

Every colour comes from the dx-components theme variables, so the grid matches
the official components and follows light and dark mode without extra work. The
component links both stylesheets itself.

Class names are prefixed `dg-`. State is exposed as data attributes you can
style against: `data-sortable`, `data-sorted`, `data-sort-priority` on headers,
`data-selected` on rows, `data-resize-handle` on resize handles, and
`data-resizing` on the grid and the handle while a column is being resized.
Group headers carry `data-group-row` and `data-expanded`, group footers
`data-group-footer`, the totals `data-footer`, and each aggregate
`data-aggregate`.

> This file assumes the default `components_dir` of `src/components`, because it
> loads its stylesheet from `/src/components/data_grid/style.css`. If you
> configure a different directory in `Dioxus.toml`, adjust the `asset!` path at
> the top of `component.rs`.
