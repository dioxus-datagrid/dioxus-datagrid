//! Multi-level column headers: how a column's group path becomes the rows of
//! header cells above it.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{ColumnSpec, GroupSpan, group_header_rows, group_levels};
use proptest::prelude::*;

/// A column named `id`, under the groups given.
fn column(id: &'static str, groups: &[&str]) -> ColumnSpec<()> {
    groups
        .iter()
        .fold(ColumnSpec::new(id), |column, group| column.group(*group))
}

fn rows(columns: &[ColumnSpec<()>]) -> Vec<Vec<GroupSpan>> {
    let refs: Vec<&ColumnSpec<()>> = columns.iter().collect();
    group_header_rows(&refs)
}

/// Each row as `(label, start, span)`, with `""` for a cell with no label.
fn shape(columns: &[ColumnSpec<()>]) -> Vec<Vec<(String, usize, usize)>> {
    rows(columns)
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|span| (span.label.unwrap_or_default(), span.start, span.span))
                .collect()
        })
        .collect()
}

#[test]
fn columns_without_groups_have_no_header_rows() {
    let columns = [column("name", &[]), column("age", &[])];

    assert_eq!(group_levels(&columns.iter().collect::<Vec<_>>()), 0);
    assert!(rows(&columns).is_empty());
}

#[test]
fn neighbours_in_the_same_group_share_one_cell() {
    let columns = [
        column("street", &["Address"]),
        column("city", &["Address"]),
        column("age", &[]),
    ];

    assert_eq!(
        shape(&columns),
        [[("Address".to_owned(), 0, 2), (String::new(), 2, 1),]]
    );
}

#[test]
fn a_column_without_a_group_gets_an_empty_cell_of_its_own() {
    let columns = [
        column("name", &[]),
        column("street", &["Address"]),
        column("age", &[]),
    ];

    // Every row covers every column, or the header would not line up.
    assert_eq!(
        shape(&columns),
        [[
            (String::new(), 0, 1),
            ("Address".to_owned(), 1, 1),
            (String::new(), 2, 1),
        ]]
    );
}

#[test]
fn a_group_split_by_another_column_becomes_two_cells() {
    let columns = [
        column("street", &["Address"]),
        column("age", &[]),
        column("city", &["Address"]),
    ];

    // The columns were reordered so that Address is no longer contiguous. Two
    // cells is the honest rendering; merging them would claim a span that
    // covers a column outside the group.
    assert_eq!(
        shape(&columns),
        [[
            ("Address".to_owned(), 0, 1),
            (String::new(), 1, 1),
            ("Address".to_owned(), 2, 1),
        ]]
    );
}

#[test]
fn groups_nest_outermost_first() {
    let columns = [
        column("street", &["Customer", "Address"]),
        column("city", &["Customer", "Address"]),
        column("phone", &["Customer"]),
    ];

    assert_eq!(
        shape(&columns),
        [
            vec![("Customer".to_owned(), 0, 3)],
            vec![("Address".to_owned(), 0, 2), (String::new(), 2, 1)],
        ]
    );
}

#[test]
fn the_same_name_under_different_parents_stays_two_groups() {
    let columns = [
        column("home_city", &["Home", "Address"]),
        column("work_city", &["Work", "Address"]),
    ];

    assert_eq!(
        shape(&columns),
        [
            vec![("Home".to_owned(), 0, 1), ("Work".to_owned(), 1, 1)],
            vec![("Address".to_owned(), 0, 1), ("Address".to_owned(), 1, 1)],
        ]
    );
}

#[test]
fn the_header_is_as_deep_as_the_longest_path() {
    let columns = [column("a", &["One"]), column("b", &["One", "Two", "Three"])];

    assert_eq!(group_levels(&columns.iter().collect::<Vec<_>>()), 3);
    assert_eq!(rows(&columns).len(), 3);
}

#[test]
fn no_columns_means_no_header_rows() {
    let columns: [ColumnSpec<()>; 0] = [];
    assert!(rows(&columns).is_empty());
}

proptest! {
    /// What the whole layout rests on: every header row covers every column
    /// exactly once, in order. A gap or an overlap would put `aria-colspan` at
    /// odds with the columns below and misalign the grid.
    #[test]
    fn every_row_covers_every_column_exactly_once(
        paths in prop::collection::vec(
            prop::collection::vec(prop::sample::select(vec!["A", "B", "C"]), 0..3),
            1..8,
        ),
    ) {
        let columns: Vec<ColumnSpec<()>> = paths
            .iter()
            .enumerate()
            .map(|(index, groups)| {
                groups.iter().fold(
                    ColumnSpec::<()>::new(format!("c{index}")),
                    |column, group| column.group(*group),
                )
            })
            .collect();

        for row in rows(&columns) {
            let mut next = 0;
            for span in &row {
                prop_assert_eq!(span.start, next);
                prop_assert!(span.span > 0);
                next += span.span;
            }
            prop_assert_eq!(next, columns.len());
        }
    }
}
