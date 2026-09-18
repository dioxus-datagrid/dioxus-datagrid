//! Framework-agnostic data grid logic.
//!
//! This crate contains the pure, deterministic half of the data grid: sorting,
//! filtering, paging, selection, virtualization math and keyboard navigation.
//! It has no UI framework dependency and can be unit-tested and benchmarked on
//! its own.
//!
//! The Dioxus bindings live in the companion crate `dioxus-datagrid`.
//!
//! # The shape of it
//!
//! Rows stay where they are. [`compute_view`] takes a row slice, the column
//! specs and the current [`GridState`], and returns a [`View`] of **indices**
//! into the original slice. Nothing is cloned, so a view over a hundred thousand
//! expensive rows costs a vector of `usize`.
//!
//! ```
//! use datagrid_core::{compute_view, ColumnSpec, GridState, GridRow};
//!
//! #[derive(Clone)]
//! struct User {
//!     id: u32,
//!     name: String,
//!     age: u32,
//! }
//!
//! impl GridRow for User {
//!     type Key = u32;
//!     fn key(&self) -> u32 {
//!         self.id
//!     }
//! }
//!
//! let rows = vec![
//!     User { id: 1, name: "Zoe".into(), age: 30 },
//!     User { id: 2, name: "adam".into(), age: 25 },
//!     User { id: 3, name: "Mia".into(), age: 30 },
//! ];
//!
//! let columns = vec![
//!     ColumnSpec::new("name")
//!         .sort_by_text(|user: &User| user.name.as_str())
//!         .filter_by(|user: &User| user.name.clone()),
//!     ColumnSpec::new("age").sort_by_value(|user: &User| user.age),
//! ];
//!
//! let mut state = GridState::new();
//! state.toggle_sort("name", false);
//!
//! let view = compute_view(&rows, &columns, &state);
//!
//! // Sorting is case-insensitive by default, so "adam" comes first.
//! let names: Vec<&str> = view
//!     .indices
//!     .iter()
//!     .filter_map(|&i| rows.get(i))
//!     .map(|user| user.name.as_str())
//!     .collect();
//! assert_eq!(names, ["adam", "Mia", "Zoe"]);
//! ```

#![forbid(unsafe_code)]

mod column;
mod filter;
mod format;
mod locale;
mod navigate;
mod remote;
mod selection;
mod sort;
mod state;
mod value;
mod view;
mod virtualize;

pub use column::{
    ColumnId, ColumnSpec, ColumnWidth, DEFAULT_MIN_COLUMN_WIDTH, FilterTextFn, SortKeyFn, ValueFn,
};
pub use filter::{ColumnFilter, Condition, FilterOp, FilterValue, ValueKind};
pub use format::{CellAlign, CellFormat, CellOverflow};
pub use locale::GridLocale;
pub use navigate::{CellFocus, NavKey, navigate};
pub use remote::{
    DEFAULT_REMOTE_PAGE_SIZE, DataSource, GridQuery, Page, RequestId, RequestTracker,
};
pub use selection::{Selection, SelectionMode};
pub use sort::{SortDirection, SortState, SortValue, TextCollation};
pub use state::{GridState, PageState};
pub use value::CellValue;
pub use view::{DistinctValues, View, compute_view, distinct_values};
pub use virtualize::{
    offset_of, reveal_scroll_top, rows_per_viewport, total_height, visible_range,
};

/// A row the grid can identify across sorting, filtering and paging.
///
/// Identity matters because a row's *position* is not stable — it changes with
/// every sort and filter — while selection, focus restoration and diffing all
/// need to refer to the same row before and after.
///
/// ```
/// use datagrid_core::GridRow;
///
/// #[derive(Clone)]
/// struct Invoice {
///     number: String,
///     total: f64,
/// }
///
/// impl GridRow for Invoice {
///     type Key = String;
///     fn key(&self) -> String {
///         self.number.clone()
///     }
/// }
/// ```
pub trait GridRow: Clone + 'static {
    /// The stable identity of a row.
    type Key: Clone + Eq + std::hash::Hash + 'static;

    /// Returns this row's key.
    ///
    /// Must be stable for a given row and unique within the data set. Duplicate
    /// keys make selection ambiguous.
    fn key(&self) -> Self::Key;
}
