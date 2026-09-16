//! Unit tests for [`Selection`].

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{Selection, SelectionMode};
use std::collections::HashSet;

/// The selected keys, sorted so assertions do not depend on hash order.
fn selected(selection: &Selection<u32>) -> Vec<u32> {
    let mut keys: Vec<u32> = selection.iter().copied().collect();
    keys.sort_unstable();
    keys
}

#[test]
fn selection_mode_none_ignores_every_mutation() {
    let mut selection = Selection::new();

    selection.select(1, SelectionMode::None);
    selection.toggle(2, SelectionMode::None);
    selection.extend_to(&[1, 2, 3], 3, SelectionMode::None);

    assert!(selection.is_empty());
    assert!(selection.anchor().is_none());
}

#[test]
fn single_mode_keeps_at_most_one_row() {
    let mut selection = Selection::new();

    selection.select(1, SelectionMode::Single);
    selection.select(2, SelectionMode::Single);

    assert_eq!(selected(&selection), [2]);
    assert_eq!(selection.len(), 1);
}

#[test]
fn single_mode_toggle_clears_the_current_row() {
    let mut selection = Selection::new();

    selection.toggle(1, SelectionMode::Single);
    assert_eq!(selected(&selection), [1]);

    selection.toggle(1, SelectionMode::Single);
    assert!(selection.is_empty());
    assert!(selection.anchor().is_none());

    // Toggling a different row replaces rather than adds.
    selection.toggle(1, SelectionMode::Single);
    selection.toggle(2, SelectionMode::Single);
    assert_eq!(selected(&selection), [2]);
}

#[test]
fn multi_mode_accumulates_and_toggles() {
    let mut selection = Selection::new();

    selection.select(1, SelectionMode::Multi);
    selection.select(3, SelectionMode::Multi);
    assert_eq!(selected(&selection), [1, 3]);

    selection.toggle(1, SelectionMode::Multi);
    assert_eq!(selected(&selection), [3]);

    selection.toggle(5, SelectionMode::Multi);
    assert_eq!(selected(&selection), [3, 5]);
}

#[test]
fn extend_to_selects_the_inclusive_range_in_display_order() {
    let order = [10, 20, 30, 40, 50];
    let mut selection = Selection::new();

    selection.select(20, SelectionMode::Multi);
    selection.extend_to(&order, 40, SelectionMode::Multi);

    assert_eq!(selected(&selection), [20, 30, 40]);
}

#[test]
fn extend_to_works_backwards() {
    let order = [10, 20, 30, 40, 50];
    let mut selection = Selection::new();

    selection.select(40, SelectionMode::Multi);
    selection.extend_to(&order, 20, SelectionMode::Multi);

    assert_eq!(selected(&selection), [20, 30, 40]);
}

#[test]
fn extend_to_keeps_the_anchor_so_a_range_can_be_redragged() {
    let order = [10, 20, 30, 40, 50];
    let mut selection = Selection::new();

    selection.select(20, SelectionMode::Multi);
    selection.extend_to(&order, 50, SelectionMode::Multi);
    assert_eq!(selection.anchor(), Some(&20));

    // Dragging back from the same anchor extends a shorter range; the earlier
    // rows stay selected because extend_to only ever adds.
    selection.extend_to(&order, 30, SelectionMode::Multi);
    assert_eq!(selection.anchor(), Some(&20));
    assert_eq!(selected(&selection), [20, 30, 40, 50]);
}

#[test]
fn extend_to_without_an_anchor_selects_just_that_row() {
    let order = [10, 20, 30];
    let mut selection = Selection::new();

    selection.extend_to(&order, 30, SelectionMode::Multi);

    assert_eq!(selected(&selection), [30]);
    assert_eq!(selection.anchor(), Some(&30));
}

#[test]
fn extend_to_falls_back_when_an_end_is_not_displayed() {
    let mut selection = Selection::new();
    selection.select(20, SelectionMode::Multi);

    // The anchor is no longer in the view — a range against it would be
    // arbitrary, so only the clicked row is selected.
    selection.extend_to(&[30, 40, 50], 40, SelectionMode::Multi);

    assert_eq!(selected(&selection), [20, 40]);
}

#[test]
fn extend_to_in_single_mode_behaves_like_a_plain_select() {
    let order = [10, 20, 30];
    let mut selection = Selection::new();

    selection.select(10, SelectionMode::Single);
    selection.extend_to(&order, 30, SelectionMode::Single);

    assert_eq!(selected(&selection), [30]);
}

#[test]
fn clear_removes_the_selection_and_the_anchor() {
    let mut selection = Selection::new();
    selection.select(1, SelectionMode::Multi);
    selection.clear();

    assert!(selection.is_empty());
    assert!(selection.anchor().is_none());
}

#[test]
fn retain_existing_drops_rows_that_disappeared() {
    let mut selection = Selection::new();
    selection.select(1, SelectionMode::Multi);
    selection.select(2, SelectionMode::Multi);
    selection.select(3, SelectionMode::Multi);

    let still_there: HashSet<u32> = [1, 3].into_iter().collect();
    selection.retain_existing(&still_there);

    assert_eq!(selected(&selection), [1, 3]);
}

#[test]
fn retain_existing_clears_an_anchor_that_disappeared() {
    let mut selection = Selection::new();
    selection.select(2, SelectionMode::Multi);
    assert_eq!(selection.anchor(), Some(&2));

    selection.retain_existing(&[1, 3].into_iter().collect());
    assert!(selection.anchor().is_none());
}

#[test]
fn equality_ignores_the_anchor() {
    let mut left = Selection::new();
    left.select(1, SelectionMode::Multi);

    let right: Selection<u32> = [1].into_iter().collect();

    // `right` has no anchor, but the selections are the same.
    assert_eq!(left, right);
    assert!(right.anchor().is_none());
}

#[test]
fn set_replaces_the_whole_selection() {
    let mut selection = Selection::new();
    selection.select(1, SelectionMode::Multi);
    selection.set([7, 8, 9]);

    assert_eq!(selected(&selection), [7, 8, 9]);
    assert!(selection.anchor().is_none());
}

#[test]
fn keys_can_be_any_hashable_type() {
    let order = ["a".to_owned(), "b".to_owned(), "c".to_owned()];
    let mut selection: Selection<String> = Selection::new();

    selection.select("a".to_owned(), SelectionMode::Multi);
    selection.extend_to(&order, "c".to_owned(), SelectionMode::Multi);

    assert_eq!(selection.len(), 3);
    assert!(selection.contains(&"b".to_owned()));
}
