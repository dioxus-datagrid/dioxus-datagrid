//! Turning rows plus state into the indices the renderer should draw.

use crate::column::{FilterTextFn, ValueFn};
use crate::filter::contains_ignore_case;
use crate::group::aggregate_all;
use crate::{
    AggregateValue, CellValue, ColumnFilter, ColumnId, ColumnSpec, Condition, GridState, Group,
    GroupKey, SortDirection, SortValue, TextCollation, Value,
};
use std::cmp::Ordering;
use std::collections::HashMap;

/// One row of a [`View`], in display order.
///
/// A view is more than its data rows: grouping adds rows of its own. Anything
/// that walks the rows on screen — rendering, keyboard navigation,
/// virtualization, `aria-rowindex` — walks [`View::rows`] and asks each row
/// what it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ViewRow {
    /// A data row: the index into the original rows.
    Data(usize),
    /// The row that heads a group and expands or collapses it: the index into
    /// [`View::groups`].
    GroupHeader(usize),
    /// The row below an expanded group's rows that shows its aggregates: the
    /// index into [`View::groups`]. Only there when a column has an aggregate.
    GroupFooter(usize),
}

impl ViewRow {
    /// The index into the original rows, for a data row.
    #[must_use]
    pub fn data_index(self) -> Option<usize> {
        match self {
            Self::Data(index) => Some(index),
            Self::GroupHeader(_) | Self::GroupFooter(_) => None,
        }
    }

    /// The index into [`View::groups`], for a group's header or footer.
    #[must_use]
    pub fn group_index(self) -> Option<usize> {
        match self {
            Self::GroupHeader(group) | Self::GroupFooter(group) => Some(group),
            Self::Data(_) => None,
        }
    }
}

/// The result of [`compute_view`].
///
/// Holds indices into the original row slice rather than cloned rows, so
/// computing a view stays cheap no matter how large or expensive `T` is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct View {
    /// The rows on the current page, in display order.
    pub rows: Vec<ViewRow>,
    /// Indices into the original rows, in display order, for the current page:
    /// the [`Data`](ViewRow::Data) rows of [`rows`](View::rows).
    pub indices: Vec<usize>,
    /// How many rows survived filtering, across every page.
    pub filtered_len: usize,
    /// How many pages the filtered rows span.
    ///
    /// `0` when paging is disabled or nothing matched.
    pub page_count: usize,
    /// How many rows the view has across every page, of every kind: what
    /// `aria-rowcount` counts, less the header.
    pub row_count: usize,
    /// Where the current page starts among those rows, zero-based: what
    /// `aria-rowindex` counts from.
    pub row_offset: usize,
    /// Every group, across every page, in display order; empty without
    /// grouping. [`ViewRow::GroupHeader`] and [`ViewRow::GroupFooter`] point
    /// in here.
    pub groups: Vec<Group>,
    /// How many columns the rows are grouped by: the depth of the data rows.
    pub group_levels: usize,
    /// The columns' aggregates over every filtered row, for the grid's footer.
    /// Empty when no column has one.
    pub totals: Vec<AggregateValue>,
}

impl View {
    /// A view of data rows only: `indices` is the page, starting `row_offset`
    /// rows into `filtered_len` filtered rows.
    ///
    /// This is what a view without grouping is, and what a remote grid builds
    /// from a page the server sent.
    #[must_use]
    pub fn of_data(
        indices: Vec<usize>,
        filtered_len: usize,
        page_count: usize,
        row_offset: usize,
    ) -> Self {
        Self {
            rows: indices.iter().copied().map(ViewRow::Data).collect(),
            indices,
            filtered_len,
            page_count,
            row_count: filtered_len,
            row_offset,
            groups: Vec::new(),
            group_levels: 0,
            totals: Vec::new(),
        }
    }

    /// The group a header or footer row at `position` on the current page
    /// belongs to.
    #[must_use]
    pub fn group_at(&self, position: usize) -> Option<&Group> {
        self.groups.get(self.row(position)?.group_index()?)
    }

    /// The position on the current page of the header of the group with
    /// `key`, if it is on this page.
    #[must_use]
    pub fn group_header_position(&self, key: &GroupKey) -> Option<usize> {
        self.rows.iter().position(|row| match row {
            ViewRow::GroupHeader(group) => self.groups.get(*group).is_some_and(|g| &g.key == key),
            _ => false,
        })
    }

    /// Whether the current page has no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// How many rows the current page holds, of every kind.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// The row at `position` on the current page.
    #[must_use]
    pub fn row(&self, position: usize) -> Option<ViewRow> {
        self.rows.get(position).copied()
    }

    /// The index into the original rows of the data row at `position` on the
    /// current page; `None` for a row of another kind.
    #[must_use]
    pub fn data_index(&self, position: usize) -> Option<usize> {
        self.row(position)?.data_index()
    }

    /// The data rows on the current page, in display order, each as its
    /// position on the page and its index into the original rows.
    pub fn data_rows(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.rows
            .iter()
            .enumerate()
            .filter_map(|(position, row)| Some((position, row.data_index()?)))
    }
}

/// Filters, sorts and pages `rows`, in that order.
///
/// # Filtering
///
/// A row must pass **every** non-empty entry in
/// [`GridState::column_filters`] and [`GridState::filters`] and, if set, the
/// [`search`](GridState::search) term.
///
/// - Filter bar text is read by [`Condition::from_text`]: `>100`, `=Berlin`,
///   `10..20`, or plain text. Plain text is a case-insensitive substring test
///   on text columns, and equality on number, date and boolean columns.
/// - Typed filters ([`ColumnFilter`]) compare the column's
///   [`value`](ColumnSpec::value); text operators use its
///   [`filter_text`](ColumnSpec::filter_text) when it has one.
/// - Search is a case-insensitive substring test on the filter text only.
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
/// # Grouping
///
/// With [`GridState::group_by`] set, rows are grouped by those columns,
/// outermost first; columns that do not exist or are not
/// [groupable](ColumnSpec::is_groupable) are skipped.
/// Groups follow their column's sort direction, ascending if it is not
/// sorted, and rows within a group follow the rest of the sort. Each group
/// adds a [`GroupHeader`](ViewRow::GroupHeader) row, and, if any column has
/// an [aggregate](ColumnSpec::aggregate), a [`GroupFooter`](ViewRow::GroupFooter)
/// after its rows. A collapsed group is its header alone.
///
/// # Paging
///
/// Pages count every row: headers and footers take a place on the page, and
/// a collapsed group takes one. A group can start on one page and end on the
/// next. A page index past the last page is clamped, so a stale index yields
/// the last page instead of nothing. A page size of `0` disables paging.
pub fn compute_view<T>(rows: &[T], columns: &[ColumnSpec<T>], state: &GridState) -> View {
    let mut indices = filter_indices(rows, columns, state);
    let filtered_len = indices.len();

    let grouped: Vec<&ColumnSpec<T>> = state
        .group_by
        .iter()
        .filter_map(|id| columns.iter().find(|column| &column.id == id))
        .filter(|column| column.is_groupable())
        .collect();

    sort_indices(&mut indices, rows, columns, state, &grouped);

    let aggregating = columns.iter().any(|column| !column.aggregates.is_empty());
    let totals = if aggregating {
        let all: Vec<&T> = indices
            .iter()
            .filter_map(|&index| rows.get(index))
            .collect();
        aggregate_all(columns, &all)
    } else {
        Vec::new()
    };

    let (all_rows, groups) = if grouped.is_empty() {
        (
            indices.iter().copied().map(ViewRow::Data).collect(),
            Vec::new(),
        )
    } else {
        let mut builder = GroupBuilder {
            rows,
            columns,
            state,
            grouped: &grouped,
            footers: aggregating,
            out: Vec::with_capacity(indices.len()),
            groups: Vec::new(),
        };
        builder.build(&indices, 0, &GroupKey::default());
        (builder.out, builder.groups)
    };

    let row_count = all_rows.len();
    let page_count = match state.page {
        Some(page) if page.size > 0 => row_count.div_ceil(page.size),
        _ => 0,
    };

    let mut page_rows = all_rows;
    let mut row_offset = 0;
    if let Some(page) = state.page {
        if page.size > 0 {
            // Clamp rather than fail: the index may be left over from a wider
            // result set that a filter has since narrowed.
            let index = page.index.min(page_count.saturating_sub(1));
            let start = index.saturating_mul(page.size).min(page_rows.len());
            let end = start.saturating_add(page.size).min(page_rows.len());
            page_rows.truncate(end);
            page_rows.drain(..start);
            row_offset = start;
        }
    }

    View {
        indices: page_rows
            .iter()
            .filter_map(|row| row.data_index())
            .collect(),
        rows: page_rows,
        filtered_len,
        page_count,
        row_count,
        row_offset,
        groups,
        group_levels: grouped.len(),
        totals,
    }
}

/// Turns sorted rows into group headers, data rows and group footers.
struct GroupBuilder<'a, T> {
    rows: &'a [T],
    columns: &'a [ColumnSpec<T>],
    state: &'a GridState,
    grouped: &'a [&'a ColumnSpec<T>],
    footers: bool,
    out: Vec<ViewRow>,
    groups: Vec<Group>,
}

impl<T> GroupBuilder<'_, T> {
    /// Groups `indices`, sorted so that equal values of the grouped column at
    /// `level` sit together, inside the group `parent`.
    fn build(&mut self, indices: &[usize], level: usize, parent: &GroupKey) {
        let Some(column) = self.grouped.get(level).copied() else {
            self.out.extend(indices.iter().copied().map(ViewRow::Data));
            return;
        };

        let runs = runs_of_equal(
            indices,
            |index| {
                self.rows
                    .get(index)
                    .map_or(CellValue::None, |row| column.read(row))
            },
            column.collation,
        );
        let siblings = runs.len();

        for (position, run) in runs.into_iter().enumerate() {
            let value = run
                .first()
                .and_then(|&index| self.rows.get(index))
                .and_then(|row| Value::from_cell(&column.read(row)));
            let key = parent.child(value.clone());
            let expanded = self.state.is_group_expanded(&key);
            let members: Vec<&T> = run
                .iter()
                .filter_map(|&index| self.rows.get(index))
                .collect();
            let group = self.groups.len();
            self.groups.push(Group {
                key: key.clone(),
                column: column.id.clone(),
                value,
                level,
                count: run.len(),
                expanded,
                position: position + 1,
                siblings,
                aggregates: aggregate_all(self.columns, &members),
            });

            self.out.push(ViewRow::GroupHeader(group));
            if expanded {
                self.build(run, level + 1, &key);
                if self.footers {
                    self.out.push(ViewRow::GroupFooter(group));
                }
            }
        }
    }
}

/// Splits `indices` into the runs whose values compare equal.
fn runs_of_equal<'r, 'v>(
    indices: &'r [usize],
    value: impl Fn(usize) -> CellValue<'v>,
    collation: TextCollation,
) -> Vec<&'r [usize]> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut current: Option<CellValue<'v>> = None;
    for (position, &index) in indices.iter().enumerate() {
        let next = value(index);
        if let Some(previous) = current {
            if previous.cmp_with(&next, collation) != Ordering::Equal {
                if let Some(run) = indices.get(start..position) {
                    runs.push(run);
                }
                start = position;
            }
        }
        current = Some(next);
    }
    if let Some(run) = indices.get(start..).filter(|run| !run.is_empty()) {
        runs.push(run);
    }
    runs
}

/// Collects the indices of every row that passes the active filters.
fn filter_indices<T>(rows: &[T], columns: &[ColumnSpec<T>], state: &GridState) -> Vec<usize> {
    filter_indices_except(rows, columns, state, None)
}

/// [`filter_indices`], leaving out the filters on `except`.
fn filter_indices_except<T>(
    rows: &[T],
    columns: &[ColumnSpec<T>],
    state: &GridState,
    except: Option<&ColumnId>,
) -> Vec<usize> {
    let active = active_filters(rows, columns, state, except);

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
                .all(|(column, filter)| passes(*row, column, filter));

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

/// The filters in effect, each paired with its column and prepared for the
/// column's kind: the filter bar's text read by [`ColumnFilter::from_bar_text`],
/// then the typed filters. `except` leaves one column's filters out, which is
/// what a value list needs to show the values its own filter would hide.
///
/// Filters naming an unknown or unfilterable column are dropped.
pub(crate) fn active_filters<'c, T>(
    rows: &[T],
    columns: &'c [ColumnSpec<T>],
    state: &GridState,
    except: Option<&ColumnId>,
) -> Vec<(&'c ColumnSpec<T>, ColumnFilter)> {
    let find = |id: &ColumnId| {
        columns
            .iter()
            .find(|column| &column.id == id)
            .filter(|column| column.is_filterable() && except != Some(id))
    };

    let from_bar = state.column_filters.iter().filter_map(|(id, text)| {
        let column = find(id)?;
        let filter = match column.value_kind(rows) {
            Some(kind) => ColumnFilter::from_bar_text(text, kind)?.prepared(kind),
            None => ColumnFilter::new(Condition::from_text(text)?),
        };
        Some((column, filter))
    });
    let typed = state
        .filters
        .iter()
        .filter(|(_, filter)| !filter.is_empty())
        .filter_map(|(id, filter)| {
            let column = find(id)?;
            let filter = match column.value_kind(rows) {
                Some(kind) => filter.prepared(kind),
                None => filter.clone(),
            };
            Some((column, filter))
        });

    from_bar.chain(typed).collect()
}

/// Whether `row` passes one column's filter.
pub(crate) fn passes<T>(row: &T, column: &ColumnSpec<T>, filter: &ColumnFilter) -> bool {
    let value = column.read(row);
    let text = column.filter_text.as_ref().map(|text| text(row));
    filter.matches(&value, text.as_deref())
}

/// Sorts `indices` in place according to [`GridState::sort`].
///
/// Sort keys are extracted once per row up front rather than on every
/// comparison, which turns `O(n log n)` closure calls into `O(n)`.
///
/// Grouped columns come first, so that each group's rows end up together: in
/// the direction the column is sorted in, ascending if it is not. They sort
/// whether or not the column is sortable, since grouping needs the order.
fn sort_indices<T>(
    indices: &mut [usize],
    rows: &[T],
    columns: &[ColumnSpec<T>],
    state: &GridState,
    grouped: &[&ColumnSpec<T>],
) {
    let groups = grouped.iter().filter_map(|column| {
        let direction = state.sort_direction(&column.id).unwrap_or_default();
        Some((column.value.as_ref()?, direction, column.collation))
    });
    let sorts = state
        .sort
        .iter()
        .filter(|entry| !grouped.iter().any(|column| column.id == entry.column))
        .filter_map(|entry| {
            let column = columns.iter().find(|column| column.id == entry.column)?;
            let key = column.value.as_ref().filter(|_| column.sortable)?;
            Some((key, entry.direction, column.collation))
        });
    let plan: Vec<(&ValueFn<T>, SortDirection, TextCollation)> = groups.chain(sorts).collect();

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

/// The values of one column, each with how many rows hold it: what a value
/// list in a filter menu offers to tick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct DistinctValues {
    /// Each value once, in sort order, with its row count.
    pub values: Vec<(Value, usize)>,
    /// How many rows have no value.
    pub empty: usize,
    /// Whether there were more distinct values than the limit allowed; the
    /// list then holds the first ones in sort order.
    pub truncated: bool,
}

/// The distinct values of `column` among the rows that pass every *other*
/// filter and the search.
///
/// Leaving the column's own filter out is what makes a value list useful: a
/// value the user unticked stays in the list, ready to be ticked again, while
/// values that other filters exclude disappear. At most `limit` values are
/// returned. `None` if the column does not exist or has no value.
///
/// Text is distinct as written: `Berlin` and `berlin` are two entries, since
/// the list shows them and a user may want either.
#[must_use]
pub fn distinct_values<T>(
    rows: &[T],
    columns: &[ColumnSpec<T>],
    state: &GridState,
    column: &ColumnId,
    limit: usize,
) -> Option<DistinctValues> {
    let spec = columns.iter().find(|spec| &spec.id == column)?;
    let value = spec.value.as_ref()?;

    let mut counts: HashMap<Value, usize> = HashMap::new();
    let mut empty = 0;
    for index in filter_indices_except(rows, columns, state, Some(column)) {
        let Some(row) = rows.get(index) else {
            continue;
        };
        match Value::from_cell(&value(row)) {
            Some(value) => *counts.entry(value).or_insert(0) += 1,
            None => empty += 1,
        }
    }

    let mut values: Vec<(Value, usize)> = counts.into_iter().collect();
    values.sort_by(|(a, _), (b, _)| a.as_cell().cmp_with(&b.as_cell(), spec.collation));
    let truncated = values.len() > limit;
    values.truncate(limit);

    Some(DistinctValues {
        values,
        empty,
        truncated,
    })
}
