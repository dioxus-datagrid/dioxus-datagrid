//! Unit tests for [`GridState`]'s mutation helpers.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{GridState, SortDirection};

#[test]
fn toggle_sort_cycles_ascending_descending_unsorted() {
    let mut state = GridState::new();
    let name = "name".into();

    state.toggle_sort("name", false);
    assert_eq!(state.sort_direction(&name), Some(SortDirection::Asc));

    state.toggle_sort("name", false);
    assert_eq!(state.sort_direction(&name), Some(SortDirection::Desc));

    state.toggle_sort("name", false);
    assert_eq!(state.sort_direction(&name), None);
    assert!(state.sort.is_empty());
}

#[test]
fn a_non_additive_toggle_replaces_the_whole_sort() {
    let mut state = GridState::new();
    state.toggle_sort("name", true);
    state.toggle_sort("age", true);
    assert_eq!(state.sort.len(), 2);

    state.toggle_sort("email", false);
    assert_eq!(state.sort.len(), 1);
    assert_eq!(state.sort_priority(&"email".into()), Some(0));
}

#[test]
fn an_additive_toggle_appends_a_new_column() {
    let mut state = GridState::new();
    state.toggle_sort("name", true);
    state.toggle_sort("age", true);

    assert_eq!(state.sort_priority(&"name".into()), Some(0));
    assert_eq!(state.sort_priority(&"age".into()), Some(1));
}

#[test]
fn an_additive_toggle_keeps_a_columns_priority_when_reversing_it() {
    let mut state = GridState::new();
    state.toggle_sort("name", true);
    state.toggle_sort("age", true);

    // Reversing the primary column must not demote it behind "age".
    state.toggle_sort("name", true);

    assert_eq!(
        state.sort_direction(&"name".into()),
        Some(SortDirection::Desc)
    );
    assert_eq!(state.sort_priority(&"name".into()), Some(0));
    assert_eq!(state.sort_priority(&"age".into()), Some(1));
}

#[test]
fn an_additive_toggle_removes_only_that_column() {
    let mut state = GridState::new();
    state.toggle_sort("name", true);
    state.toggle_sort("age", true);

    state.toggle_sort("name", true); // -> Desc
    state.toggle_sort("name", true); // -> removed

    assert_eq!(state.sort.len(), 1);
    assert_eq!(state.sort_priority(&"age".into()), Some(0));
}

#[test]
fn set_filter_replaces_an_existing_filter_for_the_same_column() {
    let mut state = GridState::new();
    state.set_filter("name", "a");
    state.set_filter("name", "b");

    assert_eq!(state.column_filters.len(), 1);
    assert_eq!(state.filter(&"name".into()), Some("b"));
}

#[test]
fn set_search_clears_on_an_empty_string() {
    let mut state = GridState::new();
    state.set_search("hello");
    assert_eq!(state.search.as_deref(), Some("hello"));

    state.set_search("");
    assert!(state.search.is_none());
}

#[test]
fn hiding_a_column_is_idempotent() {
    let mut state = GridState::new();
    state.set_column_hidden("email", true);
    state.set_column_hidden("email", true);
    assert_eq!(state.hidden_columns.len(), 1);

    state.set_column_hidden("email", false);
    assert!(state.hidden_columns.is_empty());
}

#[test]
fn column_widths_are_replaced_not_appended() {
    let mut state = GridState::new();
    state.set_column_width("name", 120.0);
    state.set_column_width("name", 200.0);

    assert_eq!(state.column_widths.len(), 1);
    assert_eq!(state.column_width(&"name".into()), Some(200.0));
}

#[test]
fn set_page_size_turns_paging_on_off_and_resets_the_page() {
    let mut state = GridState::new();

    state.set_page_size(Some(10));
    assert_eq!(state.page.map(|page| page.size), Some(10));

    state.set_page(3);
    // A different size makes the old index meaningless.
    state.set_page_size(Some(25));
    assert_eq!(
        state.page.map(|page| (page.index, page.size)),
        Some((0, 25))
    );

    // The same size is not a change and keeps the current page.
    state.set_page(2);
    state.set_page_size(Some(25));
    assert_eq!(state.page.map(|page| page.index), Some(2));

    state.set_page_size(None);
    assert!(state.page.is_none());
}

#[test]
fn set_page_on_unpaged_state_is_a_no_op() {
    let mut state = GridState::new();
    state.set_page(5);
    assert!(state.page.is_none());
}

#[cfg(feature = "serde")]
#[test]
fn state_round_trips_through_serde() {
    let mut state = GridState::paged(25);
    state.toggle_sort("name", true);
    state.toggle_sort("age", true);
    state.set_filter("email", "example.com");
    state.set_search("needle");
    state.set_column_hidden("age", true);
    state.set_column_width("name", 180.5);
    state.set_page(2);

    let json = serde_json::to_string(&state).unwrap();
    let restored: GridState = serde_json::from_str(&json).unwrap();

    assert_eq!(state, restored);
}

#[cfg(feature = "serde")]
#[test]
fn missing_fields_deserialize_to_defaults() {
    // Forward compatibility: state written by an older version must still load.
    let restored: GridState = serde_json::from_str("{}").unwrap();
    assert_eq!(restored, GridState::new());
}
