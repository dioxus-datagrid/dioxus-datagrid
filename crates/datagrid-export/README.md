# datagrid-export

Writes what a [`dioxus-datagrid`](https://crates.io/crates/dioxus-datagrid) grid shows as a file.

The table comes from `datagrid_core::export_table`: the visible columns in the order the grid lays
them out, the rows as sorted, filtered and grouped, every cell as a typed value and as the text the
grid shows for it. This crate only writes it, one format per feature, so that a format with heavy
dependencies costs nothing to anyone who does not ask for it.

| Feature | Format | Dependencies |
|---|---|---|
| `csv` (default) | CSV: RFC 4180, a semicolon or any delimiter, a byte order mark for Excel, text that would run as a formula defused | none |
| `html` (default) | an HTML table to print or to put in a page, every cell escaped, the header repeated on every printed sheet | none |
| `xlsx` | an Excel workbook: numbers, booleans and dates as such, a column's number format, the header frozen, groups as an outline, aggregates | `rust_xlsxwriter` (`zip`, `flate2` and a few small ones) |
| `chrono` | turns on `datagrid-core`'s `chrono`, so that dates are values; the writers read them with or without this feature | `chrono`, through `datagrid-core` |

PDF is not a format here: the browser makes one from the printed HTML table.

```rust
use datagrid_core::{CellValue, ColumnSpec, ExportScope, GridLocale, GridState, export_table};
use datagrid_export::{CsvOptions, to_csv};

struct Person { name: &'static str, age: i64 }

let rows = [Person { name: "Zoe", age: 31 }];
let columns = [
    ColumnSpec::new("name").value(|row: &Person| CellValue::Text(row.name)),
    ColumnSpec::new("age").value(|row: &Person| CellValue::Int(row.age)),
];
let table = export_table(
    &rows, &columns, |id| id.as_str().to_owned(),
    &GridState::new(), &GridLocale::english(), ExportScope::Filtered,
);
assert_eq!(to_csv(&table, &CsvOptions::default()), "name,age\r\nZoe,31\r\n");
```

With the `xlsx` feature, `to_xlsx(&table, &XlsxOptions::default())` writes the same table as the bytes of an
`.xlsx` file. In a browser, `rust_xlsxwriter` is built with its `wasm` feature automatically.

## License

MIT OR Apache-2.0.
