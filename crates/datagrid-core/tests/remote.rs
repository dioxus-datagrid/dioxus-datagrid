//! Server-side data: queries derived from state, debounce classification, and
//! stale responses.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{
    DEFAULT_REMOTE_PAGE_SIZE, GridQuery, GridState, Page, RequestTracker, SortDirection,
};

#[test]
fn a_query_carries_sort_filters_search_and_page() {
    let mut state = GridState::paged(20);
    state.toggle_sort("name", false);
    state.set_filter("email", "example");
    state.set_search("ada");
    state.set_page(3);

    let query = GridQuery::from_state(&state, 50);
    assert_eq!(query.sort.len(), 1);
    assert_eq!(query.sort[0].direction, SortDirection::Asc);
    assert_eq!(
        query.column_filters,
        vec![("email".into(), "example".to_owned())]
    );
    assert_eq!(query.search.as_deref(), Some("ada"));
    assert_eq!((query.page, query.page_size), (3, 20));
    assert_eq!(query.offset(), 60);
}

#[test]
fn unpaged_state_uses_the_default_page_size() {
    let query = GridQuery::from_state(&GridState::new(), 40);
    assert_eq!((query.page, query.page_size), (0, 40));
}

#[test]
fn a_zero_page_size_falls_back_to_the_remote_default() {
    assert_eq!(
        GridQuery::from_state(&GridState::new(), 0).page_size,
        DEFAULT_REMOTE_PAGE_SIZE
    );
    assert_eq!(
        GridQuery::from_state(&GridState::paged(0), 10).page_size,
        DEFAULT_REMOTE_PAGE_SIZE
    );
}

#[test]
fn empty_text_does_not_make_queries_differ() {
    let mut state = GridState::paged(10);
    state.column_filters.push(("name".into(), String::new()));
    state.search = Some(String::new());

    assert_eq!(
        GridQuery::from_state(&state, 10),
        GridQuery::from_state(&GridState::paged(10), 10)
    );
}

#[test]
fn typing_in_search_or_filters_is_a_typing_change() {
    let before = GridQuery::from_state(&GridState::paged(10), 10);

    let mut searched = GridState::paged(10);
    searched.set_search("a");
    assert!(GridQuery::from_state(&searched, 10).is_typing_change(&before));

    let mut filtered = GridState::paged(10);
    filtered.set_filter("name", "a");
    assert!(GridQuery::from_state(&filtered, 10).is_typing_change(&before));
}

#[test]
fn sorting_or_paging_loads_at_once() {
    let before = GridQuery::from_state(&GridState::paged(10), 10);

    let mut sorted = GridState::paged(10);
    sorted.toggle_sort("name", false);
    assert!(!GridQuery::from_state(&sorted, 10).is_typing_change(&before));

    let mut paged = GridState::paged(10);
    paged.set_page(2);
    assert!(!GridQuery::from_state(&paged, 10).is_typing_change(&before));

    // A sort that also changed the text is still a deliberate action.
    let mut both = GridState::paged(10);
    both.set_search("a");
    both.toggle_sort("name", false);
    assert!(!GridQuery::from_state(&both, 10).is_typing_change(&before));

    // And an identical query is no change at all.
    assert!(!before.is_typing_change(&before));
}

#[test]
fn a_page_counts_its_pages() {
    let page = Page::new(vec![1, 2, 3], 101);
    assert_eq!(page.page_count(25), 5);
    assert_eq!(page.page_count(0), 0);
    assert_eq!(Page::<u8>::new(Vec::new(), 0).page_count(25), 0);
}

/// Phase 6 acceptance, at the level of the decision itself: a slower response
/// to an older request arrives after the newer one and must be discarded.
#[test]
fn a_late_response_to_an_older_request_is_stale() {
    let mut tracker = RequestTracker::new();

    let slow = tracker.begin(); // search "a", slow
    let fast = tracker.begin(); // search "ab", fast

    // "ab" answers first and is applied.
    assert!(tracker.is_latest(fast));
    // "a" answers afterwards and must not overwrite it.
    assert!(!tracker.is_latest(slow));
}

#[test]
fn only_the_most_recent_request_is_latest() {
    let mut tracker = RequestTracker::new();
    let ids: Vec<_> = (0..5).map(|_| tracker.begin()).collect();
    for (position, id) in ids.iter().enumerate() {
        assert_eq!(tracker.is_latest(*id), position == ids.len() - 1);
    }
}

#[cfg(feature = "serde")]
#[test]
fn a_query_round_trips_through_serde() {
    let mut state = GridState::paged(20);
    state.toggle_sort("name", true);
    state.set_filter("email", "x");
    state.set_search("y");
    let query = GridQuery::from_state(&state, 20);

    let json = serde_json::to_string(&query).unwrap();
    assert_eq!(serde_json::from_str::<GridQuery>(&json).unwrap(), query);
}
