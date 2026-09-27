//! Server-rendered checks on cell selection: what a grid says about selected
//! cells, and what it keeps quiet about when cells cannot be selected at all.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{CellFocus, CellSelectionMode, GridRow, SelectionMode};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{GridBody, GridHeader, GridRoot};
use dioxus_datagrid::{Column, GridOptions, use_grid};

#[derive(Clone, PartialEq)]
struct Row {
    id: u32,
    name: String,
    city: String,
}

impl GridRow for Row {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

fn rows() -> Vec<Row> {
    (1..=3)
        .map(|id| Row {
            id,
            name: format!("Name {id}"),
            city: format!("City {id}"),
        })
        .collect()
}

fn columns() -> Vec<Column<Row>> {
    vec![
        Column::new("name", "Name").value_text(|row: &Row| row.name.as_str()),
        Column::new("city", "City").value_text(|row: &Row| row.city.as_str()),
    ]
}

#[derive(Clone, PartialEq, Props)]
struct Setup {
    cells: CellSelectionMode,
    #[props(default)]
    rows: SelectionMode,
    /// The rectangle to make before rendering, as `(anchor, focus)` in focus
    /// coordinates — row 0 is the header.
    #[props(default)]
    range: Option<((usize, usize), (usize, usize))>,
}

#[component]
fn Grid(setup: Setup) -> Element {
    let data = use_signal(rows);
    let cols = use_hook(columns);
    let grid = use_grid(
        data,
        cols,
        GridOptions::default()
            .selection(setup.rows)
            .cell_selection(setup.cells),
    );

    // The selection a user would have made with a click and a shift-click.
    use_hook(move || {
        if let Some((anchor, focus)) = setup.range {
            let mut grid = grid;
            grid.select_cell(CellFocus::new(anchor.0, anchor.1));
            grid.extend_cell_selection(CellFocus::new(focus.0, focus.1));
        }
    });

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

/// Every data cell's `aria-selected`, in the order they are rendered, with
/// `"-"` for a cell that does not carry it at all.
fn selected(html: &str) -> Vec<&str> {
    html.split(r#"role="gridcell""#)
        .skip(1)
        .map(|cell| {
            let tag = cell.split('>').next().unwrap_or_default();
            match tag.split(r#"aria-selected=""#).nth(1) {
                Some(rest) => rest.split('"').next().unwrap_or("-"),
                None => "-",
            }
        })
        .collect()
}

#[test]
fn a_grid_that_does_not_select_cells_says_nothing_about_them() {
    let html = render(Setup::builder().cells(CellSelectionMode::None).build());

    assert_eq!(selected(&html), ["-", "-", "-", "-", "-", "-"], "{html}");
    assert!(!html.contains("data-cell-selected"), "{html}");
}

#[test]
fn a_grid_that_selects_cells_says_so_on_every_cell() {
    let html = render(Setup::builder().cells(CellSelectionMode::Range).build());

    // Nothing selected yet, but every cell is a candidate, so every cell says
    // where it stands.
    assert_eq!(
        selected(&html),
        ["false", "false", "false", "false", "false", "false"],
        "{html}"
    );
}

#[test]
fn the_selected_rectangle_is_marked_cell_by_cell() {
    // From the first data row's first column to the second row's second.
    let html = render(
        Setup::builder()
            .cells(CellSelectionMode::Range)
            .range(Some(((1, 0), (2, 1))))
            .build(),
    );

    assert_eq!(
        selected(&html),
        ["true", "true", "true", "true", "false", "false"],
        "{html}"
    );
    assert_eq!(html.matches("data-cell-selected").count(), 4, "{html}");
}

#[test]
fn a_single_cell_mode_never_selects_two() {
    let html = render(
        Setup::builder()
            .cells(CellSelectionMode::Single)
            .range(Some(((1, 0), (2, 1))))
            .build(),
    );

    // The extend landed on one cell instead of growing a rectangle.
    assert_eq!(html.matches("data-cell-selected").count(), 1, "{html}");
    assert_eq!(
        selected(&html),
        ["false", "false", "false", "true", "false", "false"],
        "{html}"
    );
}

#[test]
fn a_header_cell_is_not_selectable() {
    let html = render(
        Setup::builder()
            .cells(CellSelectionMode::Range)
            .range(Some(((0, 0), (0, 1))))
            .build(),
    );

    // Row 0 is the header; the selection was refused rather than clamped into
    // the body.
    assert!(!html.contains("data-cell-selected"), "{html}");
}

#[test]
fn a_rectangle_makes_the_grid_multiselectable() {
    let range = render(Setup::builder().cells(CellSelectionMode::Range).build());
    assert!(range.contains(r#"aria-multiselectable="true""#), "{range}");

    // One cell at a time is not a multi-selection, and neither is no cell.
    let single = render(Setup::builder().cells(CellSelectionMode::Single).build());
    assert!(!single.contains("aria-multiselectable"), "{single}");
    let none = render(Setup::builder().cells(CellSelectionMode::None).build());
    assert!(!none.contains("aria-multiselectable"), "{none}");
}

#[test]
fn rows_and_cells_are_selected_separately() {
    let html = render(
        Setup::builder()
            .cells(CellSelectionMode::Range)
            .rows(SelectionMode::Multi)
            .range(Some(((1, 0), (1, 1))))
            .build(),
    );

    // The row says it is not selected; two of its cells say they are.
    assert!(html.contains(r#"aria-selected="false""#), "{html}");
    assert_eq!(html.matches("data-cell-selected").count(), 2, "{html}");
    assert_eq!(
        html.matches(r#"data-selected="false""#).count(),
        3,
        "{html}"
    );
}
