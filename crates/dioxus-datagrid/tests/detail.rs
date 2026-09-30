//! Server-rendered checks on detail rows: what an opened row says, what the
//! row below it is, and what a grid says when it has no details at all.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{GridRow, SelectionMode};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{GridBody, GridHeader, GridRoot};
use dioxus_datagrid::{Column, DetailRows, GridOptions, use_grid};

#[derive(Clone, PartialEq)]
struct Row {
    id: u32,
    name: String,
    note: String,
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
            // The second row has nothing to show.
            note: if id == 2 {
                String::new()
            } else {
                format!("Note {id}")
            },
        })
        .collect()
}

fn columns() -> Vec<Column<Row>> {
    vec![
        Column::new("expand", "").expander(),
        Column::new("name", "Name").value_text(|row: &Row| row.name.as_str()),
    ]
}

#[derive(Clone, PartialEq, Props)]
struct Setup {
    /// The rows to open, by position on the page.
    #[props(default)]
    open: Vec<usize>,
    /// Without it the grid has no detail rows at all.
    #[props(default = true)]
    details: bool,
    /// Only rows with a note have a detail.
    #[props(default = true)]
    some_rows: bool,
    /// Mounts a virtualized body, which detail rows do not mix with.
    #[props(default)]
    virtualized: bool,
}

#[component]
fn Grid(setup: Setup) -> Element {
    let data = use_signal(rows);
    let cols = use_hook(columns);
    let mut grid = use_grid(
        data,
        cols,
        GridOptions::default().selection(SelectionMode::Multi),
    );

    if setup.details {
        grid.set_detail_rows(DetailRows {
            render: Callback::new(|row: Row| {
                rsx! {
                    p { "data-note": "", "{row.note}" }
                }
            }),
            has_detail: setup
                .some_rows
                .then(|| Callback::new(|row: Row| !row.note.is_empty())),
        });
    }

    use_hook(move || {
        let mut grid = grid;
        if setup.virtualized {
            grid.set_virtual_body(Some((32.0, 5)));
        }
        for row in &setup.open {
            grid.toggle_detail(*row);
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

/// Every `role="row"` element's opening tag, in the order they are rendered.
fn row_tags(html: &str) -> Vec<String> {
    html.split("<div")
        .filter(|tag| tag.contains(r#"role="row""#))
        .map(|tag| tag.split('>').next().unwrap_or_default().to_owned())
        .collect()
}

/// The value of an attribute in a tag, if it has one.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let rest = tag.get(start..)?;
    Some(rest.split('"').next()?.to_owned())
}

#[test]
fn an_opened_row_is_followed_by_its_detail() {
    let html = render(Setup::builder().open(vec![0]).build());

    let rows = row_tags(&html);
    // The header row, the opened row, its detail, and the two rows after it.
    assert_eq!(rows.len(), 5, "{html}");
    assert_eq!(
        attribute(&rows[1], "data-expanded").as_deref(),
        Some("true")
    );
    assert!(rows[2].contains("data-detail-row"), "{}", rows[2]);
    // The row says so for styling only; what says it to a reader is the button
    // that did it — `aria-expanded` on a row is for a treegrid.
    assert!(!rows[1].contains("aria-expanded"), "{}", rows[1]);
    assert!(html.contains(r#"aria-expanded="true""#), "{html}");
    // The detail carries what the application rendered for that row.
    assert!(html.contains("Note 1"), "{html}");
}

#[test]
fn the_detail_row_takes_the_next_row_number() {
    let html = render(Setup::builder().open(vec![0]).build());
    let rows = row_tags(&html);

    // The header, the opened row, its detail, and the two rows after it: every
    // row of the view counted once, in the order they are shown. A detail row
    // that took no number would leave the rows after it saying the wrong one.
    let indices: Vec<String> = rows
        .iter()
        .filter_map(|tag| attribute(tag, "aria-rowindex"))
        .collect();
    assert_eq!(indices, ["1", "2", "3", "4", "5"]);
    assert!(rows[2].contains("data-detail-row"), "{}", rows[2]);
}

#[test]
fn the_button_points_at_the_row_it_opened() {
    let html = render(Setup::builder().open(vec![0]).build());

    let controls = html
        .split("aria-controls=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_default()
        .to_owned();
    assert!(controls.starts_with("dg-"), "{html}");
    // And that is the id of the detail row itself.
    assert!(html.contains(&format!(r#"id="{controls}""#)), "{html}");
}

#[test]
fn a_closed_row_points_at_nothing() {
    let html = render(Setup::builder().build());

    assert_eq!(
        attribute(&row_tags(&html)[1], "data-expanded").as_deref(),
        Some("false")
    );
    assert!(html.contains(r#"aria-expanded="false""#), "{html}");
    // A reference to an element that is not there would say nothing at all.
    assert!(!html.contains("aria-controls"), "{html}");
}

#[test]
fn a_row_with_nothing_to_show_has_no_button_and_says_nothing() {
    let html = render(Setup::builder().build());
    let rows = row_tags(&html);

    // The second data row has no note, so it has no detail.
    assert_eq!(attribute(&rows[2], "data-expanded"), None, "{}", rows[2]);
    // Two buttons for three rows.
    assert_eq!(html.matches("data-expander").count(), 2, "{html}");
}

#[test]
fn every_row_has_a_detail_when_the_grid_does_not_say_otherwise() {
    let html = render(Setup::builder().some_rows(false).build());

    assert_eq!(html.matches("data-expander").count(), 3, "{html}");
}

#[test]
fn a_grid_without_detail_rows_draws_no_buttons() {
    let html = render(Setup::builder().details(false).open(vec![0]).build());

    assert!(!html.contains("data-expander"), "{html}");
    assert!(!html.contains("data-detail-row"), "{html}");
    assert!(!html.contains("aria-expanded"), "{html}");
    // The column is still there, with a cell in every row.
    assert_eq!(html.matches(r#"role="gridcell""#).count(), 6);
}

#[test]
fn a_virtualized_grid_refuses_to_open_one() {
    let html = render(Setup::builder().virtualized(true).open(vec![0]).build());

    // Its rows are placed by counting equal heights; a detail row is as tall as
    // what it holds, so opening one would put every row after it in the wrong
    // place.
    assert!(!html.contains("data-detail-row"), "{html}");
    // No button either: there is nothing it could do.
    assert!(!html.contains("data-expander"), "{html}");
    assert_eq!(
        attribute(&row_tags(&html)[1], "data-expanded"),
        None,
        "{html}"
    );
}

#[test]
fn opening_and_closing_the_same_row_leaves_it_closed() {
    let html = render(Setup::builder().open(vec![0, 0]).build());

    assert!(!html.contains("data-detail-row"), "{html}");
    assert_eq!(
        attribute(&row_tags(&html)[1], "data-expanded").as_deref(),
        Some("false")
    );
}
