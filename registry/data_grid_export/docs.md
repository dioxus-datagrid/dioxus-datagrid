# data_grid_export

Export for [`data_grid`](../data_grid/docs.md): buttons that download the rows as a CSV file or an Excel
workbook, and print them.

## Usage

```rust
use crate::components::data_grid::DataGrid;
use crate::components::data_grid_export::DataGridExport;

rsx! {
    DataGrid { data: people, columns,
        DataGridExport { file_name: "people" }
    }
}
```

The file holds the columns the grid shows, in the order it shows them, and the rows as they are
sorted, filtered and grouped. Hidden columns and the checkbox, expander and drag-handle columns are
left out; a column that draws a cell of its own markup but has no `value` writes empty cells.

## Props

| Prop | Default | |
|---|---|---|
| `file_name` | `"export"` | The name of the file, without `.csv` or `.xlsx`. |
| `scope` | `ExportScope::Filtered` | `Filtered` is every row that passes the filters, on every page, with every group open. `View` is the page on screen, groups as shown. |
| `max_rows` | `100_000` | The most rows to take from a server. |
| `csv` | `CsvOptions::default()` | How the CSV is written, see below. |
| `excel` | `true` | Shows the button that downloads an Excel workbook. |
| `xlsx` | `XlsxOptions::default()` | How the workbook is written: its sheet name, a filter on the header, column widths, the outline. |
| `print` | `true` | Shows the button that prints the rows. |

## Excel

The workbook holds what the CSV holds, as cells that Excel can work with: numbers, booleans and dates as
such, each column in its number format (`#,##0.00`, a percent, a currency symbol where the grid put it, a date
pattern), the header in bold and kept in view, and, where the grid is grouped, each group as an outline that
folds, with its aggregates and the totals. Text is text: nothing in a cell is ever run as a formula.

It needs the `xlsx` feature of `datagrid-export`, which this component's `component.json` turns on. To go
without it, delete the button, the `export_xlsx` handler and the `xlsx` feature from your `Cargo.toml`.
Excel is the one format here that brings a library of its own (`rust_xlsxwriter` and `zip`).

## A grid that loads its rows from a server

`Filtered` fetches every row that passes the filters from the server, a thousand at a time, with the
sort, the filters and the search the grid shows, and writes them as one file. The button says
"Exporting …" meanwhile and cannot be pressed twice.

- **A limit.** If more than `max_rows` rows match, nothing is loaded past the first page and the
  export says so ("2,500 rows are too many to export (at most 2,000). Narrow the filter.").
- **A failure is no file.** If any page fails, there is no download, and the message says why.
- **Flat.** The rows come in the order of the sort, without the grid's groups; the totals are worked
  out over all of them.
- `View` is the page on screen, as the server sent it.

`GridHandle::export_all(max_rows)` is the same thing for your own button.

## What the CSV holds

`CsvOptions` decides, and the defaults are for a file that a program reads:

- **Values, not text.** `4200.5`, `true`, `2026-10-16`, with a decimal point in every language. Set
  `values: CsvValues::Shown` for what the screen says (`€4,200.50`).
- **Formulas defused.** Text that starts with `=`, `+`, `-` or `@` is a formula to a spreadsheet, and
  a grid shows what its users typed. It gets a `'` in front. Numbers are left alone: `-5` stays `-5`.
- **No byte order mark.** Excel reads a file without one in the system code page and mangles every
  umlaut. For a file that Excel will open, set `bom: true`, and `delimiter: ';'` where it writes a
  decimal comma.
- Groups are lines of their own: the heading, the rows, then the aggregates. The totals line starts
  with "Totals" (or the locale's word for it).

## Printing

The Print button prints a table, not the grid. A paged or virtualized grid has only some of its rows
in the page, so what is printed is made from the same export as the CSV: the same columns, the same
rows for `scope`, the groups and the aggregates, in a real `<table>` whose header the printer repeats
on every sheet. The rest of the page is hidden while it prints.

Whatever a cell says is escaped. `datagrid_export::to_html` writes the same table for a page of your
own, and `dioxus_datagrid::print_html(html)` prints any markup you trust the way the button does.

## Saving without `web-sys`

The download is a `Blob` and a link clicked through `document::eval`, the way the clipboard is
reached: Dioxus has no API for either. `dioxus_datagrid::save_file(name, mime, text)` is the same
thing for a file of your own.
