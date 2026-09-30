# data_grid_reorder

Row reordering for [`data_grid`](../data_grid/docs.md): drag a row into another place by its handle,
or move it with `Alt+Shift+ArrowUp` and `Alt+Shift+ArrowDown`.

Install `data_grid` first, then this:

```bash
dx components add data_grid --git https://github.com/dioxus-datagrid/dioxus-datagrid
dx components add data_grid_reorder --git https://github.com/dioxus-datagrid/dioxus-datagrid
```

The logic lives in the [`dioxus-datagrid`][crate] crate; this file only says what a move does.

[crate]: https://github.com/dioxus-datagrid/dioxus-datagrid

## Usage

Two things: a column that holds the handles, and `DataGridReorder` inside the grid.

```rust
use dioxus::prelude::*;
use dioxus_datagrid::{Column, GridRow, RowMove};

use crate::components::data_grid::DataGrid;
use crate::components::data_grid_reorder::DataGridReorder;

#[derive(Clone, PartialEq)]
struct Task {
    id: u32,
    title: String,
}

impl GridRow for Task {
    type Key = u32;
    fn key(&self) -> u32 {
        self.id
    }
}

#[component]
fn Tasks() -> Element {
    let mut tasks = use_signal(Vec::<Task>::new);
    let columns = use_hook(|| {
        vec![
            // The handles live in a column of their own.
            Column::new("order", "").drag_handle(),
            Column::new("title", "Title").cell(|task: &Task| rsx! { "{task.title}" }),
        ]
    });

    rsx! {
        DataGrid { data: tasks, columns,
            DataGridReorder {
                on_move: move |moved: RowMove<Task>| {
                    tasks.with_mut(|tasks| moved.apply(tasks));
                },
            }
        }
    }
}
```

## Props

| Prop | What it does |
|---|---|
| `on_move` | Takes a row that was moved. Required. |

`RowMove { row, from, to }` carries indices into the rows you gave the grid, and `to` is where the
row ends up. `moved.apply(&mut rows)` does exactly that to a `Vec`; `move_row` in `datagrid-core` is
the same move if you would rather call it yourself. The grid never changes your rows.

## What the grid does with it

- **Only where the order on screen is the rows' own.** A sorted or grouped grid draws no handles and
  moves nothing from the keyboard: a row moved inside an order of the grid's making would say
  nothing to the rows underneath. A filter is fine — it leaves the order as it is.
- **The keyboard does the same thing.** `Alt+Shift+ArrowUp` and `Alt+Shift+ArrowDown` on any cell of
  the row move it one row, counted in the rows you can see, and the focus goes with it. `Escape`
  calls off a drag.
- **A touch scrolls the page.** A touch pointer is held by the element it starts on, so the rows
  under the finger never hear of it; rather than offer a gesture that does nothing, the handle lets
  a touch through. The keyboard is the way that works everywhere.
- While a row is dragged it carries `data-dragging`, and the row under the pointer carries
  `data-drop="before"` or `"after"`.

## Styling

The handle is styled by `data_grid`'s own stylesheet: `[data-row-handle]` for the grip, plus the
rules for `[data-dragging]` and `[data-drop]`. This component brings no stylesheet of its own.
