# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crates follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Grouping. `GridState::group_by` groups rows by one or more columns, outermost first; groups follow
  their column's sort and expand and collapse one at a time (`toggle_group`) or all at once. The
  state is serializable like the rest. `GridHandle` has `group_by_column`, `ungroup_column`,
  `set_group_by`, `toggle_group`, `set_all_groups_expanded` and `group_at`.
- Aggregates. `.aggregate(Aggregate::Sum)` — or `Average`, `Min`, `Max`, `Count`, or
  `Aggregate::custom` over the rows — under every group and under the whole grid.
  `GridLocale::format_aggregate` formats results in the column's format.
- A grouped grid is a WAI-ARIA treegrid: group headers carry `aria-level`, `aria-expanded`,
  `aria-posinset` and `aria-setsize`, and `ArrowRight`, `ArrowLeft`, `Enter` and `Space` expand and
  collapse them. `GridRow` renders group headers (`GridGroupRow`) and footers
  (`GridGroupFooterRow`) where the view has them.
- `GridFooter` shows the totals, stays reachable by keyboard as the last row, and reports its
  height so a virtualized body keeps rows out from under it.
- `GridGroupPanel`: drag a column header onto it, or pick a column from its list, to group; move and
  remove grouped columns, expand or collapse everything.
- Grouping on the server. `GridQuery` carries `group_by`, the expanded groups and the aggregates
  wanted; `Page` can carry a layout of group rows, the groups, the row count and the totals.
  `GridQuery::state`, `Aggregate::apply_query` and `Page::from_view` answer from rows in memory;
  `GroupedPage::plan` lays out a page from group counts, for SQL. Older servers and clients still
  understand each other.
- The `data_grid_group_panel` registry component, and totals in `data_grid`.
- `examples/server` groups in memory; `examples/fullstack` groups in SQLite, one `GROUP BY` per
  level, fetching only the rows the page shows.

### Changed

- The view is a list of rows of kinds (`View::rows`, `ViewRow`), which everything that addresses
  rows by position walks; `View::indices` stays as the data rows. `View::row_count` and
  `View::row_offset` are what `aria-rowcount` and `aria-rowindex` count. With grouping, pages
  count group headers and footers as rows.

## [0.7.0] - 2026-09-19

### Added

- Editing. Columns become editable with a setter of the field's own type,
  `.editable(|row, age: u32| row.age = age)`; what a user types is read as the column's kind of
  value and converted with the new `FromValue` trait, and refused with a message if it does not
  fit. `.edit` takes the raw `Value`, `.validate` checks a row after a column changed, `.choices`
  offers a list, and `.editor` replaces the editor with your own (`CellEditor`).
- Four ways to edit, set with `GridHandle::set_editing` and `Editing`: one cell at a time
  (`EditMode::Cell`), a whole row inline (`Row`), a row in a form dialog (`Dialog`), and cells
  collected into a batch (`Batch`). `Enter` or `F2` starts, `Enter` and `Tab` commit and move on,
  `Escape` cancels, `Delete` deletes.
  `GridHandle::clear_editing` makes the grid read-only again; changing the mode drops an unsaved
  batch.
- The grid never writes the data. `on_save`, `on_create`, `on_delete` and `on_batch_save` receive a
  token with the rows (`Save`, `Create`, `Delete`, `SaveBatch`); dropping it means success, `fail`
  reports an error. The callbacks may be `async`. While a save runs the grid shows the edited row,
  and takes it back if the save fails.
- Primitives: editors in `GridCell` (`GridCellEditor`), `GridEditDialog` for forms and new rows,
  `GridDeleteConfirm`, `GridEditToolbar` and `GridEditStatus`. Cells carry `data-editing`,
  `data-changed` and, in an editable grid, `aria-readonly`; rows carry `data-editing`,
  `data-deleted` and `data-saving`.
- In the core: `ColumnSpec::edit`, `editable`, `validate`, `choices`, `apply_edit` and
  `edit_text`, `EditError`, `FromValue`, `Changes` for batches and `changed_columns`.
- `GridLocale` has the texts for editing, in English and German.
- The `data_grid_editor` registry component: `DataGridEditor` inside a `DataGrid` makes it
  editable, with a toolbar, status line, form dialog and delete confirmation.
- `data_grid` takes children and provides its `GridHandle` as context, for add-ons like the editor.
- `examples/fullstack` saves edits through a server function that has rules of its own.

### Changed

- `FilterValue` is now `Value`, since edits write it too. `FilterValue` remains as a deprecated
  alias.

### Fixed

- The filter menu's panel no longer sticks out of the screen on phones: when it would, it is
  moved back inside with an inline `translate`.

## [0.6.0] - 2026-09-18

### Added

- Typed column filters: `ColumnFilter` holds conditions (`Condition`, with a `FilterOp` and
  `FilterValue` operands) joined by and or or. Operators per kind of value: contains, starts
  with, ends with, equals, not equals, less, at most, greater, at least, between, empty, not
  empty, and one of. Kept per column in `GridState::filters`, next to the filter bar's text.
- The filter bar understands `>100`, `<=`, `!=`, `=` and `a..b`. Plain text stays a substring
  test on text columns and means "equals" on number, date and boolean columns.
  `ColumnFilter::from_bar_text` gives servers the same reading.
- `GridFilterMenu`, an unstyled filter menu per column: one or two conditions, or a searchable
  value list with counts. The `data_grid` component shows it with `filter_menu: true`.
- `distinct_values` and `DistinctValues` for value lists, among the rows that pass every other
  filter. `DataSource::distinct_values` answers them remotely; it has a default, so existing
  sources compile unchanged.
- `GridHandle::set_column_filter`, `column_filter`, `clear_column_filters`, `is_filtered`,
  `value_kind`, `distinct_values`, `column_template` and `pickable_columns`.
- `ValueKind`, and `ColumnSpec::kind` / `Column::kind` to declare it. Otherwise it is read from
  the rows.
- `GridLocale` has the filter menu's texts and operator names, in English and German.
- Filtered header cells carry `data-filtered`.
- `examples/fullstack` translates every operator, and/or and value lists into SQL, tested
  against the grid's own filtering.

### Changed

- A column with a value is filterable even without `filter_by`, so it gets a filter bar input.
  Search still looks only at `filter_by` text.
- `GridQuery` has a `filters` field; it is `serde(default)`, so queries from older clients still
  parse.

### Fixed

- `dioxus-datagrid` compiles when another crate enables `chrono` in `datagrid-core` only.


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

[Unreleased]: https://github.com/dioxus-datagrid/dioxus-datagrid/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/dioxus-datagrid/dioxus-datagrid/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/dioxus-datagrid/dioxus-datagrid/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/dioxus-datagrid/dioxus-datagrid/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/dioxus-datagrid/dioxus-datagrid/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/dioxus-datagrid/dioxus-datagrid/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/dioxus-datagrid/dioxus-datagrid/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/dioxus-datagrid/dioxus-datagrid/releases/tag/v0.1.0
