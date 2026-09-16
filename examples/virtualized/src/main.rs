//! 100,000 rows with only the visible ones in the DOM.
//!
//! Built directly on the primitives, so it shows everything a virtualized grid
//! needs: `GridRoot` as a fixed-height scroll container, a sticky header, and
//! `VirtualGridBody` in place of `GridBody`.
//!
//! Run it with `dx serve --package example-virtualized`.

use datagrid_core::{GridRow, SelectionMode};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{GridHeader, GridRoot, GridSearch, VirtualGridBody};
use dioxus_datagrid::{Column, ColumnWidth, GridOptions, use_grid};

const STYLE: Asset = asset!("/assets/virtualized.css");

/// Must match the row height in the stylesheet's padding and font size, since
/// every row is rendered at exactly this height.
const ROW_HEIGHT: f64 = 32.0;
const ROWS: u64 = 100_000;

fn main() {
    dioxus::launch(App);
}

#[derive(Clone, PartialEq)]
struct Measurement {
    id: u64,
    sensor: String,
    value: f64,
    ok: bool,
}

impl GridRow for Measurement {
    type Key = u64;

    fn key(&self) -> u64 {
        self.id
    }
}

/// Deterministic pseudo-random measurements, generated once.
fn measurements() -> Vec<Measurement> {
    let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
    (1..=ROWS)
        .map(|id| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            Measurement {
                id,
                sensor: format!("sensor-{:03}", seed % 250),
                value: (seed % 100_000) as f64 / 100.0,
                ok: seed % 17 != 0,
            }
        })
        .collect()
}

#[component]
fn App() -> Element {
    let rows = use_signal(measurements);

    let columns = use_hook(|| {
        vec![
            Column::new("id", "#")
                .cell(|row: &Measurement| rsx! { "{row.id}" })
                .sort_by_value(|row: &Measurement| row.id)
                .width(ColumnWidth::Px(96.0)),
            Column::new("sensor", "Sensor")
                .cell(|row: &Measurement| rsx! { "{row.sensor}" })
                .sort_by_text(|row: &Measurement| row.sensor.as_str())
                .filter_by(|row: &Measurement| row.sensor.clone()),
            Column::new("value", "Value")
                .cell(|row: &Measurement| rsx! { "{row.value:.2}" })
                .sort_by_value(|row: &Measurement| row.value),
            Column::new("ok", "Status")
                .cell(|row: &Measurement| rsx! { if row.ok { "ok" } else { "fault" } })
                .sort_by_value(|row: &Measurement| row.ok)
                .width(ColumnWidth::Px(96.0)),
        ]
    });

    // No page size: virtualization replaces paging.
    let grid = use_grid(
        rows,
        columns,
        GridOptions::default().selection(SelectionMode::Multi),
    );

    rsx! {
        document::Link { rel: "stylesheet", href: STYLE }

        main { class: "page",
            h1 { "{ROWS} rows, virtualized" }
            div { class: "toolbar",
                GridSearch { grid, placeholder: "Filter by sensor" }
                span { "{grid.filtered_len()} rows" }
            }

            GridRoot { grid, class: "grid",
                GridHeader { grid, class: "head" }
                VirtualGridBody { grid, row_height: ROW_HEIGHT, overscan: 8, class: "body" }
            }
        }
    }
}
