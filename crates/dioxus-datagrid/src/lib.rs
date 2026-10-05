//! Headless data grid hooks and unstyled primitives for Dioxus 0.7.
//!
//! This crate wires [`datagrid-core`](https://docs.rs/datagrid-core) into Dioxus:
//! a [`use_grid`] hook owns the grid state, and a set of unstyled primitives
//! render the ARIA grid pattern. Styling is deliberately left to the caller —
//! the styled `data_grid` component is distributed through the component
//! registry.
//!
//! The crate is platform neutral: it contains no `web-sys` dependency, so the
//! same code runs on web, desktop and mobile WebView renderers.
//!
//! It is also the only dependency an application needs: everything
//! `datagrid-core` makes public is re-exported here, and the crate itself as
//! [`datagrid_core`] for anything this one has not caught up with.
//!
//! # Using it
//!
//! Describe the rows, describe the columns, hand both to [`use_grid`], then
//! compose the primitives.
//!
//! ```
//! use dioxus::prelude::*;
//! use dioxus_datagrid::primitives::{GridBody, GridHeader, GridPagination, GridRoot};
//! use dioxus_datagrid::{Column, GridOptions, GridRow, use_grid};
//!
//! #[derive(Clone, PartialEq)]
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
//! #[component]
//! fn Users() -> Element {
//!     let users = use_signal(Vec::<User>::new);
//!     let columns = use_hook(|| {
//!         vec![
//!             Column::new("name", "Name")
//!                 .cell(|user: &User| rsx! { "{user.name}" })
//!                 .sort_by_text(|user: &User| user.name.as_str())
//!                 .filter_by(|user: &User| user.name.clone()),
//!             Column::new("age", "Age")
//!                 .cell(|user: &User| rsx! { "{user.age}" })
//!                 .sort_by_value(|user: &User| user.age),
//!         ]
//!     });
//!
//!     let grid = use_grid(users, columns, GridOptions::paged(25));
//!
//!     rsx! {
//!         GridRoot { grid,
//!             GridHeader { grid }
//!             GridBody { grid }
//!         }
//!         GridPagination { grid }
//!     }
//! }
//! ```
//!
//! # Accessibility
//!
//! The primitives implement the WAI-ARIA data grid pattern: roles, `aria-sort`,
//! `aria-selected`, `aria-rowindex` and `aria-colindex` that stay correct across
//! paging, and a roving tabindex with the expected key bindings. The details are
//! in `docs/ACCESSIBILITY.md`.

#![forbid(unsafe_code)]

mod attrs;
mod column;
mod column_menu;
mod copy;
mod detail;
mod edit;
mod edit_ui;
mod filter_menu;
mod grid;
mod group;
mod group_ui;
mod paste;
pub mod primitives;
mod remote;
mod row_drag;
mod select_ui;
mod timer;

pub use attrs::merge_class;
pub use column::{CellRenderer, Column, EditorRenderer, HeaderRenderer};
pub use column_menu::{ColumnAction, ColumnMenuEntry};
pub use detail::DetailRows;
pub use edit::{
    Create, Delete, EditMode, EditMove, EditStatus, EditTarget, Editing, Save, SaveBatch,
};
pub use edit_ui::CellEditor;
pub use grid::{
    COLUMN_RESIZE_STEP, DEFAULT_DEBOUNCE, GridHandle, GridOptions, IntoReadSignal, Layout, use_grid,
};
pub use group::GroupKeyPress;
pub use paste::PasteReport;
pub use remote::use_grid_remote;
pub use row_drag::{DropSide, RowMove};

// Re-exported so that an application needs this one dependency and no second
// one: everything `datagrid-core` makes public is reachable from here, because
// what a type is called is no help if it cannot be named — `GridQuery::sort` is
// a `Vec<SortState>`, so writing a `DataSource` means naming `SortState`.
//
// Keep this in step with `datagrid_core`'s own exports when the core gains a
// type; `datagrid_core` itself is re-exported below for anything that is
// missing all the same.
//
// The `GridRow` trait deliberately keeps its name here; the row *component*
// lives in [`primitives`] to avoid the collision.
pub use datagrid_core::{
    Aggregate, AggregateFn, AggregateKind, AggregateValue, CellAlign, CellFocus, CellFormat,
    CellOverflow, CellRange, CellSelectionMode, CellValue, Changes, ColumnFilter, ColumnId,
    ColumnSpec, ColumnWidth, Condition, DEFAULT_MIN_COLUMN_WIDTH, DEFAULT_REMOTE_PAGE_SIZE,
    DataSource, DistinctValues, EditError, FilterOp, FilterTextFn, FromValue, GridLocale,
    GridQuery, GridRow, GridState, Group, GroupKey, GroupSpan, GroupSummary, GroupedPage, NavKey,
    Page, PagePart, PageState, Pinned, RequestId, RequestTracker, RowSpans, Selection,
    SelectionExtent, SelectionMode, SetFn, SortDirection, SortKeyFn, SortState, SortValue, SpanFn,
    TextCollation, ValidateFn, Value, ValueFn, ValueKind, View, ViewRow, changed_columns,
    compute_view, compute_view_with_details, distinct_values, find_aggregate, from_tsv,
    group_header_rows, group_levels, move_row, navigate, offset_of, reveal_scroll_top,
    rows_per_viewport, to_tsv, total_height, tsv_field, visible_range,
};

/// The core crate, re-exported whole: anything this crate does not name
/// directly is reachable as `dioxus_datagrid::datagrid_core::…`, so an
/// application never needs to depend on it separately to keep up with a
/// version of it.
pub use datagrid_core;
