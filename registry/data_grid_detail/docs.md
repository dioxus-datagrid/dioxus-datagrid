# data_grid_detail

Detail rows for [`data_grid`](../data_grid/docs.md): a row opens a second one under itself, holding
whatever you render for it — a paragraph, a form, a grid of its own.

Install `data_grid` first, then this:

```bash
dx components add data_grid --git https://github.com/dioxus-datagrid/dioxus-datagrid
dx components add data_grid_detail --git https://github.com/dioxus-datagrid/dioxus-datagrid
```

The logic lives in the [`dioxus-datagrid`][crate] crate; this file only says what a row shows.

[crate]: https://github.com/dioxus-datagrid/dioxus-datagrid

## Usage

Two things: a column that holds the buttons, and `DataGridDetail` inside the grid.

```rust
use dioxus::prelude::*;
use dioxus_datagrid::{Column, GridRow};

use crate::components::data_grid::DataGrid;
use crate::components::data_grid_detail::DataGridDetail;

#[derive(Clone, PartialEq)]
struct Order {
    id: u32,
    customer: String,
    lines: Vec<String>,
}

impl GridRow for Order {
    type Key = u32;
    fn key(&self) -> u32 {
        self.id
    }
}

#[component]
fn Orders() -> Element {
    let orders = use_signal(Vec::<Order>::new);
    let columns = use_hook(|| {
        vec![
            // The buttons live in a column of their own.
            Column::new("expand", "").expander(),
            Column::new("customer", "Customer")
                .cell(|order: &Order| rsx! { "{order.customer}" })
                .sort_by_text(|order: &Order| order.customer.as_str()),
        ]
    });

    rsx! {
        DataGrid { data: orders, columns,
            DataGridDetail {
                render: move |order: Order| rsx! {
                    ul {
                        for line in order.lines.iter() {
                            li { "{line}" }
                        }
                    }
                },
            }
        }
    }
}
```

## Props

| Prop | What it does |
|---|---|
| `render` | Renders the detail of a row. Required. |
| `has_detail` | Whether a row has one at all. Every row has one without it. |

`has_detail` decides where a button appears: a row it says `false` for gets none and says nothing
about a state it does not have. Make it in a hook — `use_hook(|| Callback::new(…))` — so it is the
same callback on every render.

## What the grid does with it

- The detail is **a row of the view**: it has its own `aria-rowindex`, `aria-rowcount` counts it, it
  follows its row through sorting and filtering, and it takes a place on the page — so a row can end
  one page with its detail starting the next, as a group can.
- **`Enter` on the button's cell** opens and closes it, and so does a click anywhere in that cell.
  Neither the button nor anything in the detail adds a tab stop to the grid itself; what you render
  inside is part of the page like anything else.
- **`aria-expanded` sits on the button** and, while the detail is there, `aria-controls` points at
  it. A row carries that only in a treegrid, which this is not.
- **Not in a virtualized or remote grid.** Rows placed by counting equal heights, or paged by a
  server, would land in the wrong place, so the buttons are not drawn at all.

## Styling

The detail row is styled by `data_grid`'s own stylesheet: `[data-detail-row]` for the row,
`[data-detail-cell]` for the cell across it, `[data-expander]` for the button. This component brings
no stylesheet of its own.
