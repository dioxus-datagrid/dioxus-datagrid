//! Unit tests for the keyboard navigation state machine.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{CellFocus, NavKey, navigate};

/// A 10x5 grid with a page of 4 rows.
fn go(focus: CellFocus, key: NavKey) -> CellFocus {
    navigate(focus, key, 10, 5, 4)
}

#[test]
fn arrows_move_by_one() {
    let start = CellFocus::new(3, 2);
    assert_eq!(go(start, NavKey::Up), CellFocus::new(2, 2));
    assert_eq!(go(start, NavKey::Down), CellFocus::new(4, 2));
    assert_eq!(go(start, NavKey::Left), CellFocus::new(3, 1));
    assert_eq!(go(start, NavKey::Right), CellFocus::new(3, 3));
}

#[test]
fn movement_clamps_at_the_edges_instead_of_wrapping() {
    // Wrapping would make the arrow keys jump unpredictably across rows.
    assert_eq!(go(CellFocus::new(0, 0), NavKey::Up), CellFocus::new(0, 0));
    assert_eq!(go(CellFocus::new(0, 0), NavKey::Left), CellFocus::new(0, 0));
    assert_eq!(go(CellFocus::new(9, 4), NavKey::Down), CellFocus::new(9, 4));
    assert_eq!(
        go(CellFocus::new(9, 4), NavKey::Right),
        CellFocus::new(9, 4)
    );
}

#[test]
fn home_and_end_stay_within_the_row() {
    let start = CellFocus::new(3, 2);
    assert_eq!(go(start, NavKey::Home), CellFocus::new(3, 0));
    assert_eq!(go(start, NavKey::End), CellFocus::new(3, 4));
}

#[test]
fn ctrl_home_and_ctrl_end_jump_to_the_corners() {
    let start = CellFocus::new(3, 2);
    assert_eq!(go(start, NavKey::CtrlHome), CellFocus::new(0, 0));
    assert_eq!(go(start, NavKey::CtrlEnd), CellFocus::new(9, 4));
}

#[test]
fn page_keys_move_by_a_viewport_and_clamp() {
    assert_eq!(
        go(CellFocus::new(6, 1), NavKey::PageUp),
        CellFocus::new(2, 1)
    );
    assert_eq!(
        go(CellFocus::new(2, 1), NavKey::PageDown),
        CellFocus::new(6, 1)
    );
    assert_eq!(
        go(CellFocus::new(1, 1), NavKey::PageUp),
        CellFocus::new(0, 1)
    );
    assert_eq!(
        go(CellFocus::new(8, 1), NavKey::PageDown),
        CellFocus::new(9, 1)
    );
}

#[test]
fn a_page_size_of_zero_leaves_the_focus_where_it_is() {
    let start = CellFocus::new(5, 2);
    assert_eq!(navigate(start, NavKey::PageUp, 10, 5, 0), start);
    assert_eq!(navigate(start, NavKey::PageDown, 10, 5, 0), start);
}

#[test]
fn a_focus_left_over_from_a_larger_grid_is_pulled_back_in_bounds() {
    // Filtering can shrink the grid under a focus that was valid before.
    let stale = CellFocus::new(99, 99);
    assert_eq!(navigate(stale, NavKey::Up, 10, 5, 4), CellFocus::new(8, 4));
    assert_eq!(
        navigate(stale, NavKey::Home, 10, 5, 4),
        CellFocus::new(9, 0)
    );
}

#[test]
fn an_empty_grid_focuses_the_origin() {
    let start = CellFocus::new(3, 2);
    for key in [
        NavKey::Up,
        NavKey::Down,
        NavKey::Left,
        NavKey::Right,
        NavKey::Home,
        NavKey::End,
        NavKey::CtrlHome,
        NavKey::CtrlEnd,
        NavKey::PageUp,
        NavKey::PageDown,
    ] {
        assert_eq!(navigate(start, key, 0, 5, 4), CellFocus::new(0, 0));
        assert_eq!(navigate(start, key, 10, 0, 4), CellFocus::new(0, 0));
    }
}

#[test]
fn a_single_cell_grid_never_moves() {
    let origin = CellFocus::new(0, 0);
    for key in [
        NavKey::Up,
        NavKey::Down,
        NavKey::Left,
        NavKey::Right,
        NavKey::Home,
        NavKey::End,
        NavKey::CtrlHome,
        NavKey::CtrlEnd,
        NavKey::PageUp,
        NavKey::PageDown,
    ] {
        assert_eq!(navigate(origin, key, 1, 1, 4), origin);
    }
}

#[test]
fn page_down_cannot_overflow() {
    // A pathological page size must not wrap around.
    let focus = navigate(CellFocus::new(5, 0), NavKey::PageDown, 10, 5, usize::MAX);
    assert_eq!(focus, CellFocus::new(9, 0));
}
