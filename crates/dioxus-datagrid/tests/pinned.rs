//! Server-rendered checks on pinned columns: that they are laid out as a block
//! at their edge whatever the column order says, and that each one is told how
//! far from that edge it sits.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{ColumnId, ColumnWidth, GridRow, GridState, Pinned};
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

/// Fixed widths throughout, so the offsets are known without a browser: a
/// measured width only arrives once something has been laid out.
fn columns(pins: &[(&'static str, Pinned)]) -> Vec<Column<User>> {
    let pin_of = |id: &str| {
        pins.iter()
            .find(|(name, _)| *name == id)
            .map_or(Pinned::None, |(_, pin)| *pin)
    };

    vec![
        Column::new("name", "Name")
            .value_text(|user: &User| user.name.as_str())
            .width(ColumnWidth::Px(100.0))
            .pin(pin_of("name")),
        Column::new("city", "City")
            .value_text(|user: &User| user.city.as_str())
            .width(ColumnWidth::Px(60.0))
            .pin(pin_of("city")),
        Column::new("age", "Age")
            .value_of(|user: &User| user.age)
            .width(ColumnWidth::Px(40.0))
            .pin(pin_of("age")),
    ]
}

#[derive(Clone, PartialEq, Props)]
struct Setup {
    #[props(default)]
    pins: Vec<(String, Pinned)>,
    #[props(default)]
    state: Option<GridState>,
}

#[component]
fn Grid(setup: Setup) -> Element {
    let rows = use_signal(users);
    let pins: Vec<(&'static str, Pinned)> = setup
        .pins
        .iter()
        .map(|(id, pin)| {
            let id: &'static str = match id.as_str() {
                "name" => "name",
                "city" => "city",
                _ => "age",
            };
            (id, *pin)
        })
        .collect();
    let cols = use_hook(move || columns(&pins));

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

fn pins(pairs: &[(&str, Pinned)]) -> Vec<(String, Pinned)> {
    pairs
        .iter()
        .map(|(id, pin)| ((*id).to_owned(), *pin))
        .collect()
}

/// The column labels in the order they are rendered.
fn header_order(html: &str) -> Vec<&str> {
    ["Name", "City", "Age"]
        .into_iter()
        .filter_map(|label| html.find(label).map(|at| (at, label)))
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_values()
        .collect()
}

#[test]
fn a_column_that_is_not_pinned_says_nothing() {
    let html = render(Setup::builder().build());

    assert!(!html.contains("data-pinned"), "{html}");
    assert!(!html.contains("--dg-pin-offset"), "{html}");
}

#[test]
fn a_pinned_column_carries_its_edge_and_its_offset() {
    let html = render(
        Setup::builder()
            .pins(pins(&[("name", Pinned::Start)]))
            .build(),
    );

    assert!(html.contains(r#"data-pinned="start""#), "{html}");
    // First in the block, so it sits at the edge itself.
    assert!(html.contains("--dg-pin-offset: 0px;"), "{html}");
}

#[test]
fn the_second_pinned_column_clears_the_first() {
    let html = render(
        Setup::builder()
            .pins(pins(&[("name", Pinned::Start), ("city", Pinned::Start)]))
            .build(),
    );

    // Name is 100px wide, so City starts there.
    assert!(html.contains("--dg-pin-offset: 100px;"), "{html}");
}

#[test]
fn pinning_to_the_end_counts_from_the_other_side() {
    let html = render(
        Setup::builder()
            .pins(pins(&[("city", Pinned::End), ("age", Pinned::End)]))
            .build(),
    );

    // City and Age both move to the end block, in the column order: City then
    // Age. Age is at the edge, City clears Age's 40px.
    assert_eq!(header_order(&html), ["Name", "City", "Age"]);
    assert!(html.contains("--dg-pin-offset: 40px;"), "{html}");
    assert!(html.contains("--dg-pin-offset: 0px;"), "{html}");
}

#[test]
fn a_pinned_column_moves_to_its_edge_whatever_the_order_says() {
    let html = render(
        Setup::builder()
            .pins(pins(&[("age", Pinned::Start)]))
            .build(),
    );

    // Age is declared last and pinned to the start, so it is shown first.
    assert_eq!(header_order(&html), ["Age", "Name", "City"]);
}

#[test]
fn the_state_pins_a_column_the_grid_did_not() {
    let mut state = GridState::new();
    state.set_pinned(ColumnId::new("city"), Pinned::Start);

    let html = render(Setup::builder().state(Some(state)).build());

    assert_eq!(header_order(&html), ["City", "Name", "Age"]);
    assert!(html.contains(r#"data-pinned="start""#), "{html}");
}

#[test]
fn the_state_unpins_a_column_the_grid_pinned() {
    let mut state = GridState::new();
    state.set_pinned(ColumnId::new("age"), Pinned::None);

    let html = render(
        Setup::builder()
            .pins(pins(&[("age", Pinned::Start)]))
            .state(Some(state))
            .build(),
    );

    assert_eq!(header_order(&html), ["Name", "City", "Age"]);
    assert!(!html.contains("data-pinned"), "{html}");
}

#[test]
fn aria_colindex_counts_the_pinned_column_where_it_is_shown() {
    let html = render(
        Setup::builder()
            .pins(pins(&[("age", Pinned::Start)]))
            .build(),
    );

    // Age is shown first, so it is colindex 1 — the keyboard walks the columns
    // in the order they appear, not the order they were declared.
    let age_header = html
        .split("<div ")
        .find(|tag| tag.contains(r#"aria-label="Age""#))
        .unwrap();
    assert!(age_header.contains(r#"aria-colindex="1""#), "{html}");
}

#[test]
fn the_cells_are_pinned_along_with_their_header() {
    let html = render(
        Setup::builder()
            .pins(pins(&[("name", Pinned::Start)]))
            .build(),
    );

    // Once for the header, once for the one row: a pinned column that only
    // sticks in its header would tear as the grid scrolls.
    assert_eq!(html.matches(r#"data-pinned="start""#).count(), 2, "{html}");
}
