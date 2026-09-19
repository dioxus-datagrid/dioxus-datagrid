//! Server-side data: the query a grid sends, the page it gets back, and the
//! bookkeeping that keeps a slow, outdated response from overwriting a newer
//! one.
//!
//! Nothing here performs I/O or knows about time. A renderer turns state changes
//! into [`GridQuery`] values, asks a [`DataSource`] for a [`Page`], and uses
//! [`RequestTracker`] to decide whether an arriving response still matters.

use crate::{
    AggregateKind, AggregateValue, ColumnFilter, ColumnId, ColumnSpec, DistinctValues, GridState,
    Group, GroupKey, PageState, SortState, View, ViewRow,
};
use std::collections::HashMap;
use std::fmt::Display;
use std::future::Future;

/// Rows per page when a remote grid is not given a page size. A server must
/// page, so "no paging" is not an option there.
pub const DEFAULT_REMOTE_PAGE_SIZE: usize = 25;

/// Everything a server needs to produce one page of rows.
///
/// Derived from [`GridState`] with [`GridQuery::from_state`]. Two queries that
/// compare equal ask for the same rows, which is what lets a grid skip requests
/// that would change nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GridQuery {
    /// Sort order, highest priority first.
    pub sort: Vec<SortState>,
    /// Filter bar text per column. Never empty. Read it with
    /// [`Condition::from_text`](crate::Condition::from_text) to do what the
    /// grid does locally: `>100`, `10..20`, `=Berlin`, or a substring test.
    pub column_filters: Vec<(ColumnId, String)>,
    /// Typed filters per column, from a filter menu. A row must pass these as
    /// well as [`column_filters`](GridQuery::column_filters). Absent from
    /// queries of grids before 0.6.0, and then empty.
    #[cfg_attr(feature = "serde", serde(default))]
    pub filters: Vec<(ColumnId, ColumnFilter)>,
    /// Global search term, if any. Never empty.
    pub search: Option<String>,
    /// Zero-based page index.
    pub page: usize,
    /// Rows per page. Never zero. With grouping, group headers and footers
    /// count as rows, as they do locally.
    pub page_size: usize,
    /// The columns to group by, outermost first. Empty for no grouping, and
    /// absent from queries of grids before 0.8.0.
    #[cfg_attr(feature = "serde", serde(default))]
    pub group_by: Vec<ColumnId>,
    /// Whether groups start collapsed.
    #[cfg_attr(feature = "serde", serde(default))]
    pub groups_collapsed: bool,
    /// The groups expanded or collapsed against
    /// [`groups_collapsed`](GridQuery::groups_collapsed).
    #[cfg_attr(feature = "serde", serde(default))]
    pub toggled_groups: Vec<GroupKey>,
    /// The aggregates the grid shows, per column: for group footers and the
    /// totals. Custom aggregates are named by their label; a server that does
    /// not know one leaves it out.
    #[cfg_attr(feature = "serde", serde(default))]
    pub aggregates: Vec<(ColumnId, AggregateKind)>,
}

impl GridQuery {
    /// The query that shows `state`.
    ///
    /// The page size comes from the state's paging, or `default_page_size` when
    /// the state does not page. A size of zero, from either, becomes
    /// [`DEFAULT_REMOTE_PAGE_SIZE`]. Empty filters and an empty search are left
    /// out, so they cannot make otherwise identical queries differ.
    #[must_use]
    pub fn from_state(state: &GridState, default_page_size: usize) -> Self {
        let (page, size) = state
            .page
            .map_or((0, default_page_size), |page| (page.index, page.size));
        let page_size = if size == 0 {
            DEFAULT_REMOTE_PAGE_SIZE
        } else {
            size
        };

        Self {
            sort: state.sort.clone(),
            column_filters: state
                .column_filters
                .iter()
                .filter(|(_, text)| !text.is_empty())
                .cloned()
                .collect(),
            filters: state
                .filters
                .iter()
                .filter(|(_, filter)| !filter.is_empty())
                .cloned()
                .collect(),
            search: state.search.clone().filter(|text| !text.is_empty()),
            page,
            page_size,
            group_by: state.group_by.clone(),
            groups_collapsed: state.groups_collapsed,
            toggled_groups: state.toggled_groups.clone(),
            aggregates: Vec::new(),
        }
    }

    /// Asks for the aggregates `columns` define. The grid does this itself;
    /// [`from_state`](GridQuery::from_state) cannot, since state holds no
    /// columns.
    #[must_use]
    pub fn with_aggregates<T>(mut self, columns: &[ColumnSpec<T>]) -> Self {
        self.aggregates = columns
            .iter()
            .flat_map(|column| {
                column
                    .aggregates
                    .iter()
                    .map(|aggregate| (column.id.clone(), aggregate.kind()))
            })
            .collect();
        self
    }

    /// The grid state this query shows, for a server that answers with
    /// [`compute_view`](crate::compute_view) over rows it holds in memory.
    #[must_use]
    pub fn state(&self) -> GridState {
        GridState {
            sort: self.sort.clone(),
            column_filters: self.column_filters.clone(),
            filters: self.filters.clone(),
            search: self.search.clone(),
            page: Some(PageState {
                index: self.page,
                size: self.page_size,
            }),
            group_by: self.group_by.clone(),
            groups_collapsed: self.groups_collapsed,
            toggled_groups: self.toggled_groups.clone(),
            ..GridState::default()
        }
    }

    /// Whether the group with `key` shows its rows.
    #[must_use]
    pub fn is_group_expanded(&self, key: &GroupKey) -> bool {
        self.groups_collapsed == self.toggled_groups.contains(key)
    }

    /// Whether moving from `previous` to `self` changed only what the user
    /// types — search or column filters.
    ///
    /// Those arrive a keystroke at a time and are worth debouncing. A change to
    /// sort or page is a single deliberate action and should load at once, even
    /// if the text changed along with it.
    #[must_use]
    pub fn is_typing_change(&self, previous: &Self) -> bool {
        let text_changed =
            self.search != previous.search || self.column_filters != previous.column_filters;
        text_changed
            && self.sort == previous.sort
            && self.page == previous.page
            && self.page_size == previous.page_size
            && self.group_by == previous.group_by
            && self.groups_collapsed == previous.groups_collapsed
            && self.toggled_groups == previous.toggled_groups
    }

    /// Index of the first row on the requested page, across all pages.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.page.saturating_mul(self.page_size)
    }
}

/// One page of rows from a server, and how many rows match in total.
///
/// Without grouping, `rows` and `total` are all there is. A grouped page also
/// says where the group headers and footers go, in `layout`; build it with
/// [`Page::from_view`] or [`GroupedPage::into_page`](crate::GroupedPage::into_page).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Page<T> {
    /// The rows of the requested page, already filtered and sorted.
    pub rows: Vec<T>,
    /// How many rows match the query across every page.
    pub total: usize,
    /// The page in display order, when grouped: [`ViewRow::Data`] indexes
    /// `rows`, and the group rows index `groups`. Empty when every row of
    /// the page is a data row, which is also what servers before 0.8.0 send.
    #[cfg_attr(feature = "serde", serde(default))]
    pub layout: Vec<ViewRow>,
    /// The groups whose header or footer is on this page.
    #[cfg_attr(feature = "serde", serde(default))]
    pub groups: Vec<Group>,
    /// How many rows of every kind the query spans across every page, when
    /// that differs from `total` because of grouping.
    #[cfg_attr(feature = "serde", serde(default))]
    pub row_count: Option<usize>,
    /// The aggregates over every matching row that the query asked for.
    #[cfg_attr(feature = "serde", serde(default))]
    pub totals: Vec<AggregateValue>,
}

impl<T> Page<T> {
    /// A page holding `rows` out of `total` matching rows.
    #[must_use]
    pub const fn new(rows: Vec<T>, total: usize) -> Self {
        Self {
            rows,
            total,
            layout: Vec::new(),
            groups: Vec::new(),
            row_count: None,
            totals: Vec::new(),
        }
    }

    /// Adds the aggregates over every matching row.
    #[must_use]
    pub fn with_totals(mut self, totals: Vec<AggregateValue>) -> Self {
        self.totals = totals;
        self
    }

    /// The page a [`View`] shows, for a server that computes views over rows
    /// it holds in memory: the view's rows cloned, with its groups, row count
    /// and totals.
    #[must_use]
    pub fn from_view(view: &View, rows: &[T]) -> Self
    where
        T: Clone,
    {
        let mut page_rows = Vec::with_capacity(view.indices.len());
        let mut groups: Vec<Group> = Vec::new();
        let mut renumbered: HashMap<usize, usize> = HashMap::new();
        let mut layout = Vec::with_capacity(view.rows.len());
        for row in &view.rows {
            match *row {
                ViewRow::Data(index) => {
                    if let Some(row) = rows.get(index) {
                        layout.push(ViewRow::Data(page_rows.len()));
                        page_rows.push(row.clone());
                    }
                }
                ViewRow::GroupHeader(group) | ViewRow::GroupFooter(group) => {
                    let Some(spec) = view.groups.get(group) else {
                        continue;
                    };
                    let index = *renumbered.entry(group).or_insert_with(|| {
                        groups.push(spec.clone());
                        groups.len() - 1
                    });
                    layout.push(if matches!(row, ViewRow::GroupHeader(_)) {
                        ViewRow::GroupHeader(index)
                    } else {
                        ViewRow::GroupFooter(index)
                    });
                }
            }
        }
        let grouped = view.group_levels > 0;
        Self {
            rows: page_rows,
            total: view.filtered_len,
            layout: if grouped { layout } else { Vec::new() },
            groups,
            row_count: grouped.then_some(view.row_count),
            totals: view.totals.clone(),
        }
    }

    /// How many pages `total` rows span at `page_size`; `0` when nothing
    /// matches.
    #[must_use]
    pub const fn page_count(&self, page_size: usize) -> usize {
        if page_size == 0 {
            0
        } else {
            self.total.div_ceil(page_size)
        }
    }
}

/// Where a remote grid gets its rows from.
///
/// Implement it for an HTTP client, a server function, a database handle —
/// anything that can answer a [`GridQuery`]. The future is not required to be
/// `Send`: a Dioxus `VirtualDom` is single-threaded, and WASM has no threads.
///
/// ```
/// use datagrid_core::{DataSource, GridQuery, Page};
///
/// struct Numbers;
///
/// impl DataSource<u32> for Numbers {
///     type Error = String;
///
///     async fn fetch(&self, query: GridQuery) -> Result<Page<u32>, String> {
///         let all: Vec<u32> = (0..1_000).collect();
///         let rows = all.iter().copied().skip(query.offset()).take(query.page_size).collect();
///         Ok(Page::new(rows, all.len()))
///     }
/// }
/// ```
pub trait DataSource<T> {
    /// What a failed request reports. Shown to the user, so make it readable.
    type Error: Display;

    /// Loads the page `query` describes.
    fn fetch(&self, query: GridQuery) -> impl Future<Output = Result<Page<T>, Self::Error>>;

    /// The distinct values of `column` for a value list: among the rows that
    /// pass every filter in `query` except the column's own, at most `limit`
    /// of them. [`distinct_values`](crate::distinct_values) is what a local
    /// grid does.
    ///
    /// The default answers with an empty list, which a filter menu shows as
    /// having no value list.
    fn distinct_values(
        &self,
        column: ColumnId,
        query: GridQuery,
        limit: usize,
    ) -> impl Future<Output = Result<DistinctValues, Self::Error>> {
        let _ = (column, query, limit);
        async { Ok(DistinctValues::default()) }
    }
}

/// Identifies one request issued through a [`RequestTracker`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RequestId(u64);

/// Decides whether a response is still wanted.
///
/// Requests can finish in any order: a search for "a" may take longer than the
/// search for "ab" typed right after it. Only the response to the most recently
/// issued request may be applied; everything older is stale, however late it
/// arrives.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequestTracker {
    latest: u64,
}

impl RequestTracker {
    /// A tracker that has issued no requests yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { latest: 0 }
    }

    /// Issues a new request, making every earlier one stale.
    pub const fn begin(&mut self) -> RequestId {
        self.latest = self.latest.wrapping_add(1);
        RequestId(self.latest)
    }

    /// Whether `id` is the most recently issued request, so its response should
    /// be applied.
    #[must_use]
    pub const fn is_latest(&self, id: RequestId) -> bool {
        id.0 == self.latest
    }
}
