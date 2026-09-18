//! Unit tests for column filters, global search and paging.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{ColumnSpec, GridState, PageState, compute_view};

mod common;
use common::{User, ids_of, names_of, sample_columns, sample_rows};

#[test]
fn no_filters_keeps_every_row_in_input_order() {
    let rows = sample_rows();
    let view = compute_view(&rows, &sample_columns(), &GridState::new());

    assert_eq!(view.filtered_len, 5);
    assert_eq!(view.indices, [0, 1, 2, 3, 4]);
    assert_eq!(view.page_count, 0, "paging is disabled");
}

#[test]
fn column_filter_matches_case_insensitively_on_substrings() {
    let rows = sample_rows();
    let mut state = GridState::new();
    state.set_filter("name", "o");

    let view = compute_view(&rows, &sample_columns(), &state);
    // Zoe, Carol and bob all contain an "o" in some case.
    assert_eq!(names_of(&rows, &view), ["Zoe", "Carol", "bob"]);
}

#[test]
fn column_filters_combine_with_and() {
    let rows = sample_rows();
    let mut state = GridState::new();
    state.set_filter("name", "a");
    state.set_filter("email", "carol");

    let view = compute_view(&rows, &sample_columns(), &state);
    assert_eq!(names_of(&rows, &view), ["Carol"]);
}

#[test]
fn an_empty_filter_string_removes_the_filter() {
    let rows = sample_rows();
    let mut state = GridState::new();

    state.set_filter("name", "zzz");
    assert_eq!(
        compute_view(&rows, &sample_columns(), &state).filtered_len,
        0
    );

    state.set_filter("name", "");
    assert_eq!(
        compute_view(&rows, &sample_columns(), &state).filtered_len,
        5
    );
    assert!(state.filter(&"name".into()).is_none());
}

#[test]
fn filters_on_unknown_columns_are_ignored() {
    let rows = sample_rows();
    let mut state = GridState::new();
    // Persisted state can outlive the column it referred to.
    state.set_filter("column-that-was-removed", "anything");

    let view = compute_view(&rows, &sample_columns(), &state);
    assert_eq!(view.filtered_len, 5);
}

#[test]
fn filters_on_unfilterable_columns_are_ignored() {
    let rows = sample_rows();
    // Neither a value nor filter text: nothing to filter by.
    let mut columns = sample_columns();
    columns.push(ColumnSpec::new("decoration"));
    let mut state = GridState::new();
    state.set_filter("decoration", "anything");

    let view = compute_view(&rows, &columns, &state);
    assert_eq!(view.filtered_len, 5);
}

/// Since 0.6.0 a column with a value is filterable without filter text, and
/// plain text in the filter bar of a number column means "equals".
#[test]
fn the_bar_filters_a_value_column_by_equality() {
    let rows = sample_rows();
    let mut state = GridState::new();
    state.set_filter("age", "30");

    let view = compute_view(&rows, &sample_columns(), &state);
    assert_eq!(names_of(&rows, &view), ["Zoe", "Mia"]);

    state.set_filter("age", "3");
    assert_eq!(
        compute_view(&rows, &sample_columns(), &state).filtered_len,
        0
    );
}

#[test]
fn search_spans_every_filterable_column() {
    let rows = sample_rows();
    let mut state = GridState::new();
    // Matches nobody by name, but "mia@example.com" by email.
    state.set_search("mia@");

    let view = compute_view(&rows, &sample_columns(), &state);
    assert_eq!(names_of(&rows, &view), ["Mia"]);
}

#[test]
fn search_skips_hidden_columns() {
    let rows = sample_rows();
    let mut state = GridState::new();
    state.set_search("example.com");
    assert_eq!(
        compute_view(&rows, &sample_columns(), &state).filtered_len,
        5,
        "every email matches while the column is visible"
    );

    // Hiding the only column that matches must hide the rows too — a hit the
    // user cannot see is worse than no hit.
    state.set_column_hidden("email", true);
    assert_eq!(
        compute_view(&rows, &sample_columns(), &state).filtered_len,
        0
    );
}

#[test]
fn column_filters_still_apply_to_hidden_columns() {
    let rows = sample_rows();
    let mut state = GridState::new();
    state.set_filter("email", "carol");
    state.set_column_hidden("email", true);

    // The user set this filter deliberately; hiding the column must not
    // silently widen the result.
    let view = compute_view(&rows, &sample_columns(), &state);
    assert_eq!(names_of(&rows, &view), ["Carol"]);
}

#[test]
fn search_is_ignored_when_no_column_is_searchable() {
    let rows = sample_rows();
    let columns = vec![ColumnSpec::new("age").sort_by_value(|user: &User| user.age)];
    let mut state = GridState::new();
    state.set_search("anything");

    // Hiding every row would look broken; ignoring an unusable term does not.
    let view = compute_view(&rows, &columns, &state);
    assert_eq!(view.filtered_len, 5);
}

#[test]
fn search_and_column_filters_both_have_to_match() {
    let rows = sample_rows();
    let mut state = GridState::new();
    state.set_filter("name", "o");
    state.set_search("bob");

    let view = compute_view(&rows, &sample_columns(), &state);
    assert_eq!(names_of(&rows, &view), ["bob"]);
}

#[test]
fn non_ascii_text_matches_case_insensitively() {
    let rows = vec![
        User::new(1, "Ärger", 30),
        User::new(2, "Zoe", 31),
        User::new(3, "straße", 32),
    ];
    let columns = vec![ColumnSpec::new("name").filter_by(|user: &User| user.name.clone())];

    let mut state = GridState::new();
    state.set_filter("name", "ÄRG");
    assert_eq!(
        names_of(&rows, &compute_view(&rows, &columns, &state)),
        ["Ärger"]
    );

    state.set_filter("name", "STRAßE");
    assert_eq!(
        names_of(&rows, &compute_view(&rows, &columns, &state)),
        ["straße"]
    );
}

#[test]
fn paging_slices_the_filtered_rows() {
    let rows: Vec<User> = (0..10).map(|i| User::new(i, &format!("u{i}"), i)).collect();
    let columns = sample_columns();
    let mut state = GridState::paged(3);

    let first = compute_view(&rows, &columns, &state);
    assert_eq!(ids_of(&rows, &first), [0, 1, 2]);
    assert_eq!(first.filtered_len, 10, "filtered_len spans every page");
    assert_eq!(first.page_count, 4, "10 rows in pages of 3");

    state.set_page(1);
    assert_eq!(
        ids_of(&rows, &compute_view(&rows, &columns, &state)),
        [3, 4, 5]
    );

    // The last page is partial.
    state.set_page(3);
    assert_eq!(ids_of(&rows, &compute_view(&rows, &columns, &state)), [9]);
}

#[test]
fn a_page_index_past_the_end_is_clamped_to_the_last_page() {
    let rows: Vec<User> = (0..10).map(|i| User::new(i, &format!("u{i}"), i)).collect();
    let mut state = GridState::paged(3);
    state.set_page(999);

    // A stale index should show the last page, not an empty grid.
    let view = compute_view(&rows, &sample_columns(), &state);
    assert_eq!(ids_of(&rows, &view), [9]);
}

#[test]
fn a_page_size_of_zero_disables_paging_instead_of_dividing_by_zero() {
    let rows = sample_rows();
    let state = GridState {
        page: Some(PageState { index: 2, size: 0 }),
        ..GridState::new()
    };

    let view = compute_view(&rows, &sample_columns(), &state);
    assert_eq!(view.filtered_len, 5);
    assert_eq!(view.len(), 5);
    assert_eq!(view.page_count, 0);
}

#[test]
fn filtering_to_nothing_yields_an_empty_view() {
    let rows = sample_rows();
    let mut state = GridState::paged(3);
    state.set_filter("name", "no-such-user");

    let view = compute_view(&rows, &sample_columns(), &state);
    assert!(view.is_empty());
    assert_eq!(view.filtered_len, 0);
    assert_eq!(view.page_count, 0);
}

#[test]
fn an_empty_row_set_is_handled() {
    let rows: Vec<User> = Vec::new();
    let mut state = GridState::paged(25);
    state.set_search("anything");

    let view = compute_view(&rows, &sample_columns(), &state);
    assert!(view.is_empty());
    assert_eq!(view.page_count, 0);
}

#[test]
fn changing_a_filter_returns_to_the_first_page() {
    let mut state = GridState::paged(3);
    state.set_page(2);
    assert_eq!(state.page.map(|page| page.index), Some(2));

    // Staying on page 3 of a result set that just shrank would look empty.
    state.set_filter("name", "a");
    assert_eq!(state.page.map(|page| page.index), Some(0));

    state.set_page(2);
    state.set_search("b");
    assert_eq!(state.page.map(|page| page.index), Some(0));

    state.set_page(2);
    state.toggle_sort("name", false);
    assert_eq!(state.page.map(|page| page.index), Some(0));
}

#[test]
fn filter_sort_page_compose_in_that_order() {
    let rows = sample_rows();
    let mut state = GridState::paged(2);
    // Matches adam, Carol, Mia (a in name) — then sorted by name, then paged.
    state.set_filter("name", "a");
    state.toggle_sort("name", false);

    let first = compute_view(&rows, &sample_columns(), &state);
    assert_eq!(first.filtered_len, 3);
    assert_eq!(first.page_count, 2);
    assert_eq!(names_of(&rows, &first), ["adam", "Carol"]);

    state.set_page(1);
    let second = compute_view(&rows, &sample_columns(), &state);
    assert_eq!(names_of(&rows, &second), ["Mia"]);
}
