# dioxus-datagrid

A typed, accessible data grid for **Dioxus 0.7**: sorting, filtering, search, paging, selection
and full keyboard navigation. It follows the WAI-ARIA data grid pattern and runs on web, desktop
and mobile.

![The data_grid component, sorted by department with two rows selected](docs/screenshot.png)

[![crates.io](https://img.shields.io/crates/v/dioxus-datagrid.svg)](https://crates.io/crates/dioxus-datagrid)
[![docs.rs](https://docs.rs/dioxus-datagrid/badge.svg)](https://docs.rs/dioxus-datagrid)

## How it is put together

The logic is versioned in crates, so fixes reach everyone. The styled shell is copied into your
project by `dx components add` and is yours to change from then on.

| | |
|---|---|
| [`datagrid-core`](crates/datagrid-core) | Sorting, filtering, paging, selection, virtualization math. Pure Rust, no UI framework. |
| [`dioxus-datagrid`](crates/dioxus-datagrid) | The `use_grid` hook and unstyled primitives implementing the ARIA grid pattern. No `web-sys`. |
| [`registry/data_grid`](registry/data_grid) | The styled component that `dx components add data_grid` installs. |

## Installation

Point the Dioxus CLI at this repository as a component registry. Either per command:

```bash
dx components add data_grid --git https://github.com/dioxus-datagrid/dioxus-datagrid
```

or once, in your project's `Dioxus.toml`, after which a plain `dx components add data_grid` uses it:

```toml
[components]
registry = { git = "https://github.com/dioxus-datagrid/dioxus-datagrid" }
```

The command copies the component into `src/components/data_grid/`, adds `dioxus-datagrid` to your
`Cargo.toml` and copies the dx-components theme into `assets/`. If this is your first component,
add `mod components;` to `main.rs`.

## Usage

```rust
use dioxus::prelude::*;
use dioxus_datagrid::{Column, GridRow, SelectionMode};

use crate::components::data_grid::DataGrid;

#[derive(Clone, PartialEq)]
struct User {
    id: u32,
    name: String,
    age: u32,
}

// A stable identity, so selection survives sorting and paging.
impl GridRow for User {
    type Key = u32;
    fn key(&self) -> u32 {
        self.id
    }
}

#[component]
fn Users() -> Element {
    let users = use_signal(load_users);

    let columns = use_hook(|| {
        vec![
            Column::new("name", "Name")
                .cell(|user: &User| rsx! { "{user.name}" })
                .sort_by_text(|user: &User| user.name.as_str())
                .filter_by(|user: &User| user.name.clone()),
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
            on_selection_change: move |keys: Vec<u32>| { /* ... */ },
        }
    }
}
```

For large data sets, set `row_height` and `height` instead of `page_size`: only the rows in view
are rendered, while the scrollbar, keyboard navigation and `aria-rowcount` still cover every row.
[`examples/virtualized`](examples/virtualized) shows 100,000 rows built directly on the primitives.

The installed [`docs.md`](registry/data_grid/docs.md) lists every prop and column option.

## Headless usage

If you want your own markup, skip the registry component and compose the primitives. They render
the roles, ARIA attributes and keyboard handling, and nothing else.

```rust
use dioxus_datagrid::primitives::{GridBody, GridHeader, GridPagination, GridRoot, GridSearch};
use dioxus_datagrid::{GridOptions, SelectionMode, use_grid};

let grid = use_grid(users, columns, GridOptions::paged(25).selection(SelectionMode::Multi));

rsx! {
    GridSearch { grid, class: "my-search" }
    GridRoot { grid, class: "my-grid",
        GridHeader { grid }
        GridBody { grid }
    }
    GridPagination { grid }
}
```

`GridHandle` is `Copy` and exposes the state directly — `toggle_sort`, `set_filter`, `set_search`,
`set_page`, `select`, `toggle_select`, `clear_selection`, `state` / `set_state` — for building
controls of your own.

## Accessibility

The grid is a single tab stop with a roving tabindex. Arrow keys move between cells, `Home`/`End`
within a row, `Ctrl+Home`/`Ctrl+End` to the corners. `Enter` on a header sorts, `Shift+Enter` adds
a sort column. `Space` selects a row, `Shift+Space` and `Shift+Arrow` extend the selection.
`aria-rowcount` and `aria-rowindex` stay correct across pages.

[`docs/ACCESSIBILITY.md`](docs/ACCESSIBILITY.md) documents exactly what is rendered.

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

The playground renders the registry component with controls for every mode:

```bash
dx serve --web --package playground
```

It carries a copy of `registry/data_grid`. After changing the component, refresh the copy — CI
fails if the two differ:

```bash
bash scripts/sync-playground-component.sh
```

End-to-end tests run against the playground; with it already serving on port 8080 they reuse it:

```bash
cd tests/e2e && npm ci && npx playwright test
```

The registry smoke test runs `dx components add` against a fresh project and compiles it:

```bash
bash scripts/registry-smoke.sh
```

Planning and design records are in German: [`PLAN.md`](PLAN.md), [`docs/DECISIONS.md`](docs/DECISIONS.md),
[`docs/VERIFICATION.md`](docs/VERIFICATION.md).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT)
at your option.

The bundled `dx-components-theme.css` is taken from
[DioxusLabs/components](https://github.com/DioxusLabs/components), under the same terms.
