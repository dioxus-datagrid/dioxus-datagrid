//! Turning rows plus state into the indices the renderer should draw.

use crate::column::{FilterTextFn, ValueFn};
use crate::{ColumnSpec, GridState, SortDirection, SortValue, TextCollation};
use std::cmp::Ordering;

/// The result of [`compute_view`].
///
/// Holds indices into the original row slice rather than cloned rows, so
/// computing a view stays cheap no matter how large or expensive `T` is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    /// Indices into the original rows, in display order, for the current page.
    pub indices: Vec<usize>,
    /// How many rows survived filtering, across every page.
    pub filtered_len: usize,
    /// How many pages the filtered rows span.
    ///
    /// `0` when paging is disabled or nothing matched.
    pub page_count: usize,
}

impl View {
    /// Whether the current page has no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// How many rows the current page holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.indices.len()
    }
}

/// Filters, sorts and pages `rows`, in that order.
///
/// # Filtering
///
/// A row must match **every** non-empty entry in
/// [`GridState::column_filters`] and, if set, the
/// [`search`](GridState::search) term. Matching is a case-insensitive substring
/// test against the column's
/// [`filter_text`](ColumnSpec::filter_text).
///
/// - Column filters apply whether or not the column is visible: the user set
///   them deliberately, so hiding a column does not silently widen the result.
/// - Search only scans *visible* filterable columns, so a hit is always
///   something the user can actually see. If no column is searchable at all,
///   the search term is ignored rather than hiding every row.
/// - Filters naming a column that does not exist are ignored, which keeps
///   persisted state usable against a changed column set.
///
/// # Sorting
///
/// Sorting is stable and follows [`GridState::sort`] in order of priority.
/// Entries naming an unknown or unsortable column are skipped. Rows that
/// compare equal keep their original relative order.
///
/// # Paging
///
/// A page index past the last page is clamped, so a stale index yields the last
/// page instead of nothing. A page size of `0` disables paging.
pub fn compute_view<T>(rows: &[T], columns: &[ColumnSpec<T>], state: &GridState) -> View {
    let mut indices = filter_indices(rows, columns, state);
    let filtered_len = indices.len();

    sort_indices(&mut indices, rows, columns, state);

    let page_count = match state.page {
        Some(page) if page.size > 0 => filtered_len.div_ceil(page.size),
        _ => 0,
    };

    if let Some(page) = state.page {
        if page.size > 0 {
            // Clamp rather than fail: the index may be left over from a wider
            // result set that a filter has since narrowed.
            let index = page.index.min(page_count.saturating_sub(1));
            let start = index.saturating_mul(page.size).min(indices.len());
            let end = start.saturating_add(page.size).min(indices.len());
            indices.truncate(end);
            indices.drain(..start);
        }
    }

    View {
        indices,
        filtered_len,
        page_count,
    }
}

/// Collects the indices of every row that passes the active filters.
fn filter_indices<T>(rows: &[T], columns: &[ColumnSpec<T>], state: &GridState) -> Vec<usize> {
    let active: Vec<(&FilterTextFn<T>, &str)> = state
        .column_filters
        .iter()
        .filter(|(_, needle)| !needle.is_empty())
        .filter_map(|(id, needle)| {
            let column = columns.iter().find(|column| &column.id == id)?;
            Some((column.filter_text.as_ref()?, needle.as_str()))
        })
        .collect();

    let search = state.search.as_deref().filter(|term| !term.is_empty());
    let searchable: Vec<&FilterTextFn<T>> = match search {
        Some(_) => columns
            .iter()
            .filter(|column| column.is_visible(&state.hidden_columns))
            .filter_map(|column| column.filter_text.as_ref())
            .collect(),
        None => Vec::new(),
    };

    // Nothing to search in means the term cannot be applied; showing every row
    // is less surprising than showing none.
    let search = search.filter(|_| !searchable.is_empty());

    if active.is_empty() && search.is_none() {
        return (0..rows.len()).collect();
    }

    rows.iter()
        .enumerate()
        .filter(|(_, row)| {
            let passes_columns = active
                .iter()
                .all(|(text, needle)| contains_ignore_case(&text(row), needle));

            let passes_search = search.is_none_or(|term| {
                searchable
                    .iter()
                    .any(|text| contains_ignore_case(&text(row), term))
            });

            passes_columns && passes_search
        })
        .map(|(index, _)| index)
        .collect()
}

/// Case-insensitive substring test.
///
/// Pure ASCII inputs — the overwhelming majority — take a non-allocating path.
/// Anything else falls back to full Unicode lowercasing, which has to allocate
/// because lowercasing can change a string's length.
fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }

    if haystack.is_ascii() && needle.is_ascii() {
        let needle = needle.as_bytes();
        return haystack
            .as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle));
    }

    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// Sorts `indices` in place according to [`GridState::sort`].
///
/// Sort keys are extracted once per row up front rather than on every
/// comparison, which turns `O(n log n)` closure calls into `O(n)`.
fn sort_indices<T>(
    indices: &mut [usize],
    rows: &[T],
    columns: &[ColumnSpec<T>],
    state: &GridState,
) {
    let plan: Vec<(&ValueFn<T>, SortDirection, TextCollation)> = state
        .sort
        .iter()
        .filter_map(|entry| {
            let column = columns.iter().find(|column| column.id == entry.column)?;
            let key = column.value.as_ref().filter(|_| column.sortable)?;
            Some((key, entry.direction, column.collation))
        })
        .collect();

    if plan.is_empty() || indices.len() < 2 {
        return;
    }

    // The comparator runs over a million times for a large grid, so it reads
    // from this compact two-byte-per-column array rather than re-walking `plan`,
    // which holds fat trait-object pointers it does not need.
    let settings: Vec<(SortDirection, TextCollation)> = plan
        .iter()
        .map(|(_, direction, collation)| (*direction, *collation))
        .collect();

    // Pack each row's keys next to its index so the sort moves whole records
    // through memory in order. Sorting a separate permutation instead would make
    // every comparison a random lookup into a buffer far larger than L2.
    let packed = match plan.len() {
        1 => sort_packed::<T, 1>(indices, rows, &plan, &settings),
        2 => sort_packed::<T, 2>(indices, rows, &plan, &settings),
        3 => sort_packed::<T, 3>(indices, rows, &plan, &settings),
        4 => sort_packed::<T, 4>(indices, rows, &plan, &settings),
        _ => false,
    };
    if packed {
        return;
    }

    // More sort columns than the packed cases cover, or more key text than the
    // arena can address. Rare enough that the extra indirection does not matter.
    let width = plan.len();
    let mut keys: Vec<SortValue> = Vec::with_capacity(indices.len() * width);
    for &index in indices.iter() {
        for (key, _, _) in &plan {
            keys.push(rows.get(index).map_or(SortValue::None, |row| key(row)));
        }
    }

    let mut order: Vec<usize> = (0..indices.len()).collect();

    order.sort_unstable_by(|&left, &right| {
        let left_keys = keys
            .get(left * width..left * width + width)
            .unwrap_or_default();
        let right_keys = keys
            .get(right * width..right * width + width)
            .unwrap_or_default();

        compare_keys(left_keys, right_keys, &settings).then_with(|| left.cmp(&right))
    });

    let original: Vec<usize> = indices.to_vec();
    for (slot, position) in indices.iter_mut().zip(order) {
        if let Some(&index) = original.get(position) {
            *slot = index;
        }
    }
}

/// Sorts by exactly `N` columns, keeping each row's keys and its index in one
/// record.
///
/// Sorting records rather than a permutation is what keeps this cache friendly:
/// the comparator reads keys straight out of the element the sort is already
/// moving, instead of chasing an index into a separate multi-megabyte buffer.
///
/// An earlier version also interned the key text into one contiguous arena, on
/// the theory that comparing text scattered across per-row allocations was
/// costing cache misses. Measured, it was 23% *slower*, and slower by a similar
/// margin even for purely numeric sorts that never touch text at all — which
/// says the sort is bound by per-comparison work rather than by where the keys
/// live. It was removed again; see ADR-0007.
fn sort_packed<T, const N: usize>(
    indices: &mut [usize],
    rows: &[T],
    plan: &[(&ValueFn<T>, SortDirection, TextCollation)],
    settings: &[(SortDirection, TextCollation)],
) -> bool {
    let mut records: Vec<([SortValue; N], usize)> = indices
        .iter()
        .map(|&index| {
            let keys = std::array::from_fn(|column| match (rows.get(index), plan.get(column)) {
                (Some(row), Some((key, _, _))) => key(row),
                _ => SortValue::None,
            });
            (keys, index)
        })
        .collect();

    // Ties fall back to the original index, which is the order filtering
    // produced. That makes the result stable while allowing the unstable sort,
    // which is faster and needs no temporary buffer.
    records.sort_unstable_by(|(left, left_index), (right, right_index)| {
        compare_keys(left, right, settings).then_with(|| left_index.cmp(right_index))
    });

    for (slot, (_, index)) in indices.iter_mut().zip(records) {
        *slot = index;
    }
    true
}

/// Compares two rows' sort keys column by column, applying each column's
/// direction to the first difference found.
fn compare_keys(
    left: &[SortValue],
    right: &[SortValue],
    settings: &[(SortDirection, TextCollation)],
) -> Ordering {
    for ((left, right), (direction, collation)) in left.iter().zip(right).zip(settings) {
        let ordering = left.cmp_with(right, *collation);
        if ordering != Ordering::Equal {
            return direction.apply(ordering);
        }
    }
    Ordering::Equal
}
