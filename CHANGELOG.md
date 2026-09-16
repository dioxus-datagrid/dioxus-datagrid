# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crates follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-09-16

The first release.

### Added

#### `datagrid-core`

- `compute_view`: filter, stable multi-column sort and paging over row indices, cloning no rows.
- `SortValue` with a total order: missing values last, integers and floats comparable with each
  other, `NaN` ordered, text case-insensitive by default with a per-column `TextCollation`.
- Sort keys borrow from the row (`SortValue<'a>`), avoiding an allocation per row per sort column.
- Column filters (case-insensitive substring, combined with AND) and a global search across visible
  filterable columns.
- `GridState` holding sort, filters, search, paging, column widths and hidden columns, with
  optional `serde` support that tolerates missing fields.
- `Selection` with single and multi modes and anchored range selection, keyed by row identity.
- `visible_range`, `total_height` and `offset_of` for virtualization, in bounds for any input.
- `navigate`, the ARIA grid keyboard movement rules as a pure state machine.

#### `dioxus-datagrid`

- `use_grid` returning a `Copy` `GridHandle` that owns grid state and recomputes the view.
- `Column<T>` builder pairing sort and filter keys with cell and header renderers.
- Unstyled primitives implementing the WAI-ARIA data grid pattern: `GridRoot`, `GridHeader`,
  `GridHeaderCell`, `GridBody`, `GridRow`, `GridCell`, `GridPagination`, `GridSearch` and
  `GridColumnFilter`.
- Roving tabindex with arrow, Home/End, Ctrl+Home/End and PageUp/PageDown navigation; Enter and
  Shift+Enter to sort; Space, Shift+Space and Shift+Arrow to select.
- `aria-rowcount` and `aria-rowindex` that count the header row and stay correct across pages.

#### Registry

- The `data_grid` component, installable with `dx components add data_grid`, styled entirely with
  the dx-components theme variables, with optional search, per-column filters and paging.

[Unreleased]: https://github.com/dioxus-datagrid/dioxus-datagrid/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/dioxus-datagrid/dioxus-datagrid/releases/tag/v0.1.0
