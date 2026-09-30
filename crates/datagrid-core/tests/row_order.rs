//! Moving a row: what "to" means, and what a move and its reverse do.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::move_row;
use proptest::prelude::*;

fn letters() -> Vec<&'static str> {
    vec!["a", "b", "c", "d", "e"]
}

#[test]
fn a_row_ends_up_where_it_was_sent() {
    let mut rows = letters();
    assert!(move_row(&mut rows, 0, 2));
    assert_eq!(rows, ["b", "c", "a", "d", "e"]);
    assert_eq!(rows[2], "a", "the index it was sent to is where it is");
}

#[test]
fn moving_up_and_moving_down_are_each_other() {
    let mut rows = letters();
    move_row(&mut rows, 3, 1);
    assert_eq!(rows, ["a", "d", "b", "c", "e"]);
    move_row(&mut rows, 1, 3);
    assert_eq!(rows, letters());
}

#[test]
fn one_step_is_one_step_in_both_directions() {
    let mut rows = letters();
    move_row(&mut rows, 2, 3);
    assert_eq!(rows, ["a", "b", "d", "c", "e"]);
    move_row(&mut rows, 3, 2);
    assert_eq!(rows, letters());
}

#[test]
fn a_target_past_the_end_lands_at_the_end() {
    let mut rows = letters();
    assert!(move_row(&mut rows, 1, 99));
    assert_eq!(rows, ["a", "c", "d", "e", "b"]);
}

#[test]
fn a_move_that_moves_nothing_says_so() {
    let mut rows = letters();
    assert!(!move_row(&mut rows, 2, 2));
    assert!(!move_row(&mut rows, 99, 0), "no such row");
    assert_eq!(rows, letters());

    let mut empty: Vec<&str> = Vec::new();
    assert!(!move_row(&mut empty, 0, 0));
}

proptest! {
    /// A move keeps every row: it reorders, it never loses or copies one.
    #[test]
    fn a_move_keeps_every_row(
        len in 1usize..8,
        from in 0usize..8,
        to in 0usize..8,
    ) {
        let mut rows: Vec<usize> = (0..len).collect();
        let before = rows.clone();
        move_row(&mut rows, from, to);

        prop_assert_eq!(rows.len(), before.len());
        let mut sorted = rows.clone();
        sorted.sort_unstable();
        prop_assert_eq!(sorted, before);
    }

    /// Whatever a move did, moving the row back where it came from undoes it.
    #[test]
    fn a_move_and_its_reverse_cancel(len in 2usize..8, from in 0usize..8, to in 0usize..8) {
        let mut rows: Vec<usize> = (0..len).collect();
        let before = rows.clone();
        let from = from.min(len - 1);
        let to = to.min(len - 1);

        move_row(&mut rows, from, to);
        move_row(&mut rows, to, from);

        prop_assert_eq!(rows, before);
    }
}
