# data_grid_export

Export for [`data_grid`](../data_grid/docs.md): a button that downloads the rows as a
CSV file.

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
| `file_name` | `"export"` | The name of the file, without `.csv`. |
| `scope` | `ExportScope::Filtered` | `Filtered` is every row that passes the filters, on every page, with every group open. `View` is the page on screen, groups as shown. |
| `csv` | `CsvOptions::default()` | How the CSV is written, see below. |

A grid that loads its rows from a server only has the page on screen, so it exports that page, in the
order the server sent it, whatever `scope` says.

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

## Saving without `web-sys`

The download is a `Blob` and a link clicked through `document::eval`, the way the clipboard is
reached: Dioxus has no API for either. `dioxus_datagrid::save_file(name, mime, text)` is the same
thing for a file of your own.
