//! Cells that cover more than one column: how a row's declared spans become
//! cells, and where the arrow keys land among them.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{ColumnSpec, Pinned, RowSpans};
use proptest::prelude::*;

/// Spans declared for one row, none of the columns pinned.
fn spans(declared: &[usize]) -> RowSpans {
    let columns: Vec<(usize, Pinned)> = declared.iter().map(|span| (*span, Pinned::None)).collect();
    RowSpans::resolve(&columns)
}

/// The cells of a row as `(start, width)`.
fn cells(spans: &RowSpans) -> Vec<(usize, usize)> {
    (0..spans.len())
        .filter(|column| spans.is_anchor(*column))
        .map(|column| (column, spans.width(column)))
        .collect()
}

#[test]
fn a_row_without_spans_gives_every_column_its_own_cell() {
    let spans = spans(&[1, 1, 1]);

    assert_eq!(cells(&spans), [(0, 1), (1, 1), (2, 1)]);
    assert!(!spans.has_spans());
    assert_eq!(RowSpans::none(3), spans);
}

#[test]
fn a_span_swallows_the_columns_after_it() {
    let spans = spans(&[3, 1, 1, 1]);

    assert_eq!(cells(&spans), [(0, 3), (3, 1)]);
    assert!(spans.has_spans());
    assert_eq!(spans.anchor(1), 0);
    assert_eq!(spans.anchor(2), 0);
    assert_eq!(spans.width(2), 3);
}

#[test]
fn a_span_declared_by_a_covered_column_is_never_read() {
    // The second column would span two, but the first already covers it.
    let spans = spans(&[3, 2, 1, 1]);

    assert_eq!(cells(&spans), [(0, 3), (3, 1)]);
}

#[test]
fn a_span_stops_at_the_last_column() {
    assert_eq!(cells(&spans(&[1, 9, 1])), [(0, 1), (1, 2)]);
}

#[test]
fn a_span_of_zero_covers_only_its_own_column() {
    assert_eq!(cells(&spans(&[0, 0])), [(0, 1), (1, 1)]);
}

#[test]
fn a_span_never_leaves_its_pinned_block() {
    // Two columns held at the start, then two that scroll. The first would
    // cover three, which would make a cell that is both held and scrolling.
    let spans = RowSpans::resolve(&[
        (3, Pinned::Start),
        (1, Pinned::Start),
        (1, Pinned::None),
        (1, Pinned::None),
    ]);

    assert_eq!(cells(&spans), [(0, 2), (2, 1), (3, 1)]);
}

#[test]
fn a_span_within_a_pinned_block_is_kept() {
    let spans = RowSpans::resolve(&[
        (2, Pinned::Start),
        (1, Pinned::Start),
        (2, Pinned::None),
        (1, Pinned::None),
    ]);

    assert_eq!(cells(&spans), [(0, 2), (2, 2)]);
}

#[test]
fn an_empty_row_has_nothing_to_step_to() {
    let spans = spans(&[]);

    assert!(spans.is_empty());
    assert_eq!(spans.step(0, 1), 0);
}

#[test]
fn moving_right_out_of_a_wide_cell_skips_what_it_covers() {
    let spans = spans(&[3, 1, 1, 1]);

    // From the wide cell, one to the right is the column after it, not the one
    // it covers.
    assert_eq!(spans.step(0, 1), 3);
    // And back again lands on the cell, not on the column it covers.
    assert_eq!(spans.step(3, 2), 0);
}

#[test]
fn a_wide_cell_at_the_end_holds_the_focus() {
    let spans = spans(&[1, 3]);

    // There is nothing after it, so End and the right arrow both stay.
    assert_eq!(spans.step(1, 2), 1);
    assert_eq!(spans.step(1, 3), 1);
}

#[test]
fn moving_down_onto_a_covered_column_lands_on_its_cell() {
    let spans = spans(&[1, 3]);

    // The row above had four cells; this one has two.
    assert_eq!(spans.step(3, 3), 1);
    assert_eq!(spans.step(2, 2), 1);
}

#[test]
fn a_move_straight_down_is_not_a_move_to_the_right() {
    let spans = spans(&[3, 1, 1, 1]);

    // Coming down the third column, which this row's first cell covers: the
    // cell under it, not the one after it.
    assert_eq!(spans.step(2, 2), 0);
    assert_eq!(spans.step(1, 1), 0);
}

#[test]
fn a_move_past_the_end_stops_at_the_last_cell() {
    let spans = spans(&[1, 1, 1]);

    assert_eq!(spans.step(0, 99), 2);
    assert_eq!(spans.anchor(99), 99, "a stale column stays where it is");
}

#[test]
fn a_column_spans_one_unless_it_says_otherwise() {
    let plain: ColumnSpec<u32> = ColumnSpec::new("plain");
    assert_eq!(plain.span_at(&7), 1);

    let wide = ColumnSpec::new("wide").span(|row: &u32| *row as usize);
    assert_eq!(wide.span_at(&3), 3);
    // Never zero, so a caller can use it as a width without checking.
    assert_eq!(wide.span_at(&0), 1);
}

proptest! {
    /// Whatever the columns declare, the cells tile the row: every column is
    /// covered exactly once, by a cell that starts at or before it.
    #[test]
    fn cells_tile_the_row(declared in prop::collection::vec(0usize..6, 0..12)) {
        let spans = spans(&declared);
        prop_assert_eq!(spans.len(), declared.len());

        let mut column = 0;
        for (start, width) in cells(&spans) {
            prop_assert_eq!(start, column, "a gap or an overlap between cells");
            prop_assert!(width >= 1);
            for covered in start..start + width {
                prop_assert_eq!(spans.anchor(covered), start);
            }
            column += width;
        }
        prop_assert_eq!(column, declared.len(), "the last cell stops at the last column");
    }

    /// A step always lands on a cell, and never inside the one it started on
    /// while there is another one to reach.
    #[test]
    fn a_step_lands_on_a_cell(
        declared in prop::collection::vec(0usize..6, 1..12),
        from in 0usize..12,
        to in 0usize..12,
    ) {
        let spans = spans(&declared);
        let landed = spans.step(from, to);

        prop_assert!(landed < spans.len());
        prop_assert!(spans.is_anchor(landed));

        let start = spans.anchor(from.min(spans.len() - 1));
        if to > from && landed == start {
            // It stayed put, so the cell it started on must reach the end.
            prop_assert_eq!(start + spans.width(start), spans.len());
        }
    }
}
