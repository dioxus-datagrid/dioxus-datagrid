//! Server-rendered checks on cells that cover several columns: that the covered
//! columns draw nothing, that the wide cell says how far it reaches, and that
//! `aria-colindex` keeps counting straight past it.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{GridRow, Pinned};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{GridBody, GridHeader, GridRoot};
use dioxus_datagrid::{Column, GridOptions, use_grid};

#[derive(Clone, PartialEq)]
struct Entry {
    id: u32,
    label: String,
    a: u32,
    b: u32,
    c: u32,
    /// What the label column covers in this row.
    span: usize,
}

impl GridRow for Entry {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

fn entry(id: u32, label: &str, span: usize) -> Entry {
    Entry {
        id,
        label: label.to_owned(),
        a: 1,
        b: 2,
        c: 3,
        span,
    }
}

fn entries() -> Vec<Entry> {
    vec![
        entry(1, "Plain", 1),
        entry(2, "Wide", 3),
        entry(3, "Wider than the row", 9),
    ]
}

fn columns(pins: &[(&str, Pinned)]) -> Vec<Column<Entry>> {
    let pin_of = |id: &str| {
        pins.iter()
            .find(|(name, _)| *name == id)
            .map_or(Pinned::None, |(_, pin)| *pin)
    };

    vec![
        Column::new("label", "Label")
            .value_text(|entry: &Entry| entry.label.as_str())
            .span(|entry: &Entry| entry.span)
            .pin(pin_of("label")),
        Column::new("a", "A")
            .value_of(|entry: &Entry| entry.a)
            .pin(pin_of("a")),
        Column::new("b", "B")
            .value_of(|entry: &Entry| entry.b)
            .pin(pin_of("b")),
        Column::new("c", "C")
            .value_of(|entry: &Entry| entry.c)
            .pin(pin_of("c")),
    ]
}

#[derive(Clone, PartialEq, Props)]
struct Setup {
    #[props(default)]
    pins: Vec<(String, Pinned)>,
}

#[component]
fn Grid(setup: Setup) -> Element {
    let rows = use_signal(entries);
    let pins: Vec<(String, Pinned)> = setup.pins.clone();
    let cols = use_hook(move || {
        let pins: Vec<(&str, Pinned)> = pins.iter().map(|(id, pin)| (id.as_str(), *pin)).collect();
        columns(&pins)
    });
    let grid = use_grid(rows, cols, GridOptions::default());

    rsx! {
        GridRoot { grid,
            GridHeader { grid }
            GridBody { grid }
        }
    }
}

fn render(setup: Setup) -> String {
    #[component]
    fn Harness(setup: Setup) -> Element {
        rsx! { Grid { setup } }
    }

    let mut dom = VirtualDom::new_with_props(Harness, HarnessProps { setup });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

fn pins(pairs: &[(&str, Pinned)]) -> Vec<(String, Pinned)> {
    pairs
        .iter()
        .map(|(id, pin)| ((*id).to_owned(), *pin))
        .collect()
}

/// The data rows, each as the markup of its cells.
///
/// Everything after the header, split at the rows; a data row is the one that
/// carries `data-selected`.
fn data_rows(html: &str) -> Vec<&str> {
    html.split(r#"role="row""#)
        .filter(|row| row.contains("data-selected"))
        .collect()
}

/// How many cells each data row renders.
fn cell_counts(html: &str) -> Vec<usize> {
    data_rows(html)
        .iter()
        .map(|row| row.matches(r#"role="gridcell""#).count())
        .collect()
}

/// The `aria-colindex` of each cell of one data row.
fn col_indexes(row: &str) -> Vec<&str> {
    row.split(r#"aria-colindex=""#)
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .collect()
}

#[test]
fn a_row_that_spans_nothing_renders_a_cell_per_column() {
    let html = render(Setup::builder().build());

    assert_eq!(cell_counts(&html)[0], 4, "{html}");
    assert!(!data_rows(&html)[0].contains("aria-colspan"), "{html}");
    assert!(!data_rows(&html)[0].contains("grid-column"), "{html}");
}

#[test]
fn a_wide_cell_replaces_the_columns_it_covers() {
    let html = render(Setup::builder().build());
    let row = data_rows(&html)[1];

    // The wide cell plus the one column it leaves over.
    assert_eq!(cell_counts(&html)[1], 2, "{html}");
    assert!(row.contains(r#"aria-colspan="3""#), "{html}");
    // A row is a subgrid, so the layout has to be told as well.
    assert!(row.contains("grid-column: span 3;"), "{html}");
}

#[test]
fn the_cell_after_a_wide_one_keeps_its_column_index() {
    let html = render(Setup::builder().build());

    // Not 1 and 2: the second cell is the fourth column.
    assert_eq!(col_indexes(data_rows(&html)[1]), ["1", "4"]);
}

#[test]
fn a_span_stops_at_the_last_column() {
    let html = render(Setup::builder().build());
    let row = data_rows(&html)[2];

    assert_eq!(cell_counts(&html)[2], 1, "{html}");
    assert!(row.contains(r#"aria-colspan="4""#), "{html}");
}

#[test]
fn a_span_never_leaves_its_pinned_block() {
    // Two columns held at the start; the label would cover three, which would
    // make one cell both held in place and scrolling.
    let html = render(
        Setup::builder()
            .pins(pins(&[("label", Pinned::Start), ("a", Pinned::Start)]))
            .build(),
    );
    let row = data_rows(&html)[1];

    assert!(row.contains(r#"aria-colspan="2""#), "{html}");
    // Clamped to the block, so three cells are left: the wide one and B and C.
    assert_eq!(cell_counts(&html)[1], 3, "{html}");
}

#[test]
fn a_wide_cell_is_still_held_at_its_edge() {
    let html = render(
        Setup::builder()
            .pins(pins(&[("label", Pinned::Start), ("a", Pinned::Start)]))
            .build(),
    );
    let row = data_rows(&html)[1];

    assert!(row.contains(r#"data-pinned="start""#), "{html}");
    assert!(
        row.contains("grid-column: span 2; --dg-pin-offset: 0px;"),
        "{html}"
    );
}

#[test]
fn the_header_is_untouched_by_a_row_that_spans() {
    let html = render(Setup::builder().build());
    let header = html.split(r#"role="row""#).next().unwrap_or_default();

    assert!(!header.contains("aria-colspan"), "{html}");
    assert_eq!(html.matches(r#"role="columnheader""#).count(), 4, "{html}");
}
