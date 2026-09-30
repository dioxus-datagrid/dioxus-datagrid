//! Detail rows: where a row's detail sits in the view, and what counts it.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{
    ColumnSpec, GridState, PageState, SortState, ViewRow, compute_view_with_details,
};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq)]
struct Row {
    id: usize,
    city: String,
}

fn rows() -> Vec<Row> {
    ["Berlin", "Aachen", "Kiel", "Dresden"]
        .iter()
        .enumerate()
        .map(|(id, city)| Row {
            id,
            city: (*city).to_owned(),
        })
        .collect()
}

fn columns() -> Vec<ColumnSpec<Row>> {
    vec![
        ColumnSpec::new("id").value_of(|row: &Row| i64::try_from(row.id).unwrap_or_default()),
        ColumnSpec::new("city")
            .value_text(|row: &Row| row.city.as_str())
            .filter_by(|row: &Row| row.city.clone()),
    ]
}

/// The view's rows, spelled out: `d0` for the data row of row 0, `x0` for its
/// detail.
fn shape(view: &datagrid_core::View) -> Vec<String> {
    view.rows
        .iter()
        .map(|row| match row {
            ViewRow::Data(index) => format!("d{index}"),
            ViewRow::Detail(index) => format!("x{index}"),
            ViewRow::GroupHeader(group) => format!("g{group}"),
            ViewRow::GroupFooter(group) => format!("f{group}"),
            _ => "?".to_owned(),
        })
        .collect()
}

fn expanded(indices: impl IntoIterator<Item = usize>) -> HashSet<usize> {
    indices.into_iter().collect()
}

#[test]
fn a_detail_row_follows_the_row_it_belongs_to() {
    let view = compute_view_with_details(&rows(), &columns(), &GridState::new(), &expanded([1]));

    assert_eq!(shape(&view), ["d0", "d1", "x1", "d2", "d3"]);
}

#[test]
fn a_detail_row_is_not_a_data_row() {
    let view = compute_view_with_details(&rows(), &columns(), &GridState::new(), &expanded([0, 2]));

    // Whatever walks the data rows meets each of them once, detail or not.
    assert_eq!(view.indices, [0, 1, 2, 3]);
    assert_eq!(view.data_rows().count(), 4);
    assert_eq!(view.data_index(1), None, "the detail of row 0");
    assert_eq!(view.row(1).unwrap().detail_index(), Some(0));
    assert_eq!(view.row(0).unwrap().detail_index(), None);
}

#[test]
fn a_detail_row_counts_among_the_rows() {
    let plain = compute_view_with_details(&rows(), &columns(), &GridState::new(), &expanded([]));
    let opened = compute_view_with_details(&rows(), &columns(), &GridState::new(), &expanded([1]));

    // What `aria-rowcount` counts grows by the row that is now there.
    assert_eq!(plain.row_count, 4);
    assert_eq!(opened.row_count, 5);
    // What was filtered does not: a detail row is not a result.
    assert_eq!(opened.filtered_len, 4);
}

#[test]
fn a_detail_follows_its_row_through_sorting() {
    let state = GridState {
        sort: vec![SortState::asc("city")],
        ..GridState::new()
    };
    let view = compute_view_with_details(&rows(), &columns(), &state, &expanded([2]));

    // Aachen, Berlin, Dresden, Kiel — and Kiel's detail is still under Kiel.
    assert_eq!(shape(&view), ["d1", "d0", "d3", "d2", "x2"]);
}

#[test]
fn a_detail_of_a_filtered_out_row_is_not_there() {
    let mut state = GridState::new();
    state.search = Some("ki".to_owned());
    let view = compute_view_with_details(&rows(), &columns(), &state, &expanded([0, 2]));

    // Only Kiel survives the search, so only its detail does.
    assert_eq!(shape(&view), ["d2", "x2"]);
}

#[test]
fn an_expanded_row_that_is_not_there_changes_nothing() {
    let view = compute_view_with_details(&rows(), &columns(), &GridState::new(), &expanded([99]));

    assert_eq!(shape(&view), ["d0", "d1", "d2", "d3"]);
}

#[test]
fn a_detail_row_takes_its_place_on_the_page() {
    let state = GridState {
        page: Some(PageState::new(2)),
        ..GridState::new()
    };
    let first = compute_view_with_details(&rows(), &columns(), &state, &expanded([0]));

    // Two rows to a page, and the detail is one of them.
    assert_eq!(shape(&first), ["d0", "x0"]);
    assert_eq!(first.row_count, 5);
    assert_eq!(first.page_count, 3);

    let second = GridState {
        page: Some(PageState { index: 1, size: 2 }),
        ..GridState::new()
    };
    let second = compute_view_with_details(&rows(), &columns(), &second, &expanded([0]));

    // The page after it starts where the first one stopped, detail included.
    assert_eq!(shape(&second), ["d1", "d2"]);
    assert_eq!(second.row_offset, 2);
}

#[test]
fn a_row_can_end_one_page_and_its_detail_start_the_next() {
    let state = |index: usize| GridState {
        page: Some(PageState { index, size: 2 }),
        ..GridState::new()
    };
    let first = compute_view_with_details(&rows(), &columns(), &state(0), &expanded([1]));
    let second = compute_view_with_details(&rows(), &columns(), &state(1), &expanded([1]));

    // The same rule groups already follow: a page holds what fits in it.
    assert_eq!(shape(&first), ["d0", "d1"]);
    assert_eq!(shape(&second), ["x1", "d2"]);
}
