# Architecture

Four layers, each usable without the one above it.

```
  your app
     │
     ├── registry/data_grid …………… styled component, copied into your project, yours to edit
     │      + data_grid_editor              (thin: markup and CSS, < ~250 lines each)
     │      + data_grid_group_panel
     │
     ├── dioxus-datagrid …………………… use_grid / use_grid_remote, unstyled primitives
     │                                      the ARIA pattern, events, focus. No web-sys.
     │
     └── datagrid-core ……………………… sorting, filtering, grouping, aggregates, paging,
                                            selection, virtualization maths, navigation.
                                            Pure Rust. No Dioxus.
```

Pick your level: install the component and be done, compose the primitives with your own markup, or
drive the core from something that is not Dioxus at all.

## The boundaries, and why they are worth keeping

**`datagrid-core` knows nothing about Dioxus or the DOM.** Everything that can be decided without a
renderer is decided there, as a pure function of the rows, the columns and the state. That is what
makes the interesting parts testable at speed: around 170 tests, property tests among them, run in
milliseconds with no browser and no DOM.

**`dioxus-datagrid` has no `web-sys`.** Scrolling goes through `onscroll`, measurements through
`onmounted` and `get_client_rect()`, resizing through `onresize`. The same code therefore runs on
web, desktop and mobile WebViews rather than on web alone. CI enforces it: a job asserts that
`cargo tree -p dioxus-datagrid -e normal` mentions no `web-sys`.

**The registry components stay thin.** They are copied into user projects, where our updates no
longer reach them. Anything with logic in it belongs in a crate, so a fix reaches everyone through
a version bump. Styling goes only through the dx-components theme variables — never a hard-coded
colour — so an installed grid matches the rest of a user's components.

These three rules live in [`CLAUDE.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/CLAUDE.md)
and are checked in CI, not just believed.

## From a keystroke to a rendered row

The centre of the whole design is one function:

```rust
compute_view(&rows, &columns, &state) -> View
```

`View` does not contain rows. It contains a list of **row kinds**, each pointing into the slice you
already own:

```rust
enum ViewRow {
    Data(usize),         // an index into your rows
    GroupHeader(Group),  // "Berlin — 42 rows": level, key, expanded
    GroupFooter(Group),  // the aggregates for that group
}
```

Sorting, filtering, search, grouping and paging all happen by rearranging that list. A view over a
hundred thousand expensive rows costs a vector of indices and nothing else — no clones, no copies of
your data.

Everything that addresses a row by position then walks the same list: the renderer, the keyboard,
virtualization, `aria-rowindex`, editing. That is why grouping could be added without touching each
of them separately — and why the row-kinds rewrite (A2 in the roadmap) was done as its own step,
with every existing test as the safety net, before the first group existed.

The full round trip:

```
 user event ──> GridHandle method ──> GridState changes ──> compute_view ──> View.rows
                                                                                │
   focus, aria-rowindex, virtual window  <───────────────────────────────────────┤
                                                                                │
             GridRow dispatches per kind ──> GridCell / GridGroupRow / GridGroupFooterRow
```

`GridHandle` is `Copy`, so it can be passed around freely and put into a context. It is the whole
public surface for building controls of your own: `toggle_sort`, `set_filter`, `set_search`,
`set_page`, `set_column_hidden`, `set_column_width`, `select`, `group_by_column`, `toggle_group`,
`start_edit`, `commit_edit`, `save_changes`, `state` / `set_state`.

## One column, one value

A column is described once. `value`, `value_of` or `value_text` yields a typed `CellValue` — text,
integer, float, bool, date, date-time or empty — and sorting, filter operators, formatting,
aggregates, editing and, later, export all read that same value.

The alternative, which the project started with, was a separate closure per concern: a sort key
here, a filter string there. Every new feature then means another closure on every column. Making
the value typed and shared (ADR-0022) is what keeps `Column::new(…)` short while the feature list
grows.

## Server-side data

`use_grid_remote` swaps the source of truth without changing anything above it: the same handle, the
same primitives, the same component.

```
state change ──> GridQuery ──(your DataSource)──> Page<T> ──> View
```

`GridQuery` carries the sort, the filter bar, the typed column filters, the search, the page, and —
since Phase 10 — the grouping, the collapsed groups and the aggregates wanted. `Page` carries the
rows plus, optionally, a layout of group rows, the groups, the row count and the totals. Every field
is `#[serde(default)]`, so an old server understands a new client's query and a new server can answer
an old client (A6 in the roadmap).

Two ways to answer a grouped query, both shown in the examples:

- **Rows in memory:** `Page::from_view(&compute_view(&rows, &columns, &query.state()), &rows)` — the
  server reuses the very core the client would have used.
- **SQL:** count the groups with one `GROUP BY` per level, hand the counts to `GroupedPage::plan`,
  and it works out which group headers, footers and row ranges that page actually shows. Only those
  rows are fetched. `examples/fullstack` does this against SQLite, and a test compares it with the
  local grid page for page.

Column ids in a query come from the client. Map them through a fixed list of allowed columns and
bind all filter text as parameters — never build SQL from the ids or the text.

## The registry family

`data_grid` is the entry point and does not grow. It puts its `GridHandle` into a context and takes
`children`; add-ons are children that take the handle back out:

```rust
DataGrid { data, columns,
    DataGridGroupPanel::<Order> {}
    DataGridEditor { on_save }
}
```

Where an add-on has to reach *inside* the table — an editor in a cell, a filter button in a header —
it registers at an extension point the core component offers. `data_grid` does not know the add-ons
exist, and what you do not install is not compiled.

The add-ons deliberately do **not** declare `componentDependencies`: a spike (ADR-0025) found the
mechanism not dependable enough to build on, so the popups are plain primitives of our own instead.

## Accessibility is part of the architecture, not a pass at the end

The primitives *are* the ARIA pattern: roles, `aria-rowcount` and `aria-rowindex` that stay right
across pages and through a virtual window, a roving tabindex so the grid is a single tab stop, and
`role="treegrid"` with `aria-level`, `aria-expanded`, `aria-posinset` and `aria-setsize` once rows
are grouped. Keyboard handling lives in the core (`navigate.rs`) and is unit-tested without a
browser; what actually reaches the DOM is tested by rendering to a string and by axe in a real one.

The complete, current list of what is rendered is
[`docs/ACCESSIBILITY.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/docs/ACCESSIBILITY.md).

## Where to look in the source

| | |
|---|---|
| `crates/datagrid-core/src/view.rs` | `compute_view`, `ViewRow`, the paging and grouping walk |
| `crates/datagrid-core/src/column.rs`, `value.rs`, `format.rs` | Column spec, typed values, formats |
| `crates/datagrid-core/src/filter.rs`, `sort.rs`, `group.rs` | Filter operators, sorting, groups, aggregates, the server-side page planner |
| `crates/datagrid-core/src/navigate.rs`, `virtualize.rs`, `selection.rs` | Keyboard, virtual window, selection |
| `crates/datagrid-core/src/remote.rs`, `locale.rs`, `state.rs` | `GridQuery` and `Page`, all texts and formats, `GridState` |
| `crates/dioxus-datagrid/src/grid.rs`, `remote.rs` | `use_grid`, `use_grid_remote`, `GridHandle` |
| `crates/dioxus-datagrid/src/primitives.rs` | `GridRoot`, `GridHeader`, `GridBody`, `VirtualGridBody`, `GridRow`, `GridCell`, `GridPagination`, `GridSearch`, … |
| `crates/dioxus-datagrid/src/group_ui.rs`, `edit_ui.rs`, `filter_menu.rs` | The primitives for grouping, editing and the filter menu |
| `registry/*/component.rs` | The styled components users install |

Why a given thing is the way it is: 28 decision records in
[`docs/DECISIONS.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/docs/DECISIONS.md)
(German).
