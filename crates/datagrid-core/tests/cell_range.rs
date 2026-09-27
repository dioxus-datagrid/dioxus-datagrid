//! Rectangles of cells: how the two corners a user makes become a rectangle,
//! and what happens to one that outlives the view it was made in.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{CellFocus, CellRange, CellSelectionMode};
use proptest::prelude::*;

fn at(row: usize, col: usize) -> CellFocus {
    CellFocus::new(row, col)
}

/// Every cell the range covers, row by row.
fn cells(range: &CellRange) -> Vec<CellFocus> {
    range
        .rows()
        .flat_map(|row| range.columns().map(move |col| at(row, col)))
        .collect()
}

#[test]
fn a_new_range_holds_one_cell() {
    let range = CellRange::single(at(2, 3));

    assert!(range.is_single());
    assert_eq!(range.cell_count(), 1);
    assert_eq!(range.top_left(), at(2, 3));
    assert_eq!(range.bottom_right(), at(2, 3));
    assert!(range.contains(at(2, 3)));
    assert!(!range.contains(at(2, 4)));
}

#[test]
fn extending_keeps_the_anchor_and_moves_the_other_end() {
    let range = CellRange::single(at(1, 1)).extended_to(at(3, 2));

    assert_eq!(range.anchor, at(1, 1));
    assert_eq!(range.focus, at(3, 2));

    // Extended again from the same anchor: one rectangle, not two.
    let smaller = range.extended_to(at(2, 1));
    assert_eq!(smaller.anchor, at(1, 1));
    assert_eq!(smaller.cell_count(), 2);
}

#[test]
fn a_range_built_backwards_reads_as_a_rectangle() {
    // Dragged up and to the left, so neither corner is the top left.
    let range = CellRange::single(at(4, 5)).extended_to(at(2, 3));

    assert_eq!(range.top_left(), at(2, 3));
    assert_eq!(range.bottom_right(), at(4, 5));
    assert_eq!(range.rows().collect::<Vec<_>>(), [2, 3, 4]);
    assert_eq!(range.columns().collect::<Vec<_>>(), [3, 4, 5]);
    assert_eq!(range.cell_count(), 9);
}

#[test]
fn a_range_covers_every_cell_between_its_corners() {
    let range = CellRange::single(at(0, 0)).extended_to(at(1, 1));

    assert_eq!(cells(&range), [at(0, 0), at(0, 1), at(1, 0), at(1, 1)]);
    for cell in cells(&range) {
        assert!(range.contains(cell), "{cell:?}");
    }
    assert!(!range.contains(at(2, 0)));
    assert!(!range.contains(at(0, 2)));
}

#[test]
fn a_range_from_a_larger_view_is_pulled_inside_this_one() {
    // Made over six rows and four columns, read back in a grid of three by two.
    let range = CellRange::single(at(5, 3))
        .extended_to(at(1, 0))
        .clamped(3, 2);

    assert_eq!(range.top_left(), at(1, 0));
    assert_eq!(range.bottom_right(), at(2, 1));
}

#[test]
fn a_range_in_an_empty_grid_collapses_to_the_origin() {
    let range = CellRange::single(at(4, 4)).clamped(0, 0);

    assert_eq!(range.top_left(), at(0, 0));
    assert_eq!(range.cell_count(), 1);
}

#[test]
fn a_mode_says_what_it_allows() {
    assert!(!CellSelectionMode::None.is_enabled());
    assert!(!CellSelectionMode::None.is_range());
    assert!(CellSelectionMode::Single.is_enabled());
    assert!(!CellSelectionMode::Single.is_range());
    assert!(CellSelectionMode::Range.is_enabled());
    assert!(CellSelectionMode::Range.is_range());
    assert_eq!(CellSelectionMode::default(), CellSelectionMode::None);
}

proptest! {
    /// However the corners lie, the rectangle holds both of them, holds as many
    /// cells as its sides say, and holds nothing outside them.
    #[test]
    fn a_rectangle_agrees_with_its_corners(
        anchor_row in 0usize..8, anchor_col in 0usize..8,
        focus_row in 0usize..8, focus_col in 0usize..8,
    ) {
        let range = CellRange::single(at(anchor_row, anchor_col))
            .extended_to(at(focus_row, focus_col));

        prop_assert!(range.contains(range.anchor));
        prop_assert!(range.contains(range.focus));

        let rows = anchor_row.abs_diff(focus_row) + 1;
        let cols = anchor_col.abs_diff(focus_col) + 1;
        prop_assert_eq!(range.cell_count(), rows * cols);
        prop_assert_eq!(cells(&range).len(), rows * cols);
        prop_assert_eq!(range.is_single(), rows == 1 && cols == 1);

        // One row and one column outside the rectangle are outside it.
        let outside = at(range.bottom_right().row + 1, range.bottom_right().col + 1);
        prop_assert!(!range.contains(outside));
    }

    /// Clamping never leaves the grid, and never turns a rectangle inside out.
    #[test]
    fn clamping_lands_inside_the_grid(
        anchor_row in 0usize..20, anchor_col in 0usize..20,
        focus_row in 0usize..20, focus_col in 0usize..20,
        rows in 1usize..6, cols in 1usize..6,
    ) {
        let range = CellRange::single(at(anchor_row, anchor_col))
            .extended_to(at(focus_row, focus_col))
            .clamped(rows, cols);

        prop_assert!(range.bottom_right().row < rows);
        prop_assert!(range.bottom_right().col < cols);
        prop_assert!(range.top_left().row <= range.bottom_right().row);
        prop_assert!(range.top_left().col <= range.bottom_right().col);
    }
}
