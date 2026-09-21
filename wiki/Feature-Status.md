# Feature status

What the grid does today, area by area, and what is not there yet. The state is `main` after
Phase 10 (grouping and aggregates); everything except that phase is in the released **0.7.0**.

The comparison is against the Syncfusion Blazor DataGrid, which is the yardstick the
[roadmap](Roadmap.md) uses. "Planned" names the phase that will close the gap;
"not a goal" means we decided against it, with the reason in the roadmap.

## Data

| | Today | Gap |
|---|---|---|
| Local rows | A `Signal<Vec<T>>`. Rows stay where they are — the grid works on indices into your slice, so nothing is cloned. | — |
| Server-side | `DataSource` trait: every sort, filter, search, page or grouping change becomes one `GridQuery`, answered with one `Page`. Typing is debounced; a slow answer to an old query never overwrites a newer one. | Adapters for REST, OData or GraphQL. The trait is deliberately the only contract — `examples/fullstack` shows the SQL translation. |
| Live updates | Changing the signal re-renders; selection follows the rows that are still there. | Insert, change and remove without a reload, with focus and selection held stable — **Phase 15**. |

## Reading the data

| | Today | Gap |
|---|---|---|
| Sorting | Single and multi-column (`Shift+Enter` adds one), stable, collation-aware for text, typed for everything else. | A fully custom comparator per column; today you sort by a key, not by a comparison. |
| Filter bar | One text box per column, with operator shorthands: `>100`, `!=Berlin`, `10..20`. | — |
| Filter menu | Per column: two conditions joined by *and* or *or*, operators per kind of value (contains, starts with, equals, between, empty, one of, …), or a list of the distinct values with counts to tick. Remote grids ask the server through `distinct_values`. | — |
| Search | One box across all visible columns. | — |
| Paging | Page size, page navigation, `aria-rowcount` across pages. Group headers and footers count as rows, so groups run across page boundaries. | — |
| Grouping | By one or more columns, outermost first. Expand and collapse one at a time or all at once. Works with paging, with virtualization, and on a server. | Grouping from a column menu — **Phase 11**, when the column menu arrives. The group panel's list is the mouse-free route until then. |
| Aggregates | Sum, average, min, max, count and your own function, under each group, in a collapsed group's header, and under the whole grid. | — |

## Showing the data

| | Today | Gap |
|---|---|---|
| Cells | Your own markup per column, or the value formatted by the grid. | — |
| Formatting | Number (decimals, thousands), currency, percent, date and date-time (`chrono` feature), alignment, wrap or truncate with a tooltip. | — |
| Columns | Resize by dragging or with `Alt+Arrow`, show and hide through `column_picker`, width and visibility kept in the state. | Order, pinned columns left and right, multi-level headers, a column menu, width from content, column spanning — **Phase 11**. |
| Rows | Fixed row height, virtualized. | Detail rows, row drag and drop, row spanning — **Phase 12**. Variable heights — **Phase 15**. |
| Virtualization | Rows of a fixed height: only what is in view is in the DOM, while the scrollbar, the keyboard and `aria-rowcount` cover all of them. 100,000 rows stay fluid, grouped as well. | Column virtualization, loading blocks while scrolling instead of paging, infinite scroll — **Phase 15**. |
| Localization | Every text the grid writes comes from a `GridLocale`: English by default, German included. Number, currency and date formats per locale. | More shipped languages; one is a table of strings away. |
| Responsive | Touch targets, virtualization verified on an Android emulator. | Rows as cards on narrow screens, filters and editing as a sheet — **Phase 14**. |

## Changing the data

| | Today | Gap |
|---|---|---|
| Editing | One cell, a whole row inline, a row in a form dialog, or cells collected into a batch. `Enter`/`F2` starts, `Escape` cancels, `Enter`/`Tab` commits and moves on. | — |
| Validation | Per column by the value's kind, per row after a column changed; errors at the cell with `aria-invalid` and `aria-describedby`. | — |
| Add and delete | With confirmation, and the selection cleaned up afterwards. | — |
| Who writes | Never the grid. It hands the edited row to your callback — which may be `async` — shows it while the save runs, and takes it back with the message if the save fails. | — |
| Selection | Rows: single, multi, range with `Shift`. Survives sorting, filtering and paging, because it is keyed. | Cell and rectangular selection, a checkbox column with a three-state "select all" — **Phase 12**. |
| Clipboard | — | Copy as TSV that Excel understands, paste into editable cells — **Phase 12**. |

## Around the grid

| | Today | Gap |
|---|---|---|
| State | The whole grid state — sort, filters, search, page, widths, hidden columns, grouping, collapsed groups — round trips through `initial_state` and `on_state_change` with the `serde` feature. | — |
| Accessibility | A single tab stop with a roving tabindex, the full WAI-ARIA grid pattern, and a treegrid when grouped. axe finds nothing, in light and dark. | RTL — **Phase 14**. A screen-reader field test with NVDA, JAWS and VoiceOver — **before 1.0**. |
| Toolbar, context menu | — | **Phase 14**. |
| Export, print | — | CSV, Excel and PDF in a separate `datagrid-export` crate, plus a print stylesheet — **Phase 13**. |

## Deliberately not a goal

AI features, a theme studio with its own themes, ORM integrations, and pivot / tree grid /
spreadsheet-with-formulas. The reasons are in section 3 of the
[roadmap](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/docs/ROADMAP.md).
