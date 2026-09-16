//! Property-based tests for the invariants `PLAN.md` phase 1 requires.
//!
//! These check the properties that must hold for *any* input, not just the
//! cases a hand-written test happened to think of: sorting is a stable
//! permutation, filtering yields a subset, paging covers the filtered rows
//! without gaps or overlap, and `visible_range` stays in bounds.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{
    ColumnSpec, GridState, PageState, SortDirection, SortState, compute_view, visible_range,
};
use proptest::prelude::*;
use std::collections::HashSet;

/// A row with enough variety to exercise ties, cases and missing values.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    id: usize,
    label: String,
    score: Option<i64>,
}

fn columns() -> Vec<ColumnSpec<Row>> {
    vec![
        ColumnSpec::new("label")
            .sort_by_text(|row: &Row| row.label.as_str())
            .filter_by(|row: &Row| row.label.clone()),
        ColumnSpec::new("score").sort_by_value(|row: &Row| row.score),
    ]
}

/// Rows with short labels from a tiny alphabet, so ties happen often, and
/// scores that are frequently missing, so `None` ordering gets exercised.
fn arb_rows(max: usize) -> impl Strategy<Value = Vec<Row>> {
    prop::collection::vec(("[aAbB]{0,3}", prop::option::of(-3_i64..3)), 0..=max).prop_map(|items| {
        items
            .into_iter()
            .enumerate()
            .map(|(id, (label, score))| Row { id, label, score })
            .collect()
    })
}

/// A sort covering both columns, both directions, and the unsorted case.
fn arb_sort() -> impl Strategy<Value = Vec<SortState>> {
    let entry = (
        prop::sample::select(vec!["label", "score", "unknown-column"]),
        prop::sample::select(vec![SortDirection::Asc, SortDirection::Desc]),
    )
        .prop_map(|(column, direction)| SortState::new(column, direction));
    prop::collection::vec(entry, 0..3)
}

proptest! {
    /// Sorting must not invent, drop or duplicate rows.
    #[test]
    fn sorting_is_a_permutation_of_the_filtered_rows(
        rows in arb_rows(40),
        sort in arb_sort(),
    ) {
        let state = GridState { sort, ..GridState::new() };
        let view = compute_view(&rows, &columns(), &state);

        let mut got = view.indices.clone();
        got.sort_unstable();
        let expected: Vec<usize> = (0..rows.len()).collect();

        prop_assert_eq!(got, expected);
        prop_assert_eq!(view.filtered_len, rows.len());
    }

    /// Rows that compare equal on every sort column keep their original order.
    #[test]
    fn sorting_is_stable(rows in arb_rows(40)) {
        // Sort by label only, so every row sharing a label is a tie.
        let state = GridState {
            sort: vec![SortState::asc("label")],
            ..GridState::new()
        };
        let view = compute_view(&rows, &columns(), &state);

        for window in view.indices.windows(2) {
            let (left, right) = (window[0], window[1]);
            if rows[left].label == rows[right].label {
                prop_assert!(
                    left < right,
                    "tied rows {left} and {right} were reordered"
                );
            }
        }
    }

    /// Sorting really does produce a non-decreasing sequence.
    #[test]
    fn ascending_sort_is_ordered(rows in arb_rows(40)) {
        let state = GridState {
            sort: vec![SortState::asc("score")],
            ..GridState::new()
        };
        let view = compute_view(&rows, &columns(), &state);

        for window in view.indices.windows(2) {
            let left = rows[window[0]].score;
            let right = rows[window[1]].score;
            // `None` sorts last, so only compare when both are present.
            if let (Some(left), Some(right)) = (left, right) {
                prop_assert!(left <= right);
            }
            if left.is_none() {
                prop_assert!(right.is_none(), "a value followed a missing one");
            }
        }
    }

    /// Filtering may only ever remove rows, never reorder or invent them.
    #[test]
    fn filtering_yields_a_subset_in_input_order(
        rows in arb_rows(40),
        needle in "[aAbB]{0,2}",
        search in prop::option::of("[aAbB]{0,2}"),
    ) {
        let mut state = GridState::new();
        state.set_filter("label", needle);
        if let Some(search) = search {
            state.set_search(search);
        }

        let view = compute_view(&rows, &columns(), &state);

        prop_assert!(view.indices.len() <= rows.len());
        prop_assert_eq!(view.indices.len(), view.filtered_len);

        // Every index is valid and unique.
        let unique: HashSet<usize> = view.indices.iter().copied().collect();
        prop_assert_eq!(unique.len(), view.indices.len());
        prop_assert!(view.indices.iter().all(|&i| i < rows.len()));

        // Without a sort the surviving rows keep their input order.
        prop_assert!(view.indices.windows(2).all(|w| w[0] < w[1]));
    }

    /// Walking every page must reproduce the filtered rows exactly once.
    #[test]
    fn paging_covers_the_filtered_rows_without_gaps_or_overlap(
        rows in arb_rows(40),
        page_size in 1_usize..8,
        needle in "[aAbB]{0,2}",
        sort in arb_sort(),
    ) {
        let mut state = GridState {
            sort,
            page: Some(PageState::new(page_size)),
            ..GridState::new()
        };
        state.set_filter("label", needle);

        let first = compute_view(&rows, &columns(), &state);
        let filtered_len = first.filtered_len;
        prop_assert_eq!(first.page_count, filtered_len.div_ceil(page_size));

        let mut walked = Vec::new();
        for page in 0..first.page_count {
            state.set_page(page);
            let view = compute_view(&rows, &columns(), &state);

            prop_assert_eq!(view.filtered_len, filtered_len);
            prop_assert!(view.len() <= page_size);
            // Only the last page may be short.
            if page + 1 < first.page_count {
                prop_assert_eq!(view.len(), page_size);
            }
            walked.extend(view.indices);
        }

        prop_assert_eq!(walked.len(), filtered_len);

        // The concatenation of all pages is exactly the unpaged result.
        let unpaged = compute_view(
            &rows,
            &columns(),
            &GridState { page: None, ..state.clone() },
        );
        prop_assert_eq!(walked, unpaged.indices);
    }

    /// A page index past the end yields the last page, never nothing.
    #[test]
    fn an_out_of_range_page_is_clamped(
        rows in arb_rows(40),
        page_size in 1_usize..8,
        index in 0_usize..1000,
    ) {
        let mut state = GridState::paged(page_size);
        state.set_page(index);

        let view = compute_view(&rows, &columns(), &state);

        if view.filtered_len == 0 {
            prop_assert!(view.is_empty());
        } else {
            prop_assert!(!view.is_empty(), "a clamped page must still show rows");
            prop_assert!(view.len() <= page_size);
        }
    }

    /// The virtual window must stay inside the data, whatever the geometry.
    #[test]
    fn visible_range_stays_in_bounds(
        scroll_top in -10_000.0_f64..1_000_000.0,
        viewport_height in 0.0_f64..5_000.0,
        row_height in 0.0_f64..500.0,
        total_rows in 0_usize..100_000,
        overscan in 0_usize..50,
    ) {
        let range = visible_range(scroll_top, viewport_height, row_height, total_rows, overscan);

        prop_assert!(range.start <= range.end);
        prop_assert!(range.end <= total_rows);
    }

    /// The rendered window must actually cover the viewport, or the user sees
    /// blank space where rows should be.
    #[test]
    fn visible_range_covers_the_viewport(
        scroll_top in 0.0_f64..50_000.0,
        viewport_height in 1.0_f64..2_000.0,
        row_height in 1.0_f64..200.0,
        total_rows in 1_usize..10_000,
    ) {
        let range = visible_range(scroll_top, viewport_height, row_height, total_rows, 0);
        let total_height = total_rows as f64 * row_height;

        // Only meaningful where the viewport actually overlaps the data.
        if scroll_top < total_height && !range.is_empty() {
            let rendered_top = range.start as f64 * row_height;
            let rendered_bottom = range.end as f64 * row_height;

            prop_assert!(
                rendered_top <= scroll_top,
                "gap above: rendered from {rendered_top} but scrolled to {scroll_top}"
            );
            prop_assert!(
                rendered_bottom >= (scroll_top + viewport_height).min(total_height),
                "gap below: rendered to {rendered_bottom}"
            );
        }
    }
}
