//! Server-rendered checks on multi-level column headers: the extra rows, their
//! spans, and what they do to the row counts everything else is measured by.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{GridRow, GridState};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{GridBody, GridHeader, GridRoot};
use dioxus_datagrid::{Column, GridOptions, use_grid};

#[derive(Clone, PartialEq)]
struct User {
    id: u32,
    first: String,
    last: String,
    city: String,
    age: u32,
}

impl GridRow for User {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

fn users() -> Vec<User> {
    (1..=3)
        .map(|id| User {
            id,
            first: format!("First{id}"),
            last: format!("Last{id}"),
            city: "Berlin".to_owned(),
            age: 30 + id,
        })
        .collect()
}

/// Name.First, Name.Last, City — all under Person — then an ungrouped Age.
fn columns(grouped: bool) -> Vec<Column<User>> {
    let group = |column: Column<User>, path: &[&str]| {
        if grouped {
            path.iter()
                .fold(column, |column, label| column.group(*label))
        } else {
            column
        }
    };

    vec![
        group(
            Column::new("first", "First").value_text(|user: &User| user.first.as_str()),
            &["Person", "Name"],
        ),
        group(
            Column::new("last", "Last").value_text(|user: &User| user.last.as_str()),
            &["Person", "Name"],
        ),
        group(
            Column::new("city", "City").value_text(|user: &User| user.city.as_str()),
            &["Person"],
        ),
        Column::new("age", "Age").value_of(|user: &User| user.age),
    ]
}

#[derive(Clone, PartialEq, Props)]
struct Setup {
    #[props(default = true)]
    grouped: bool,
    #[props(default)]
    state: Option<GridState>,
}

#[component]
fn Grid(setup: Setup) -> Element {
    let rows = use_signal(users);
    let grouped = setup.grouped;
    let cols = use_hook(move || columns(grouped));

    let mut options = GridOptions::default();
    if let Some(state) = setup.state.clone() {
        options = options.initial_state(state);
    }
    let grid = use_grid(rows, cols, options);

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

fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

#[test]
fn an_ungrouped_grid_has_one_header_row() {
    let html = render(Setup::builder().grouped(false).build());

    assert!(!html.contains("data-group-header"), "{html}");
    // Three rows plus the one header row.
    assert!(html.contains(r#"aria-rowcount="4""#), "{html}");
}

#[test]
fn each_level_of_groups_adds_a_header_row() {
    let html = render(Setup::builder().build());

    assert_eq!(count(&html, r#"data-group-header-row="#), 2, "{html}");
    // Three rows, plus two group rows and the column headers.
    assert!(html.contains(r#"aria-rowcount="6""#), "{html}");
}

#[test]
fn a_group_spans_the_columns_it_covers() {
    let html = render(Setup::builder().build());

    // Person covers First, Last and City; Name covers First and Last.
    assert!(html.contains(r#"aria-colspan="3""#), "{html}");
    assert!(html.contains(r#"aria-colspan="2""#), "{html}");
    assert!(html.contains("Person"), "{html}");
    assert!(html.contains("Name"), "{html}");
}

#[test]
fn a_column_outside_every_group_gets_an_empty_cell_above_it() {
    let html = render(Setup::builder().build());

    // Age is in no group, so both rows above it hold a cell with no label —
    // without them the rows would not cover every column.
    let level_one = html
        .split(r#"data-group-header-row="0""#)
        .nth(1)
        .unwrap()
        .split("</div></div>")
        .next()
        .unwrap();
    assert!(
        level_one.contains(r#"aria-colindex="4""#),
        "the outer row must reach column 4: {level_one}"
    );
}

#[test]
fn the_header_rows_are_numbered_in_order() {
    let html = render(Setup::builder().build());

    for index in 1..=3 {
        assert!(
            html.contains(&format!(r#"aria-rowindex="{index}""#)),
            "missing header row {index}: {html}"
        );
    }
}

#[test]
fn the_first_data_row_starts_below_the_whole_header() {
    let html = render(Setup::builder().build());

    // Two group rows and the column headers are rows 1 to 3, so the first data
    // row is row 4. Getting this wrong is what a screen reader would read out.
    assert!(html.contains(r#"aria-rowindex="4""#), "{html}");
    assert!(html.contains("First1"), "{html}");
}

#[test]
fn the_roving_tab_stop_sits_on_the_column_headers_not_above_them() {
    let html = render(Setup::builder().build());

    // Exactly one tab stop, and it is in the last header row: that is where
    // the focus starts, so Tab lands on a column rather than on a group.
    assert_eq!(count(&html, r#"tabindex="0""#), 1, "{html}");
    let stop = html.find(r#"tabindex="0""#).unwrap();
    let before = &html[..stop];
    assert!(
        before.rfind(r#"aria-rowindex="3""#) > before.rfind(r#"aria-rowindex="2""#),
        "the tab stop belongs to the third header row: {html}"
    );
}

#[test]
fn hiding_a_whole_group_drops_its_header_row() {
    let mut state = GridState::new();
    state.set_column_hidden("first", true);
    state.set_column_hidden("last", true);

    let html = render(Setup::builder().state(Some(state)).build());

    // Name had only those two columns, so that level is gone; Person remains
    // over City alone.
    assert_eq!(count(&html, r#"data-group-header-row="#), 1, "{html}");
    assert!(html.contains("Person"), "{html}");
    assert!(!html.contains(">Name<"), "{html}");
    // Two rows of header now, so the data rows move up with them.
    assert!(html.contains(r#"aria-rowcount="5""#), "{html}");
}
