//! Writes what a grid shows as a file.
//!
//! The table comes from [`datagrid_core::export_table`], which takes the
//! visible columns in the order the grid lays them out and the rows as sorted,
//! filtered and grouped. This crate only writes it: each format is a feature, so
//! that the ones with heavy dependencies cost nothing to anyone who does not
//! ask for them. CSV needs no dependency at all.
//!
//! ```
//! use datagrid_core::{
//!     CellValue, ColumnSpec, ExportScope, GridLocale, GridState, export_table,
//! };
//! use datagrid_export::{CsvOptions, to_csv};
//!
//! struct Person {
//!     name: &'static str,
//!     age: i64,
//! }
//!
//! let rows = [Person { name: "Zoe", age: 31 }, Person { name: "Adi", age: 27 }];
//! let columns = [
//!     ColumnSpec::new("name").value(|row: &Person| CellValue::Text(row.name)),
//!     ColumnSpec::new("age").value(|row: &Person| CellValue::Int(row.age)),
//! ];
//! let table = export_table(
//!     &rows,
//!     &columns,
//!     |id| id.as_str().to_owned(),
//!     &GridState::new(),
//!     &GridLocale::english(),
//!     ExportScope::Filtered,
//! );
//!
//! assert_eq!(to_csv(&table, &CsvOptions::default()), "name,age\r\nZoe,31\r\nAdi,27\r\n");
//! ```

#![forbid(unsafe_code)]

#[cfg(feature = "csv")]
mod csv;

#[cfg(feature = "csv")]
pub use csv::{CsvOptions, CsvValues, LineEnding, to_csv};
