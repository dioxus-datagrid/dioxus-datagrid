# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crates follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.5.0] - 2026-09-18

### Added

- Typed cell values: `ColumnSpec::value` / `Column::value` (with `value_text` and `value_of`) reads
  a `CellValue` from a row. The column sorts by it and, without a `.cell()` renderer, shows it.
- Formatting per column: `CellFormat` (`number`, `currency`, `percent`, `Date`, `DateTime`,
  `date_pattern`), `CellAlign` and `CellOverflow` (`Truncate`, `TruncateWithTooltip`, `Wrap`).
  Header and data cells carry `data-align` and `data-overflow`; numeric formats align at the end.
- `GridLocale` with every text the grid writes and its number and date formats; English by
  default, `GridLocale::german()` included. `GridOptions::locale`, `GridHandle::locale` and
  `set_locale`; the `data_grid` component takes a `locale` prop.
- Optional `chrono` feature on both crates: `CellValue::Date` and `CellValue::DateTime`.
- The playground switches between English and German and shows a currency and a date column.

### Changed

- `ColumnSpec::sort_key` is now `ColumnSpec::value`, with a separate `sortable` flag.
  `SortValue` is an alias of `CellValue`; `sort_by`, `sort_by_text` and `sort_by_value` remain as
  shortcuts and sort exactly as before.
- `Column::render_cell` takes the locale.
- Text props are `Option<String>` and default to the locale: `GridSearch::placeholder`,
  `GridStatus::loading_label` and `retry_label`, and the component's `search_placeholder`,
  `empty_message` and `column_picker_label`. Passing a plain string still works.
- The row count reads "1 row" for a single row.

### Fixed

- A local grid drops selected rows that are removed from the data, instead of keeping them
  selected and counted.


## [0.4.0] - 2026-09-17

### Added

- Server-side data: `use_grid_remote` takes a `DataSource` and returns the same `GridHandle` as
  `use_grid`, so every primitive works with it. Typing in search or filters is debounced
  (`GridOptions::debounce`, 300ms by default); sorting and paging load at once. A slower response
  to an older request never overwrites a newer one.
- `GridHandle::is_loading`, `load_error` and `reload`; `GridRoot` sets `aria-busy` while loading.
- `GridStatus`, a live region showing the loading state or the error with a retry button.
- `datagrid-core`: `GridQuery`, `Page`, the `DataSource` trait, `RequestTracker` and
  `DEFAULT_REMOTE_PAGE_SIZE`.
- `examples/server`: a simulated server with adjustable latency and a request log.
- `examples/fullstack`: a Dioxus server function answering grid queries from SQLite, with the
  translation of `GridQuery` into SQL and a column allow-list.

### Changed

- `GridOptions` has a new public field, `debounce`.
- `dioxus-datagrid` depends on `tokio` (`time` only) outside WASM and on `gloo-timers` on WASM,
  for the debounce. Both are already in the tree of the renderer for that platform.

## [0.3.0] - 2026-09-17

### Added

- Column resizing: `ColumnResizeHandle`, enabled with `GridHeader { resizable: true }`. Drag a
  header's edge, press it twice to reset, or use `Alt+ArrowLeft` / `Alt+ArrowRight` on a focused
  header. The drag keeps following the pointer outside the grid.
- `datagrid-core`: `ColumnSpec::resizable`, `resize_min_width`, `clamp_width` and
  `effective_width`; `DEFAULT_MIN_COLUMN_WIDTH`; `GridState::reset_column_width`.
- `dioxus-datagrid`: `GridHandle::column_width`, `set_column_width`, `resize_column_by`,
  `is_column_visible` and the resize gesture methods; `COLUMN_RESIZE_STEP`.
- `data_grid` component: `resizable_columns` (on by default), `column_picker`,
  `column_picker_label`, `initial_state` and `on_state_change`.
- A property test that any grid state survives a serde round trip with an identical view.
- `data_grid` component: an `overscan` prop, defaulting to 20 rows. On the Android emulator the
  primitive's default of 5 showed blank rows during a fast fling; see ADR-0017.

### Changed

- `GridHandle::set_column_hidden` refuses to hide the last visible column.
- `GridHandle::focus` is clamped to the cells that exist, so filtering rows or hiding columns
  never leaves the grid without a tab stop.
- `GridState::set_column_width` ignores non-finite widths.
- `ColumnSpec` has a new public field, `resizable`.

### Fixed

- `data_grid` component: the header row stays in view while a height-limited grid scrolls. The
  sticky position sat on the header cells, which had no room to stick within their row.

## [0.2.0] - 2026-09-16

### Added

- Virtualization: `VirtualGridBody` renders only the rows in view plus overscan, with fixed row
  heights, for data sets in the hundreds of thousands. `aria-rowindex` stays tied to the data.
- Keyboard navigation in a virtualized grid scrolls the focused row into view by the minimum amount,
  and `PageUp` / `PageDown` move by one viewport.
- The grid root parks keyboard focus when the focused row scrolls out of the DOM, and hands it back
  to that row when the user tabs in again.
- `datagrid-core`: `reveal_scroll_top` and `rows_per_viewport`.
- `dioxus-datagrid`: `GridHandle` layout tracking (`Layout`, `rendered_range`, `reveal_focus`).
- `data_grid` component: `row_height` and `height` props.

### Changed

- `GridRoot` is now the scroll container, reporting scroll position and size through `onscroll` and
  `onresize`. In the `data_grid` component the grid element scrolls instead of a wrapper.
- `GridRoot` carries `tabindex="-1"` so it can hold focus programmatically; it becomes the tab stop
  only while the focused cell is not rendered.

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

[Unreleased]: https://github.com/dioxus-datagrid/dioxus-datagrid/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/dioxus-datagrid/dioxus-datagrid/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/dioxus-datagrid/dioxus-datagrid/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/dioxus-datagrid/dioxus-datagrid/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/dioxus-datagrid/dioxus-datagrid/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/dioxus-datagrid/dioxus-datagrid/releases/tag/v0.1.0
