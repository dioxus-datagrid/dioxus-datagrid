//! Unit tests for the virtualization arithmetic.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{offset_of, total_height, visible_range};

#[test]
fn returns_the_rows_intersecting_the_viewport() {
    // 20px rows, scrolled 100px, 50px viewport: rows 5, 6 and 7.
    assert_eq!(visible_range(100.0, 50.0, 20.0, 1000, 0), 5..8);
}

#[test]
fn overscan_widens_the_range_on_both_sides() {
    assert_eq!(visible_range(100.0, 50.0, 20.0, 1000, 2), 3..10);
}

#[test]
fn overscan_does_not_escape_the_row_count() {
    // At the very top there is nothing above to overscan into.
    assert_eq!(visible_range(0.0, 50.0, 20.0, 4, 10), 0..4);
    // At the very bottom, nothing below.
    assert_eq!(visible_range(1000.0, 50.0, 20.0, 52, 10), 40..52);
}

#[test]
fn negative_scroll_is_clamped() {
    // iOS rubber-band overscroll reports a negative scrollTop.
    assert_eq!(visible_range(-40.0, 50.0, 20.0, 1000, 0), 0..3);
}

#[test]
fn non_finite_input_never_panics_and_stays_in_bounds() {
    for scroll in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let range = visible_range(scroll, 50.0, 20.0, 100, 2);
        assert!(range.start <= range.end);
        assert!(range.end <= 100);
    }
    for viewport in [f64::NAN, f64::INFINITY] {
        let range = visible_range(0.0, viewport, 20.0, 100, 2);
        assert!(range.end <= 100);
    }
}

#[test]
fn a_degenerate_row_height_yields_an_empty_range() {
    // There is no correct answer here; guessing one would render wrong rows.
    for height in [0.0, -20.0, f64::NAN, f64::INFINITY] {
        assert_eq!(visible_range(100.0, 50.0, height, 1000, 2), 0..0);
    }
}

#[test]
fn an_empty_grid_yields_an_empty_range() {
    assert_eq!(visible_range(0.0, 500.0, 20.0, 0, 5), 0..0);
}

#[test]
fn a_zero_height_viewport_still_yields_the_row_under_the_scroll_position() {
    let range = visible_range(100.0, 0.0, 20.0, 1000, 0);
    assert!(range.start <= range.end);
    assert_eq!(range.start, 5);
}

#[test]
fn scrolling_past_the_end_clamps_to_the_last_row() {
    let range = visible_range(1_000_000.0, 50.0, 20.0, 100, 0);
    assert_eq!(range, 100..100);
}

#[test]
fn total_height_matches_the_rendered_rows() {
    assert_eq!(total_height(100, 20.0), 2000.0);
    assert_eq!(total_height(0, 20.0), 0.0);
    // A degenerate row height contributes no scrollable area.
    assert_eq!(total_height(100, 0.0), 0.0);
    assert_eq!(total_height(100, f64::NAN), 0.0);
}

#[test]
fn spacers_above_and_below_add_up_to_the_total_height() {
    let (row_height, total_rows, overscan) = (20.0, 500, 3);
    let range = visible_range(1234.0, 400.0, row_height, total_rows, overscan);

    let above = offset_of(range.start, row_height);
    let rendered = total_height(range.end - range.start, row_height);
    let below = total_height(total_rows - range.end, row_height);

    // If these three do not sum to the total, the scrollbar lies about the
    // size of the data.
    assert!((above + rendered + below - total_height(total_rows, row_height)).abs() < f64::EPSILON);
}
