//! Framework-agnostic data grid logic.
//!
//! This crate contains the pure, deterministic half of the data grid: sorting,
//! filtering, paging, selection, virtualization math and keyboard navigation.
//! It has no UI framework dependency and can be unit-tested and benchmarked on
//! its own.
//!
//! The Dioxus bindings live in the companion crate `dioxus-datagrid`.

#![forbid(unsafe_code)]
