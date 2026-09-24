//! Server-rendered checks on the column order: that a reordered grid renders
//! its headers and cells in the new order with `aria-colindex` to match, and
//! that a reorderable header announces its keys.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{ColumnId, GridRow, GridState};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{GridBody, GridHeader, GridRoot};
use dioxus_datagrid::{Column, GridOptions, use_grid};

#[derive(Clone, PartialEq)]
struct User {
    id: u32,
    name: String,
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
    vec![User {
        id: 1,
        name: "Zoe".to_owned(),
        city: "Berlin".to_owned(),
        age: 30,
    }]
}

fn columns() -> Vec<Column<User>> {
    vec![
        Column::new("name", "Name").value_text(|user: &User| user.name.as_str()),
        Column::new("city", "City").value_text(|user: &User| user.city.as_str()),
        Column::new("age", "Age").value_of(|user: &User| user.age),
    ]
}

#[derive(Clone, PartialEq, Props)]
struct Setup {
    #[props(default)]
    state: Option<GridState>,
    #[props(default)]
    reorderable: bool,
}

#[component]
fn Grid(setup: Setup) -> Element {
    let rows = use_signal(users);
    let cols = use_hook(columns);

    let mut options = GridOptions::default();
    if let Some(state) = setup.state.clone() {
        options = options.initial_state(state);
    }
    let grid = use_grid(rows, cols, options);

    rsx! {
        GridRoot { grid,
            GridHeader { grid, reorderable: setup.reorderable }
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

/// The order the column labels appear in the markup.
fn header_order(html: &str) -> Vec<&str> {
    ["Name", "City", "Age"]
        .into_iter()
        .filter_map(|label| html.find(label).map(|at| (at, label)))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_values()
        .collect()
}

fn order(names: &[&str]) -> GridState {
    let mut state = GridState::new();
    state.set_column_order(
        names
            .iter()
            .map(|name| ColumnId::new(name.to_string()))
            .collect(),
    );
    state
}

#[test]
fn columns_render_in_their_declared_order_by_default() {
    let html = render(Setup::builder().build());
    assert_eq!(header_order(&html), ["Name", "City", "Age"]);
}

#[test]
fn a_column_order_in_the_state_reorders_the_headers() {
    let html = render(
        Setup::builder()
            .state(Some(order(&["age", "name"])))
            .build(),
    );
    assert_eq!(header_order(&html), ["Age", "Name", "City"]);
}

#[test]
fn aria_colindex_follows_the_new_order() {
    let html = render(Setup::builder().state(Some(order(&["age"]))).build());

    // Age is now the first column, so it is colindex 1 — in the header and in
    // the body, or keyboard navigation and screen readers would disagree.
    let age_header = html
        .split("<div ")
        .find(|tag| tag.contains(r#"aria-label="Age""#))
        .unwrap();
    assert!(
        age_header.contains(r#"aria-colindex="1""#),
        "the Age header should carry colindex 1: {html}"
    );
    assert!(html.contains(r#"aria-colindex="3""#));
}

#[test]
fn the_cells_follow_the_headers() {
    let html = render(Setup::builder().state(Some(order(&["city"]))).build());

    let berlin = html.find("Berlin").unwrap();
    let zoe = html.find("Zoe").unwrap();
    assert!(berlin < zoe, "City moved first, so did its cell: {html}");
}

#[test]
fn a_plain_header_announces_no_reordering_keys() {
    let html = render(Setup::builder().build());
    assert!(!html.contains("Alt+Shift+ArrowLeft"), "{html}");
    assert!(!html.contains(r#"draggable="true""#), "{html}");
}

#[test]
fn a_reorderable_header_is_draggable_and_announces_its_keys() {
    let html = render(Setup::builder().reorderable(true).build());

    assert!(html.contains(r#"draggable="true""#), "{html}");
    assert!(
        html.contains("aria-keyshortcuts=\"Alt+Shift+ArrowLeft Alt+Shift+ArrowRight\""),
        "{html}"
    );
}

#[test]
fn hidden_columns_keep_their_place_in_the_order() {
    let mut state = order(&["age", "name", "city"]);
    state.set_column_hidden(ColumnId::new("name"), true);

    let html = render(Setup::builder().state(Some(state)).build());

    // Name is out of the way, but it has not taken City's place with it.
    assert_eq!(header_order(&html), ["Age", "City"]);
    assert!(html.contains(r#"aria-colcount="2""#), "{html}");
}
