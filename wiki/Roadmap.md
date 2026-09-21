# Roadmap

The goal: someone using a Syncfusion Blazor DataGrid in a business application should find the
features they actually use here too — typed, accessible, on web, desktop and mobile. Not a copy of
its API, and not every corner of it.

Phases run strictly in order. Each one ends in a release, so the project is usable at any point and
can be stopped at any version. **S** is roughly one working session, **M** two or three, **L** four
or more.

This page is a summary. The binding documents are
[`PLAN.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/PLAN.md) (phases 0–6) and
[`docs/ROADMAP.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/docs/ROADMAP.md)
(phases 7–15), both German, both with the acceptance criteria that decide when a phase is finished.

## Done

| | | |
|---|---|---|
| Phases 0–6 | 0.1.0 – 0.4.0 | The grid itself: sorting, filtering, search, paging, selection, virtualization, column widths, the column picker, the ARIA grid pattern, and server-side data through `DataSource` |
| Phase 7 | 0.5.0 | **Foundation** — one typed value per column, formats (number, currency, percent, date), and `GridLocale` with English and German |
| Phase 8 | 0.6.0 | **Filtering** — operators per kind of value, the filter menu, Excel-style value lists, typed filters in `GridQuery` |
| Phase 9 | 0.7.0 | **Editing** — cell, row, dialog and batch; validation; add and delete; the grid never writes your data |
| Phase 10 | 0.8.0 *(on `main`)* | **Grouping and aggregates** — multi-level grouping, a group panel, sum/average/min/max/count, the treegrid, grouping on the server |

## Ahead

| | | |
|---|---|---|
| Phase 11 | 0.9.0 (M) | **Column layout** — order by drag and by keyboard, pinned columns, multi-level headers, a column menu, width from content, column spanning |
| Phase 12 | 0.10.0 (M) | **Rows, selection, clipboard** — detail rows, row reordering, a checkbox column, cell and range selection, copy as TSV and paste |
| Phase 13 | 0.11.0 (M) | **Export and print** — a separate `datagrid-export` crate: CSV, Excel with formatting and groups, PDF; a print stylesheet |
| Phase 14 | 0.12.0 (M) | **The UI around it** — toolbar, context menu, an adaptive layout that turns rows into cards on narrow screens, RTL |
| Phase 15 | 0.13.0 (L) | **Large and live data** — loading blocks while scrolling instead of paging, infinite scroll, column virtualization, variable row heights, live updates |
| | 1.0 | An API review, deprecations removed, a screen-reader field test with NVDA, JAWS and VoiceOver, a documentation site with a demo per feature, benchmarks against the target numbers |

Roughly 20–30 sessions to 1.0, at the pace the phases so far have taken.

## The decisions that shaped it

Four of them were taken before Phase 7, because reworking them later would have cost more than they
were worth:

- **One typed value per column** instead of a closure per concern — see [Architecture](Architecture.md).
- **The view as a list of row kinds**, which grouping, detail rows and aggregate rows all need. The
  largest single rewrite in the plan, done as its own step before the first group existed.
- **The registry component becomes a family**: `data_grid` stays the entry point and is extended by
  add-ons through a context, rather than growing.
- **Heavy dependencies into their own optional crates** — Excel and PDF export pull in large
  libraries and will live in `datagrid-export`, never in the core.

## Known risks

- **Clipboard and file download have no platform-neutral API in Dioxus 0.7.** The way out is
  `document::eval`, which the architecture rules otherwise exclude; it is allowed for those two
  cases only, proven by a spike and argued in a decision record each time.
- **Dioxus 0.8 is coming** (an alpha exists). Switching mid-plan probably costs a phase; the state
  of 0.8 is tracked in
  [`docs/VERIFICATION.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/docs/VERIFICATION.md).
- **Pinned columns and column virtualization** both have to survive next to virtualization and
  resizing; each starts with a spike.

## Not a goal

AI features — semantic search, anomaly detection — belong in the application; the `DataSource` trait
is enough for them. No second theme system: styling goes through the dx-components theme variables.
No ORM binding; `examples/fullstack` shows the pattern and anyone can wrap their own. No pivot grid,
tree grid or spreadsheet with formulas — those are separate products, and they are out of scope here
too.
