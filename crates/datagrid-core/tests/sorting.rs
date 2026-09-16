//! Unit tests for [`SortValue`]'s total order and for multi-column sorting.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{
    ColumnSpec, GridState, SortDirection, SortState, SortValue, TextCollation, compute_view,
};
use std::cmp::Ordering;

mod common;
use common::{User, names_of, sample_rows};

#[test]
fn none_sorts_after_every_other_value() {
    let values = [
        SortValue::Bool(true),
        SortValue::Int(i64::MAX),
        SortValue::Float(f64::INFINITY),
        SortValue::Text("zzz"),
    ];
    for value in values {
        assert_eq!(
            value.cmp(&SortValue::None),
            Ordering::Less,
            "{value:?} should sort before None"
        );
    }
    assert_eq!(SortValue::None.cmp(&SortValue::None), Ordering::Equal);
}

#[test]
fn integers_and_floats_compare_numerically_across_variants() {
    assert_eq!(
        SortValue::Int(2).cmp(&SortValue::Float(10.5)),
        Ordering::Less
    );
    assert_eq!(
        SortValue::Float(10.5).cmp(&SortValue::Int(2)),
        Ordering::Greater
    );
    // Equal values across the two variants really are equal.
    assert_eq!(SortValue::Int(3), SortValue::Float(3.0));
}

#[test]
fn floats_use_total_cmp_so_nan_is_ordered() {
    let nan = SortValue::Float(f64::NAN);
    // total_cmp puts NaN at the ends rather than declaring it unordered, so
    // sorting a column containing NaN still terminates with a defined result.
    assert_eq!(nan.cmp(&nan), Ordering::Equal);
    assert_eq!(nan, SortValue::Float(f64::NAN));
    assert_ne!(nan, SortValue::Float(1.0));
}

#[test]
fn text_is_case_insensitive_by_default() {
    let lower = SortValue::Text("adam");
    let upper = SortValue::Text("Zoe");
    assert_eq!(lower.cmp(&upper), Ordering::Less);

    // Case-sensitive collation orders uppercase first instead.
    assert_eq!(
        lower.cmp_with(&upper, TextCollation::CaseSensitive),
        Ordering::Greater
    );
}

#[test]
fn values_differing_only_in_case_are_ordered_not_equal() {
    let lower = SortValue::Text("abc");
    let upper = SortValue::Text("ABC");

    // They must not compare Equal, otherwise Ord and PartialEq would disagree
    // with each other and the order would stop being total.
    assert_ne!(lower.cmp(&upper), Ordering::Equal);
    assert_ne!(lower, upper);
}

#[test]
fn ord_and_eq_agree() {
    let values = [
        SortValue::None,
        SortValue::Bool(false),
        SortValue::Int(1),
        SortValue::Float(1.0),
        SortValue::Text("a"),
        SortValue::Text("A"),
    ];
    for left in &values {
        for right in &values {
            assert_eq!(
                left.cmp(right) == Ordering::Equal,
                left == right,
                "{left:?} vs {right:?}"
            );
        }
    }
}

#[test]
fn option_maps_none_to_sort_value_none() {
    assert_eq!(SortValue::from(None::<u32>), SortValue::None);
    assert_eq!(SortValue::from(Some(7_u32)), SortValue::Int(7));
}

#[test]
fn wide_integers_beyond_i64_degrade_to_float_instead_of_wrapping() {
    let big = u64::MAX;
    let value = SortValue::from(big);
    assert!(matches!(value, SortValue::Float(_)));
    // Still ordered above anything that fits in an i64.
    assert_eq!(value.cmp(&SortValue::Int(i64::MAX)), Ordering::Greater);
}

#[test]
fn sorting_is_stable_for_equal_keys() {
    // Every row has the same age, so the sort must preserve the input order.
    let rows: Vec<User> = (0..10)
        .map(|i| User::new(i, &format!("user{i}"), 40))
        .collect();
    let columns = vec![ColumnSpec::new("age").sort_by_value(|user: &User| user.age)];

    let mut state = GridState::new();
    state.sort = vec![SortState::asc("age")];

    let view = compute_view(&rows, &columns, &state);
    assert_eq!(view.indices, (0..10).collect::<Vec<_>>());
}

#[test]
fn multi_sort_follows_priority_order() {
    let rows = sample_rows();
    let columns = vec![
        ColumnSpec::new("age").sort_by_value(|user: &User| user.age),
        ColumnSpec::new("name").sort_by_text(|user: &User| user.name.as_str()),
    ];

    let mut state = GridState::new();
    state.sort = vec![SortState::asc("age"), SortState::asc("name")];

    let view = compute_view(&rows, &columns, &state);
    let ordered: Vec<(u32, &str)> = view
        .indices
        .iter()
        .map(|&i| (rows[i].age, rows[i].name.as_str()))
        .collect();

    assert_eq!(
        ordered,
        [
            (25, "adam"),
            (30, "Mia"),
            (30, "Zoe"),
            (41, "bob"),
            (41, "Carol"),
        ]
    );
}

#[test]
fn reversing_the_primary_column_keeps_the_secondary_ascending() {
    let rows = sample_rows();
    let columns = vec![
        ColumnSpec::new("age").sort_by_value(|user: &User| user.age),
        ColumnSpec::new("name").sort_by_text(|user: &User| user.name.as_str()),
    ];

    let mut state = GridState::new();
    state.sort = vec![
        SortState::new("age", SortDirection::Desc),
        SortState::asc("name"),
    ];

    let view = compute_view(&rows, &columns, &state);
    let ordered: Vec<(u32, &str)> = view
        .indices
        .iter()
        .map(|&i| (rows[i].age, rows[i].name.as_str()))
        .collect();

    assert_eq!(
        ordered,
        [
            (41, "bob"),
            (41, "Carol"),
            (30, "Mia"),
            (30, "Zoe"),
            (25, "adam"),
        ]
    );
}

#[test]
fn sort_entries_for_unknown_or_unsortable_columns_are_skipped() {
    let rows = sample_rows();
    let columns = vec![
        // Deliberately has no sort key.
        ColumnSpec::new("email").filter_by(|user: &User| user.email.clone()),
        ColumnSpec::new("name").sort_by_text(|user: &User| user.name.as_str()),
    ];

    let mut state = GridState::new();
    state.sort = vec![
        SortState::asc("does-not-exist"),
        SortState::asc("email"),
        SortState::asc("name"),
    ];

    let view = compute_view(&rows, &columns, &state);
    assert_eq!(
        names_of(&rows, &view),
        ["adam", "bob", "Carol", "Mia", "Zoe"]
    );
}

#[test]
fn descending_puts_missing_values_first() {
    let rows = vec![
        User::new(1, "has", 10).with_nickname("nick"),
        User::new(2, "missing", 20),
        User::new(3, "also", 30).with_nickname("other"),
    ];
    // `Option<String>` borrows via `as_deref`, which yields `Option<&str>`.
    let columns =
        vec![ColumnSpec::new("nickname").sort_by(|user: &User| user.nickname.as_deref().into())];

    // Ascending: nicknames in order, then the row that has none.
    let mut state = GridState::new();
    state.sort = vec![SortState::asc("nickname")];
    let ascending = compute_view(&rows, &columns, &state);
    assert_eq!(names_of(&rows, &ascending), ["has", "also", "missing"]);

    // Descending reverses the whole order, so the missing value leads.
    state.sort = vec![SortState::new("nickname", SortDirection::Desc)];
    let descending = compute_view(&rows, &columns, &state);
    assert_eq!(names_of(&rows, &descending), ["missing", "also", "has"]);
}
