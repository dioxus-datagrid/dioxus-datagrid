//! Column widths: the minimum honoured while resizing, and state overriding a
//! column's own width.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{ColumnSpec, ColumnWidth, DEFAULT_MIN_COLUMN_WIDTH, GridState};

fn column() -> ColumnSpec<()> {
    ColumnSpec::new("name")
}

#[test]
fn resizing_never_goes_below_min_width() {
    let column = column().min_width(120.0);
    assert_eq!(column.clamp_width(80.0), 120.0);
    assert_eq!(column.clamp_width(-500.0), 120.0);
    assert_eq!(column.clamp_width(120.0), 120.0);
    assert_eq!(column.clamp_width(300.5), 300.5);
}

#[test]
fn a_column_without_min_width_uses_the_default_minimum() {
    assert_eq!(column().resize_min_width(), DEFAULT_MIN_COLUMN_WIDTH);
    assert_eq!(column().clamp_width(0.0), DEFAULT_MIN_COLUMN_WIDTH);
}

#[test]
fn an_unusable_min_width_falls_back_to_the_default() {
    for min in [f32::NAN, f32::INFINITY, -10.0] {
        assert_eq!(
            column().min_width(min).resize_min_width(),
            DEFAULT_MIN_COLUMN_WIDTH,
            "min_width {min}"
        );
    }
}

#[test]
fn a_non_finite_width_clamps_to_the_minimum() {
    let column = column().min_width(64.0);
    assert_eq!(column.clamp_width(f32::NAN), 64.0);
    assert_eq!(column.clamp_width(f32::INFINITY), 64.0);
}

#[test]
fn a_resized_width_overrides_the_column_width() {
    let column = column().width(ColumnWidth::Fraction(1.0));
    let mut state = GridState::new();
    assert_eq!(column.effective_width(&state), ColumnWidth::Fraction(1.0));

    state.set_column_width("name", 240.0);
    assert_eq!(column.effective_width(&state), ColumnWidth::Px(240.0));
}

#[test]
fn restored_state_cannot_undercut_min_width() {
    // Saved before the column gained a larger minimum.
    let column = column().min_width(150.0);
    let mut state = GridState::new();
    state.set_column_width("name", 90.0);
    assert_eq!(column.effective_width(&state), ColumnWidth::Px(150.0));
}

#[test]
fn a_width_for_another_column_is_ignored() {
    let mut state = GridState::new();
    state.set_column_width("email", 240.0);
    assert_eq!(column().effective_width(&state), ColumnWidth::Auto);
}

#[test]
fn a_non_finite_width_is_not_recorded() {
    let mut state = GridState::new();
    state.set_column_width("name", 200.0);
    state.set_column_width("name", f32::NAN);
    assert_eq!(state.column_width(&"name".into()), Some(200.0));
}

#[test]
fn columns_are_resizable_unless_opted_out() {
    assert!(column().resizable);
    assert!(!column().resizable(false).resizable);
}

#[test]
fn resetting_a_width_falls_back_to_the_column_width() {
    let column = column().width(ColumnWidth::Px(100.0));
    let mut state = GridState::new();
    state.set_column_width("name", 240.0);
    state.reset_column_width(&"name".into());
    assert_eq!(column.effective_width(&state), ColumnWidth::Px(100.0));
    assert!(state.column_widths.is_empty());
}
