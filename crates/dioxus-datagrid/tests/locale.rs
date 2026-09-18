//! Texts and formats from the locale, formatted cells, and a selection that
//! follows its rows out of the data.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{GridRow, SelectionMode};
use dioxus::core::NoOpMutations;
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{
    GridBody, GridColumnFilter, GridHeader, GridPagination, GridRoot, GridSearch,
};
use dioxus_datagrid::{CellFormat, CellOverflow, Column, GridLocale, GridOptions, use_grid};
use std::time::Duration;
use tokio::time::{Instant, sleep, sleep_until};

#[derive(Clone, PartialEq)]
struct Employee {
    id: u32,
    name: String,
    salary: f64,
}

impl GridRow for Employee {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

fn employees() -> Vec<Employee> {
    [(1, "Ada", 5250.5), (2, "Ben", 71_000.0), (3, "Cleo", 980.0)]
        .into_iter()
        .map(|(id, name, salary)| Employee {
            id,
            name: name.to_owned(),
            salary,
        })
        .collect()
}

fn columns() -> Vec<Column<Employee>> {
    vec![
        Column::new("name", "Name")
            .value_text(|row: &Employee| row.name.as_str())
            .overflow(CellOverflow::TruncateWithTooltip)
            .filter_by(|row: &Employee| row.name.clone()),
        Column::new("salary", "Salary")
            .value_of(|row: &Employee| row.salary)
            .format(CellFormat::currency("€", 2)),
    ]
}

#[component]
fn Localized(german: bool) -> Element {
    let rows = use_signal(employees);
    let cols = use_hook(columns);
    let locale = if german {
        GridLocale::german()
    } else {
        GridLocale::english()
    };
    let grid = use_grid(rows, cols, GridOptions::paged(2).locale(locale));

    rsx! {
        GridSearch { grid }
        GridColumnFilter { grid, column_index: 0 }
        GridRoot { grid,
            GridHeader { grid }
            GridBody { grid }
        }
        GridPagination { grid }
    }
}

fn render(german: bool) -> String {
    let mut dom = VirtualDom::new_with_props(Localized, LocalizedProps { german });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn english_is_the_default() {
    let html = render(false);

    assert!(html.contains(r#"placeholder="Search""#), "{html}");
    assert!(html.contains(r#"aria-label="Filter Name""#), "{html}");
    assert!(html.contains(r#"aria-label="Next page""#), "{html}");
    assert!(html.contains("Page 1 of 2"), "{html}");
    assert!(html.contains(">€5,250.50<"), "{html}");
}

#[test]
fn the_german_locale_translates_texts_and_formats() {
    let html = render(true);

    assert!(html.contains(r#"placeholder="Suchen""#), "{html}");
    assert!(html.contains(r#"aria-label="Name filtern""#), "{html}");
    assert!(html.contains(r#"aria-label="Nächste Seite""#), "{html}");
    assert!(html.contains(">Zurück<"), "{html}");
    assert!(html.contains("Seite 1 von 2"), "{html}");
    assert!(html.contains(">5.250,50\u{a0}€<"), "{html}");
    assert!(html.contains(">71.000,00\u{a0}€<"), "{html}");
}

#[test]
fn a_value_without_a_cell_renderer_is_shown_formatted() {
    let html = render(false);
    // The name column has no `.cell()`; its value is its content.
    assert!(html.contains(">Ada<"), "{html}");
}

#[test]
fn numeric_columns_align_at_the_end_header_included() {
    let html = render(false);

    assert_eq!(html.matches(r#"data-align="end""#).count(), 3, "{html}");
    assert_eq!(html.matches(r#"data-align="start""#).count(), 3, "{html}");
}

#[test]
fn only_truncate_with_tooltip_sets_a_title() {
    let html = render(false);

    assert!(html.contains(r#"title="Ada""#), "{html}");
    assert!(!html.contains(r#"title="€"#), "{html}");
}

#[component]
fn Shrinking() -> Element {
    let mut rows = use_signal(employees);
    let cols = use_hook(columns);
    let mut grid = use_grid(
        rows,
        cols,
        GridOptions::default().selection(SelectionMode::Multi),
    );

    use_hook(move || {
        grid.select(1);
        grid.toggle_select(2);
        spawn(async move {
            sleep(Duration::from_millis(10)).await;
            rows.write().retain(|row| row.id != 1);
        })
    });

    let mut keys = grid.selected_keys();
    keys.sort_unstable();
    rsx! {
        output { "{keys:?}" }
    }
}

/// A row deleted from the data leaves the selection with it, instead of
/// lingering as a selected row nobody can see or deselect.
#[tokio::test(start_paused = true)]
async fn a_selected_row_that_leaves_the_data_leaves_the_selection() {
    let mut dom = VirtualDom::new(Shrinking);
    dom.rebuild_in_place();
    assert!(dioxus_ssr::render(&dom).contains("[1, 2]"));

    let deadline = Instant::now() + Duration::from_millis(100);
    loop {
        tokio::select! {
            () = dom.wait_for_work() => dom.render_immediate(&mut NoOpMutations),
            () = sleep_until(deadline) => break,
        }
    }

    let html = dioxus_ssr::render(&dom);
    assert!(html.contains("<output>[2]</output>"), "{html}");
}
