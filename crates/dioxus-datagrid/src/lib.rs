//! Headless data grid hooks and unstyled primitives for Dioxus 0.7.
//!
//! This crate wires [`datagrid-core`](https://docs.rs/datagrid-core) into Dioxus:
//! a `use_grid` hook owns the grid state, and a set of unstyled primitives render
//! the ARIA grid pattern. Styling is deliberately left to the caller — the
//! styled `data_grid` component is distributed through the component registry.
//!
//! The crate is platform neutral: it contains no `web-sys` dependency, so the
//! same code runs on web, desktop and mobile WebView renderers.

#![forbid(unsafe_code)]
