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
//! # Using it
//!
//! Describe the rows, describe the columns, hand both to [`use_grid`], then
//! compose the primitives.
//!
//! ```
//! use datagrid_core::GridRow;
//! use dioxus::prelude::*;
//! use dioxus_datagrid::primitives::{GridBody, GridHeader, GridPagination, GridRoot};
//! use dioxus_datagrid::{Column, GridOptions, use_grid};
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

mod column;
mod grid;
pub mod primitives;

pub use column::{CellRenderer, Column, HeaderRenderer};
pub use grid::{GridHandle, GridOptions, IntoReadSignal, Layout, use_grid};

// Re-exported so callers need only one dependency for the common path. The
// `GridRow` trait deliberately keeps its name here; the row *component* lives in
// [`primitives`] to avoid the collision.
pub use datagrid_core::{
    CellFocus, ColumnId, ColumnWidth, GridRow, GridState, NavKey, SelectionMode, SortDirection,
    SortValue, TextCollation, View,
};
