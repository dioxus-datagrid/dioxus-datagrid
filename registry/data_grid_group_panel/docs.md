# data_grid_group_panel

Grouping for [`data_grid`](../data_grid/docs.md): drag a column header onto the panel, or pick a
column from its list, and the grid groups its rows by that column. Groups expand and collapse, and
columns with aggregates show them under every group and under the whole grid.

Install `data_grid` first, then this:

```bash
dx components add data_grid --git https://github.com/dioxus-datagrid/dioxus-datagrid
dx components add data_grid_group_panel --git https://github.com/dioxus-datagrid/dioxus-datagrid
```

The logic lives in the [`dioxus-datagrid`][crate] crate; this file is the styled shell.

[crate]: https://github.com/dioxus-datagrid/dioxus-datagrid

## Usage

Put `DataGridGroupPanel` inside the `DataGrid`, naming the row type:

```rust
use dioxus::prelude::*;
use dioxus_datagrid::{Aggregate, Column, GridRow};

use crate::components::data_grid::DataGrid;
use crate::components::data_grid_group_panel::DataGridGroupPanel;

#[component]
fn Orders() -> Element {
    let orders = use_signal(load_orders);

    let columns = use_hook(|| {
        vec![
            Column::new("customer", "Customer").value_text(|order: &Order| order.customer.as_str()),
            Column::new("region", "Region").value_text(|order: &Order| order.region.as_str()),
            Column::new("total", "Total")
                .value_of(|order: &Order| order.total)
                .aggregate(Aggregate::Sum)
                .aggregate(Aggregate::Average),
        ]
    });

    rsx! {
        DataGrid { data: orders, columns,
            DataGridGroupPanel::<Order> {}
        }
    }
}
```

Without the panel a grid can still be grouped: start it grouped with `initial_state`, or call
`group_by_column` on the grid's handle.

```rust
DataGrid {
    data: orders,
    columns,
    initial_state: GridState {
        group_by: vec!["region".into()],
        ..GridState::default()
    },
}
```

## Grouping

- **Drag** a column header onto the panel to group by it; later ones group inside earlier ones.
- **The list** in the panel does the same without dragging.
- Each grouped column has a button to **group by it first**, one level further out, and one to
  **stop grouping** by it.
- **Expand all** and **Collapse all** act on every group.

Groups follow the sort of their column, ascending until the column is sorted. Inside a group the
rest of the sort applies. With paging, group headers and footers take a place on the page like
rows, and a collapsed group takes one.

A column without a value cannot be grouped by, nor one marked `.groupable(false)`.

## Aggregates

| Aggregate | Result |
|---|---|
| `Aggregate::Sum` | The sum of the numbers. |
| `Aggregate::Average` | Their mean. |
| `Aggregate::Min`, `Aggregate::Max` | The smallest and largest value, in the column's order: numbers, dates or text. |
| `Aggregate::Count` | How many rows have a value. |
| `Aggregate::custom("Label", \|rows\| …)` | Anything computed from the rows, such as a weighted average. |

Empty cells are skipped. Results use the column's format and the grid's locale. An expanded group
shows them in a footer row under its rows, a collapsed group in its header, and the grid in a
footer that stays at the bottom while the rows scroll.

## Keyboard

A grouped grid is a WAI-ARIA treegrid. On a group's header:

| Key | Action |
|---|---|
| `ArrowRight` | Expands the group. |
| `ArrowLeft` | Collapses it; on a collapsed group, moves to the group it is in. |
| `Enter`, `Space` | Expand or collapse. |

`ArrowUp` and `ArrowDown` move through headers, rows and footers alike; `Ctrl+End` reaches the
totals.

## Styling

The panel carries `data-drop-target` while a header is being dragged and `data-drag-over` while it
is over the panel. In the grid, group headers carry `data-group-row`, `data-group-level` and
`data-expanded`; their cell sets `--dg-group-level` for indenting. Group footers carry
`data-group-footer`, the grid's footer `data-footer`, and each aggregate `data-aggregate` with the
kind as its value.
