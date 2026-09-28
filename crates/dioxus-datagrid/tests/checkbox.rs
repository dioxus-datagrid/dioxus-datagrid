//! Server-rendered checks on the checkbox column: what the boxes say about the
//! selection, what the header's box says about the page, and what a grid that
//! cannot select rows draws instead.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{GridRow, SelectionMode};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{GridBody, GridHeader, GridRoot};
use dioxus_datagrid::{Column, GridOptions, use_grid};

#[derive(Clone, PartialEq)]
struct Row {
    id: u32,
    name: String,
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
        })
        .collect()
}

fn columns() -> Vec<Column<Row>> {
    vec![
        Column::new("select", "").checkbox(),
        Column::new("name", "Name").value_text(|row: &Row| row.name.as_str()),
    ]
}

#[derive(Clone, PartialEq, Props)]
struct Setup {
    #[props(default = SelectionMode::Multi)]
    mode: SelectionMode,
    /// The rows to select before rendering, by key.
    #[props(default)]
    selected: Vec<u32>,
    /// Renders the grid without a checkbox column, for the comparisons.
    #[props(default)]
    plain: bool,
}

#[component]
fn Grid(setup: Setup) -> Element {
    let data = use_signal(rows);
    let cols = use_hook({
        let plain = setup.plain;
        move || {
            if plain {
                columns().into_iter().skip(1).collect()
            } else {
                columns()
            }
        }
    });
    let grid = use_grid(data, cols, GridOptions::default().selection(setup.mode));

    use_hook(move || {
        let mut grid = grid;
        for key in &setup.selected {
            grid.select(*key);
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

/// Every rendered checkbox, as the whole `<input …>` tag.
fn boxes(html: &str) -> Vec<String> {
    html.split("<input")
        .skip(1)
        .map(|rest| rest.split('>').next().unwrap_or_default().to_owned())
        .collect()
}

/// Whether each box is ticked, in the order they are rendered — the header's
/// first.
fn ticked(html: &str) -> Vec<bool> {
    boxes(html)
        .iter()
        .map(|tag| tag.contains("checked"))
        .collect()
}

#[test]
fn every_row_gets_a_box_and_the_header_gets_one_too() {
    let html = render(Setup::builder().build());

    // One for the header, one per row.
    assert_eq!(boxes(&html).len(), 4, "{html}");
    assert!(boxes(&html)[0].contains(r#"data-select-all="none""#));
    assert!(boxes(&html)[0].contains(r#"aria-label="Select all rows""#));
    assert!(boxes(&html)[1].contains(r#"aria-label="Select row""#));
    // Not a tab stop: the grid is one, and the cell around each box carries it.
    assert!(
        boxes(&html)
            .iter()
            .all(|tag| tag.contains(r#"tabindex="-1""#)),
        "{html}"
    );
}

#[test]
fn a_box_is_ticked_when_its_row_is_selected() {
    let html = render(Setup::builder().selected(vec![2]).build());

    assert_eq!(ticked(&html), [false, false, true, false]);
}

#[test]
fn the_header_box_says_when_only_some_rows_are_selected() {
    let html = render(Setup::builder().selected(vec![1, 3]).build());

    assert!(
        boxes(&html)[0].contains(r#"data-select-all="partial""#),
        "{html}"
    );
    // Not ticked: not all of them are selected. The mixed state itself is a DOM
    // property, set after rendering (docs/VERIFICATION.md §16).
    assert!(!ticked(&html)[0]);
}

#[test]
fn the_header_box_is_ticked_when_every_row_is_selected() {
    let html = render(Setup::builder().selected(vec![1, 2, 3]).build());

    assert!(
        boxes(&html)[0].contains(r#"data-select-all="all""#),
        "{html}"
    );
    assert_eq!(ticked(&html), [true, true, true, true]);
}

#[test]
fn a_single_selection_gets_boxes_but_no_select_all() {
    let html = render(
        Setup::builder()
            .mode(SelectionMode::Single)
            .selected(vec![2])
            .build(),
    );

    // Three boxes, not four: "all of them" is not something a single selection
    // can hold.
    assert_eq!(boxes(&html).len(), 3, "{html}");
    assert_eq!(ticked(&html), [false, true, false]);
}

#[test]
fn a_grid_that_cannot_select_rows_draws_no_boxes() {
    let html = render(Setup::builder().mode(SelectionMode::None).build());

    assert!(boxes(&html).is_empty(), "{html}");
    // The column is still there, and still a cell: it just has nothing to show.
    assert_eq!(html.matches(r#"role="gridcell""#).count(), 6);
}

#[test]
fn the_header_cell_borrows_the_boxs_name_when_it_has_none_of_its_own() {
    let html = render(Setup::builder().build());

    // An unnamed column header would be a cell a reader cannot place.
    let header = html
        .split(r#"role="columnheader""#)
        .nth(1)
        .unwrap_or_default();
    assert!(
        header.contains(r#"aria-label="Select all rows""#),
        "{header}"
    );
    // And it does not offer a sort, because there is nothing to sort by.
    assert!(header.contains(r#"data-sortable="false""#), "{header}");
    assert!(!header.contains("aria-sort"), "{header}");
}

#[test]
fn a_grid_without_a_checkbox_column_has_no_boxes_at_all() {
    let html = render(Setup::builder().plain(true).selected(vec![1]).build());

    assert!(boxes(&html).is_empty(), "{html}");
    // The selection itself is unaffected, and still announced on the row.
    assert_eq!(html.matches(r#"aria-selected="true""#).count(), 1);
}

/// What `Ctrl+C` would copy with these rows selected.
fn copied(selected: Vec<u32>) -> String {
    #[component]
    fn CopyHarness(selected: Vec<u32>) -> Element {
        let data = use_signal(rows);
        let cols = use_hook(columns);
        let grid = use_grid(
            data,
            cols,
            GridOptions::default().selection(SelectionMode::Multi),
        );
        use_hook(move || {
            let mut grid = grid;
            for key in &selected {
                grid.select(*key);
            }
        });

        rsx! {
            pre { {grid.copy_text().unwrap_or_default()} }
        }
    }

    let mut dom = VirtualDom::new_with_props(CopyHarness, CopyHarnessProps { selected });
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    html.split("<pre>")
        .nth(1)
        .and_then(|rest| rest.split("</pre>").next())
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn copying_selected_rows_leaves_the_checkbox_column_out() {
    // It holds no value, so it would paste an empty first column into a sheet.
    assert_eq!(copied(vec![1, 3]), "Name 1\r\nName 3");
}
