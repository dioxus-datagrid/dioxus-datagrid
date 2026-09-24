//! The column menu: which entries a column offers, what they are called, and
//! what the trigger tells assistive technology.
//!
//! The entries are asserted through [`column_menu_entries`] rather than through
//! an opened panel: a server render cannot click a button, and the list is the
//! part with the decisions in it.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{ColumnId, GridLocale, GridRow, GridState, Pinned, SortDirection};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{GridColumnMenu, GridHeader, GridRoot, column_menu_entries};
use dioxus_datagrid::{Column, GridOptions, use_grid};

#[derive(Clone, PartialEq)]
struct User {
    id: u32,
    name: String,
    note: String,
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
        note: "hello".to_owned(),
        age: 30,
    }]
}

fn columns() -> Vec<Column<User>> {
    vec![
        Column::new("name", "Name").value_text(|user: &User| user.name.as_str()),
        // No value: nothing to sort by and nothing to group by.
        Column::new("note", "Note").cell(|user: &User| rsx! { "{user.note}" }),
        Column::new("age", "Age").value_of(|user: &User| user.age),
    ]
}

#[derive(Clone, PartialEq, Props)]
struct Setup {
    #[props(default)]
    column_index: usize,
    #[props(default)]
    state: Option<GridState>,
    #[props(default)]
    german: bool,
}

/// Renders the menu's trigger, and the entries it would show as `data-entry`
/// attributes, so a server render can assert on both.
#[component]
fn Grid(setup: Setup) -> Element {
    let rows = use_signal(users);
    let cols = use_hook(columns);

    let mut options = GridOptions::default();
    if let Some(state) = setup.state.clone() {
        options = options.initial_state(state);
    }
    if setup.german {
        options = options.locale(GridLocale::german());
    }
    let grid = use_grid(rows, cols, options);
    let entries = column_menu_entries(&grid, setup.column_index);

    rsx! {
        GridRoot { grid,
            GridHeader { grid }
            GridColumnMenu { grid, column_index: setup.column_index }
        }
        div { id: "entries",
            for entry in entries {
                span {
                    key: "{entry.label}",
                    "data-entry": "{entry.action.as_str()}",
                    "data-checked": entry.checked.map(|on| on.to_string()),
                    "{entry.label}"
                }
            }
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

/// The entries, by what they do, in order.
fn entries(html: &str) -> Vec<String> {
    html.split(r#"data-entry=""#)
        .skip(1)
        .filter_map(|part| part.split('"').next())
        .map(str::to_owned)
        .collect()
}

fn sorted_state(direction: SortDirection) -> GridState {
    let mut state = GridState::new();
    state.set_sort("name", direction);
    state
}

#[test]
fn a_plain_column_offers_sorting_grouping_pinning_and_hiding() {
    let html = render(Setup::builder().build());

    assert_eq!(
        entries(&html),
        [
            "sort-asc",
            "sort-desc",
            "group",
            "pin-start",
            "pin-end",
            "hide"
        ]
    );
}

#[test]
fn a_column_without_a_value_offers_neither_sorting_nor_grouping() {
    let html = render(Setup::builder().column_index(1).build());

    assert_eq!(entries(&html), ["pin-start", "pin-end", "hide"]);
}

#[test]
fn clearing_the_sort_is_offered_only_once_there_is_one() {
    let plain = render(Setup::builder().build());
    assert!(!entries(&plain).contains(&"sort-clear".to_owned()));

    let sorted = render(
        Setup::builder()
            .state(Some(sorted_state(SortDirection::Asc)))
            .build(),
    );
    assert!(entries(&sorted).contains(&"sort-clear".to_owned()));
}

#[test]
fn the_direction_a_column_is_sorted_in_is_the_one_that_holds() {
    let html = render(
        Setup::builder()
            .state(Some(sorted_state(SortDirection::Desc)))
            .build(),
    );

    // The entries of a choice say which one holds, so a screen reader can
    // announce it: descending is set, ascending is not.
    let desc = html.split(r#"data-entry="sort-desc""#).nth(1).unwrap();
    assert!(desc.starts_with(r#" data-checked="true""#), "{html}");
    let asc = html.split(r#"data-entry="sort-asc""#).nth(1).unwrap();
    assert!(asc.starts_with(r#" data-checked="false""#), "{html}");
}

#[test]
fn unpinning_is_offered_only_for_a_pinned_column() {
    let plain = render(Setup::builder().build());
    assert!(!entries(&plain).contains(&"unpin".to_owned()));

    let mut state = GridState::new();
    state.set_pinned(ColumnId::new("name"), Pinned::Start);
    let pinned = render(Setup::builder().state(Some(state)).build());

    let list = entries(&pinned);
    assert!(list.contains(&"unpin".to_owned()), "{list:?}");
    let start = pinned.split(r#"data-entry="pin-start""#).nth(1).unwrap();
    assert!(start.starts_with(r#" data-checked="true""#), "{pinned}");
}

#[test]
fn fitting_the_width_is_offered_only_after_a_resize() {
    let plain = render(Setup::builder().build());
    assert!(!entries(&plain).contains(&"fit-width".to_owned()));

    let mut state = GridState::new();
    state.set_column_width("name", 200.0);
    let resized = render(Setup::builder().state(Some(state)).build());
    assert!(entries(&resized).contains(&"fit-width".to_owned()));
}

#[test]
fn a_grouped_column_offers_to_stop_grouping() {
    let mut state = GridState::new();
    state.group_by_column("name", None);
    let html = render(Setup::builder().state(Some(state)).build());

    let list = entries(&html);
    assert!(list.contains(&"ungroup".to_owned()), "{list:?}");
    assert!(!list.contains(&"group".to_owned()), "{list:?}");
}

#[test]
fn the_last_visible_column_cannot_be_hidden_from_the_menu() {
    let mut state = GridState::new();
    state.set_column_hidden("note", true);
    state.set_column_hidden("age", true);
    let html = render(Setup::builder().state(Some(state)).build());

    assert!(!entries(&html).contains(&"hide".to_owned()), "{html}");
}

#[test]
fn the_trigger_says_what_it_opens() {
    let html = render(Setup::builder().build());

    assert!(html.contains(r#"aria-haspopup="menu""#), "{html}");
    assert!(html.contains(r#"aria-expanded="false""#), "{html}");
    assert!(
        html.contains(r#"aria-label="Column options for Name""#),
        "{html}"
    );
}

#[test]
fn every_text_comes_from_the_locale() {
    let html = render(Setup::builder().german(true).build());

    assert!(
        html.contains(r#"aria-label="Spaltenoptionen für Name""#),
        "{html}"
    );
    assert!(html.contains("Aufsteigend sortieren"), "{html}");
    assert!(html.contains("Am Anfang fixieren"), "{html}");
    assert!(html.contains("Spalte ausblenden"), "{html}");
}

#[test]
fn a_column_that_is_gone_renders_no_menu() {
    let html = render(Setup::builder().column_index(9).build());

    assert!(!html.contains("data-column-menu"), "{html}");
    assert!(entries(&html).is_empty(), "{html}");
}
