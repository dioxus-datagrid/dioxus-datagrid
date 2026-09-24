# data_grid_column_menu

A menu at every column header of [`data_grid`](../data_grid/docs.md): sort the column, group the
rows by it, hold it at either edge while the grid scrolls sideways, hide it, or size it back to
its content.

Install `data_grid` first, then this:

```bash
dx components add data_grid --git https://github.com/dioxus-datagrid/dioxus-datagrid
dx components add data_grid_column_menu --git https://github.com/dioxus-datagrid/dioxus-datagrid
```

The logic lives in the [`dioxus-datagrid`][crate] crate; this file is the styled shell.

[crate]: https://github.com/dioxus-datagrid/dioxus-datagrid

## Usage

Put `DataGridColumnMenu` inside the `DataGrid`, naming the row type:

```rust
use dioxus::prelude::*;
use dioxus_datagrid::{Column, GridRow};

use crate::components::data_grid::DataGrid;
use crate::components::data_grid_column_menu::DataGridColumnMenu;

#[component]
fn Orders() -> Element {
    let orders = use_signal(load_orders);
    let columns = use_hook(|| vec![
        Column::new("customer", "Customer").value_text(|order: &Order| order.customer.as_str()),
        Column::new("total", "Total").value_of(|order: &Order| order.total),
    ]);

    rsx! {
        DataGrid { data: orders, columns,
            DataGridColumnMenu::<Order> {}
        }
    }
}
```

The component renders no markup of its own. It tells the grid that a menu is wanted, and the
headers render it — so `data_grid` does not have to know this component exists, and a project
without it compiles none of it.

## What a column offers

Only what would do something. A column with no value cannot be sorted or grouped by, so it offers
neither; "Clear sort" appears once the column is sorted, "Unpin" once it is pinned, and "Fit width
to content" once its width has been dragged. The last visible column is not offered for hiding,
because the grid refuses to hide it.

| Entry | What it does |
|---|---|
| Sort ascending / descending | Sorts by this column alone, replacing any other sort |
| Clear sort | Takes this column out of the sort, leaving the others |
| Group by this column | Groups the rows; needs `data_grid_group_panel` to ungroup by mouse, or use the menu again |
| Pin to the start / end | Holds the column at that edge while the grid scrolls sideways |
| Unpin | Lets it scroll with the rest again |
| Hide column | Hides it; `column_picker` on `data_grid` brings it back |
| Fit width to content | Forgets a width set by dragging |

Filtering is deliberately not in this menu: `data_grid` already puts a filter menu at each column
when `filter_menu` is set, and two ways to reach the same panel would be two places to look.

## Keyboard

The grid stays a single tab stop, so the button is **not** one: it carries `tabindex="-1"` and is
opened with `Alt+↓` on the focused column header, the usual key for opening a menu attached to a
control.

| Key | Effect |
|---|---|
| `Alt+↓` on a column header | Opens its menu and puts focus on the first entry |
| `↓` / `↑` | Next / previous entry, wrapping |
| `Home` / `End` | First / last entry |
| `Enter` / `Space` on an entry | Chooses it and closes the menu |
| `Escape` | Closes the menu; focus returns to the column header |

Closing returns focus to the header cell rather than to the button, because the button is not a
tab stop: focus parked there would leave the grid's arrow keys with nothing to move.

## Styling

Everything is addressed by data attributes, so the CSS is yours to replace:

| | |
|---|---|
| `[data-column-menu]` | The wrapper inside the header cell |
| `[data-column-menu-trigger]` | The button; `aria-expanded` says whether it is open |
| `[data-column-menu-backdrop]` | The full-page layer that closes the menu on a click outside |
| `[data-column-menu-panel]` | The `role="menu"` panel |
| `[data-column-menu-item]` | One entry; `aria-checked` marks the state that holds |
| `[data-column-menu-action]` | What the entry does: `sort-asc`, `pin-start`, `hide`, … |

The trigger is invisible until its header is hovered, the menu is open, or it has keyboard focus.
Remove the `opacity` rules to show it always.

## Building your own

`column_menu_entries(&grid, column_index)` returns what a column offers as
`Vec<ColumnMenuEntry>` — the action, its text in the grid's language, and whether it is the state
that holds. `ColumnAction::apply(&mut grid, column)` carries one out. A menu of your own can use
both and skip this component entirely.
