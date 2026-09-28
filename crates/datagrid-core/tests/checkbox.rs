//! What a checkbox column is, before any of it is rendered.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{CellAlign, ColumnSpec, ColumnWidth};

#[test]
fn a_checkbox_column_asks_to_be_narrow_and_out_of_the_way() {
    let column = ColumnSpec::<()>::new("select").checkbox();

    assert!(column.is_checkbox());
    assert_eq!(column.width, ColumnWidth::Px(44.0));
    assert_eq!(column.align, Some(CellAlign::Center));
    // Nothing to sort, group, resize or edit by: the column holds no value.
    assert!(!column.is_sortable());
    assert!(!column.is_groupable());
    assert!(!column.resizable);
    assert!(!column.is_editable());
}

#[test]
fn what_a_checkbox_column_asks_for_can_be_overridden() {
    let column = ColumnSpec::<()>::new("select")
        .checkbox()
        .width(ColumnWidth::Px(64.0))
        .resizable(true);

    assert!(column.is_checkbox());
    assert_eq!(column.width, ColumnWidth::Px(64.0));
    assert!(column.resizable);
}

#[test]
fn an_ordinary_column_is_not_one() {
    let column = ColumnSpec::<()>::new("name");

    assert!(!column.is_checkbox());
    // And the two are told apart when a column list is compared.
    assert_ne!(column.clone(), column.checkbox());
}
