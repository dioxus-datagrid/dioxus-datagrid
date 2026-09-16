//! Server-rendered checks on the ARIA grid pattern.
//!
//! These render real component trees with `dioxus-ssr` and assert on the markup,
//! which is the only way to verify attributes that only exist once rendered —
//! `aria-rowindex` across pages, `aria-sort` per state, and the roving tabindex.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{GridRow, GridState, SelectionMode, SortState};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{
    GridBody, GridHeader, GridPagination, GridRoot, VirtualGridBody,
};
use dioxus_datagrid::{Column, GridOptions, use_grid};

#[derive(Clone, PartialEq)]
struct User {
    id: u32,
    name: String,
    age: u32,
}

impl GridRow for User {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

fn users() -> Vec<User> {
    [
        (1, "Zoe", 30),
        (2, "adam", 25),
        (3, "Mia", 30),
        (4, "Carol", 41),
        (5, "bob", 41),
    ]
    .into_iter()
    .map(|(id, name, age)| User {
        id,
        name: name.to_owned(),
        age,
    })
    .collect()
}

fn columns() -> Vec<Column<User>> {
    vec![
        Column::new("name", "Name")
            .cell(|user: &User| rsx! { "{user.name}" })
            .sort_by_text(|user: &User| user.name.as_str())
            .filter_by(|user: &User| user.name.clone()),
        Column::new("age", "Age")
            .cell(|user: &User| rsx! { "{user.age}" })
            .sort_by_value(|user: &User| user.age),
    ]
}

/// What a test wants the grid configured with, threaded through as props
/// because `use_grid` owns its state.
#[derive(Clone, PartialEq, Props)]
struct Setup {
    #[props(default)]
    state: Option<GridState>,
    #[props(default)]
    mode: SelectionMode,
    /// Keys to select once, during the first render.
    #[props(default)]
    select: Vec<u32>,
}

#[component]
fn Grid(setup: Setup) -> Element {
    let rows = use_signal(users);
    let cols = use_hook(columns);

    let mut options = GridOptions::default().selection(setup.mode);
    if let Some(state) = setup.state.clone() {
        options = options.initial_state(state);
    }

    let grid = use_grid(rows, cols, options);

    // Selection is interaction state, so there is no "initial selection"
    // option; a one-shot hook is how an app would do it too.
    use_hook(move || {
        let mut grid = grid;
        for key in setup.select.clone() {
            grid.select(key);
        }
    });

    rsx! {
        GridRoot { grid,
            GridHeader { grid }
            GridBody { grid }
        }
        GridPagination { grid }
    }
}

/// Renders a configured grid to HTML.
fn render(setup: Setup) -> String {
    #[component]
    fn Harness(setup: Setup) -> Element {
        rsx! { Grid { setup } }
    }

    let mut dom = VirtualDom::new_with_props(Harness, HarnessProps { setup });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

/// Counts non-overlapping occurrences of `needle`.
fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

#[test]
fn root_carries_grid_role_and_counts() {
    let html = render(Setup::builder().build());

    assert!(html.contains(r#"role="grid""#));
    // Five rows plus the header row.
    assert!(
        html.contains(r#"aria-rowcount="6""#),
        "aria-rowcount must include the header row: {html}"
    );
    assert!(html.contains(r#"aria-colcount="2""#));
}

#[test]
fn a_sortable_but_unsorted_column_reports_aria_sort_none() {
    let html = render(Setup::builder().build());

    // "none" rather than no attribute at all: it is what tells assistive
    // technology that this column *can* be sorted.
    assert_eq!(count(&html, r#"aria-sort="none""#), 2);
    assert!(!html.contains(r#"aria-sort="ascending""#));
}

#[test]
fn the_sorted_column_reports_its_direction() {
    let ascending = render(
        Setup::builder()
            .state(Some(GridState {
                sort: vec![SortState::asc("name")],
                ..GridState::new()
            }))
            .build(),
    );
    assert!(ascending.contains(r#"aria-sort="ascending""#));
    // The other column stays "none".
    assert_eq!(count(&ascending, r#"aria-sort="none""#), 1);

    let descending = render(
        Setup::builder()
            .state(Some(GridState {
                sort: vec![SortState::desc("name")],
                ..GridState::new()
            }))
            .build(),
    );
    assert!(descending.contains(r#"aria-sort="descending""#));
}

#[test]
fn an_unsortable_column_has_no_aria_sort() {
    #[component]
    fn Unsortable() -> Element {
        let rows = use_signal(users);
        let cols = use_hook(|| {
            vec![Column::new("name", "Name").cell(|user: &User| rsx! { "{user.name}" })]
        });
        let grid = use_grid(rows, cols, GridOptions::default());
        rsx! {
            GridRoot { grid,
                GridHeader { grid }
            }
        }
    }

    let mut dom = VirtualDom::new(Unsortable);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);

    assert!(html.contains(r#"role="columnheader""#));
    assert!(!html.contains("aria-sort"));
}

#[test]
fn sorting_reorders_the_rendered_rows() {
    let html = render(
        Setup::builder()
            .state(Some(GridState {
                sort: vec![SortState::asc("name")],
                ..GridState::new()
            }))
            .build(),
    );

    let adam = html.find("adam").unwrap();
    let zoe = html.find("Zoe").unwrap();
    assert!(adam < zoe, "case-insensitive sort should put adam first");
}

#[test]
fn rows_report_a_one_based_rowindex_that_counts_the_header() {
    let html = render(Setup::builder().build());

    // Header is row 1, so the first body row is row 2.
    assert!(html.contains(r#"aria-rowindex="1""#));
    assert!(html.contains(r#"aria-rowindex="2""#));
    assert!(html.contains(r#"aria-rowindex="6""#));
    assert!(!html.contains(r#"aria-rowindex="7""#));
}

#[test]
fn rowindex_continues_across_pages() {
    let mut state = GridState::paged(2);
    state.set_page(1);
    let html = render(Setup::builder().state(Some(state)).build());

    // Page 2 of a 2-row page holds the third and fourth rows, which are
    // rowindex 4 and 5 once the header is counted.
    assert!(
        html.contains(r#"aria-rowindex="4""#),
        "page 2 should continue the row numbering: {html}"
    );
    assert!(html.contains(r#"aria-rowindex="5""#));
    assert!(!html.contains(r#"aria-rowindex="2""#));

    // aria-rowcount still spans every page.
    assert!(html.contains(r#"aria-rowcount="6""#));
}

#[test]
fn cells_report_a_one_based_colindex() {
    let html = render(Setup::builder().build());

    // Two columns, six rows including the header.
    assert_eq!(count(&html, r#"aria-colindex="1""#), 6);
    assert_eq!(count(&html, r#"aria-colindex="2""#), 6);
    assert!(!html.contains(r#"aria-colindex="3""#));
}

#[test]
fn selection_is_reported_only_when_selection_is_enabled() {
    let off = render(Setup::builder().build());
    assert!(
        !off.contains("aria-selected"),
        "rows must not claim to be selectable when they are not"
    );
    assert!(!off.contains("aria-multiselectable"));

    let on = render(
        Setup::builder()
            .mode(SelectionMode::Multi)
            .select(vec![3])
            .build(),
    );
    assert!(on.contains(r#"aria-multiselectable="true""#));
    assert_eq!(count(&on, r#"aria-selected="true""#), 1);
    assert_eq!(count(&on, r#"aria-selected="false""#), 4);
}

#[test]
fn single_selection_is_not_advertised_as_multiselectable() {
    let html = render(
        Setup::builder()
            .mode(SelectionMode::Single)
            .select(vec![1])
            .build(),
    );

    assert!(!html.contains("aria-multiselectable"));
    assert_eq!(count(&html, r#"aria-selected="true""#), 1);
}

#[test]
fn exactly_one_cell_is_tabbable() {
    let html = render(Setup::builder().build());

    // The roving tabindex: one entry point for Tab, everything else reachable
    // only with the arrow keys.
    assert_eq!(
        count(&html, r#"tabindex="0""#),
        1,
        "exactly one element may be tabbable: {html}"
    );
    // Two header cells and ten body cells, minus the one that is tabbable, plus
    // the root, which is focusable from script but not a tab stop.
    assert_eq!(count(&html, r#"tabindex="-1""#), 12);
}

#[test]
fn the_root_is_not_a_tab_stop_while_the_focused_cell_is_rendered() {
    let html = render(Setup::builder().build());

    let root_start = html.find(r#"role="grid""#).unwrap();
    let root_tag_end = root_start + html[root_start..].find('>').unwrap();
    let root_tag = &html[root_start..root_tag_end];
    assert!(
        root_tag.contains(r#"tabindex="-1""#),
        "a second tab stop on the root would make Tab land on the grid twice: {root_tag}"
    );
}

#[test]
fn the_tabbable_cell_follows_the_focus() {
    // Focus starts on the header, so the first header cell is the entry point.
    let html = render(Setup::builder().build());
    let first_tabbable = html.find(r#"tabindex="0""#).unwrap();
    let first_header = html.find(r#"role="columnheader""#).unwrap();
    let second_header = html[first_header + 1..]
        .find(r#"role="columnheader""#)
        .unwrap()
        + first_header
        + 1;

    assert!(
        first_tabbable > first_header && first_tabbable < second_header,
        "the first header cell should hold the tabindex: {html}"
    );
}

#[test]
fn pagination_renders_only_when_paging_is_on() {
    let unpaged = render(Setup::builder().build());
    assert!(!unpaged.contains(r#"aria-label="Pagination""#));

    let paged = render(Setup::builder().state(Some(GridState::paged(2))).build());
    assert!(paged.contains(r#"aria-label="Pagination""#));
    assert!(paged.contains(r#"data-page-count="3""#));
    assert!(paged.contains("Page 1 of 3"));
}

#[test]
fn only_the_current_page_is_rendered() {
    let html = render(Setup::builder().state(Some(GridState::paged(2))).build());

    assert_eq!(count(&html, r#"role="row""#), 3, "header plus two rows");
}

#[test]
fn filtering_narrows_the_rendered_rows_and_the_rowcount() {
    let mut state = GridState::new();
    state.set_filter("name", "a");
    let html = render(Setup::builder().state(Some(state)).build());

    // adam, Carol and Mia contain an "a".
    assert_eq!(count(&html, r#"role="row""#), 4, "header plus three rows");
    assert!(html.contains(r#"aria-rowcount="4""#));
}

#[test]
fn a_virtual_body_renders_a_window_and_pads_for_the_rest() {
    #[component]
    fn Virtual() -> Element {
        let rows = use_signal(|| {
            (1..=100)
                .map(|id| User {
                    id,
                    name: format!("user {id}"),
                    age: 20 + id % 50,
                })
                .collect::<Vec<_>>()
        });
        let cols = use_hook(columns);
        let grid = use_grid(rows, cols, GridOptions::default());
        rsx! {
            GridRoot { grid,
                GridHeader { grid }
                VirtualGridBody { grid, row_height: 30.0, overscan: 4 }
            }
        }
    }

    let mut dom = VirtualDom::new(Virtual);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);

    // Server-side there is no viewport, so only the overscan is rendered — but
    // the grid still describes every row.
    assert_eq!(
        count(&html, r#"role="row""#),
        1 + 4,
        "header plus overscan: {html}"
    );
    assert!(html.contains(r#"aria-rowcount="101""#));
    assert!(html.contains(r#"aria-rowindex="2""#));
    assert!(html.contains(r#"aria-rowindex="5""#));
    assert!(!html.contains(r#"aria-rowindex="6""#));

    // The 96 rows not rendered are accounted for below the window, so the
    // scrollbar reflects all 100.
    assert!(
        html.contains("padding-top: 0px; padding-bottom: 2880px;"),
        "{html}"
    );
    assert!(html.contains("height: 30px;"));
}
