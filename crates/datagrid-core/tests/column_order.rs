//! The column order: resolving it against the declared columns, moving a
//! column, and what a persisted order does when the columns have changed.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{ColumnId, GridState};
use proptest::prelude::*;

fn ids(names: &[&str]) -> Vec<ColumnId> {
    names
        .iter()
        .map(|name| ColumnId::new(name.to_string()))
        .collect()
}

fn names(ids: &[ColumnId]) -> Vec<String> {
    ids.iter().map(|id| id.as_str().to_owned()).collect()
}

fn declared() -> Vec<ColumnId> {
    ids(&["name", "email", "age", "city"])
}

#[test]
fn without_an_order_the_columns_stay_as_declared() {
    let state = GridState::new();
    assert_eq!(state.ordered_columns(&declared()), declared());
}

#[test]
fn a_full_order_is_followed() {
    let mut state = GridState::new();
    state.set_column_order(ids(&["city", "name", "age", "email"]));

    assert_eq!(
        names(&state.ordered_columns(&declared())),
        ["city", "name", "age", "email"]
    );
}

#[test]
fn columns_the_order_does_not_mention_follow_in_their_declared_order() {
    let mut state = GridState::new();
    state.set_column_order(ids(&["city"]));

    assert_eq!(
        names(&state.ordered_columns(&declared())),
        ["city", "name", "email", "age"]
    );
}

#[test]
fn an_order_naming_columns_that_are_gone_ignores_them() {
    let mut state = GridState::new();
    // A state persisted when the grid still had a "phone" column.
    state.set_column_order(ids(&["phone", "city", "name"]));

    assert_eq!(
        names(&state.ordered_columns(&declared())),
        ["city", "name", "email", "age"]
    );
}

#[test]
fn moving_a_column_puts_it_before_the_named_one() {
    let mut state = GridState::new();
    state.move_column(
        &declared(),
        &ColumnId::new("city"),
        Some(&ColumnId::new("email")),
    );

    assert_eq!(
        names(&state.ordered_columns(&declared())),
        ["name", "city", "email", "age"]
    );
}

#[test]
fn moving_a_column_without_a_target_puts_it_last() {
    let mut state = GridState::new();
    state.move_column(&declared(), &ColumnId::new("name"), None);

    assert_eq!(
        names(&state.ordered_columns(&declared())),
        ["email", "age", "city", "name"]
    );
}

#[test]
fn a_move_writes_out_the_whole_order() {
    let mut state = GridState::new();
    state.move_column(
        &declared(),
        &ColumnId::new("age"),
        Some(&ColumnId::new("name")),
    );

    // Not just the moved column: later moves must not depend on the declared
    // order any more.
    assert_eq!(state.column_order.len(), declared().len());
}

#[test]
fn moving_a_column_before_itself_changes_nothing() {
    let mut state = GridState::new();
    let city = ColumnId::new("city");
    state.move_column(&declared(), &city, Some(&city));

    assert!(state.column_order.is_empty());
    assert_eq!(state.ordered_columns(&declared()), declared());
}

#[test]
fn moving_a_column_the_grid_does_not_declare_changes_nothing() {
    let mut state = GridState::new();
    state.move_column(
        &declared(),
        &ColumnId::new("phone"),
        Some(&ColumnId::new("age")),
    );

    assert!(state.column_order.is_empty());
}

#[test]
fn moving_before_a_column_that_is_gone_puts_it_last() {
    let mut state = GridState::new();
    state.move_column(
        &declared(),
        &ColumnId::new("name"),
        Some(&ColumnId::new("phone")),
    );

    assert_eq!(
        names(&state.ordered_columns(&declared())),
        ["email", "age", "city", "name"]
    );
}

proptest! {
    /// The invariant the whole feature rests on: whatever is moved, the grid
    /// shows every declared column exactly once. Losing one would lose data
    /// from the screen; showing one twice would break `aria-colindex`.
    #[test]
    fn an_order_is_always_a_permutation_of_the_declared_columns(
        moves in prop::collection::vec((0usize..4, 0usize..5), 0..12),
    ) {
        let declared = declared();
        let mut state = GridState::new();

        for (column, before) in moves {
            let order = state.ordered_columns(&declared);
            let column = order[column % order.len()].clone();
            let before = order.get(before).cloned();
            state.move_column(&declared, &column, before.as_ref());

            let shown = state.ordered_columns(&declared);
            prop_assert_eq!(shown.len(), declared.len());
            for id in &declared {
                prop_assert_eq!(shown.iter().filter(|shown| *shown == id).count(), 1);
            }
        }
    }
}
