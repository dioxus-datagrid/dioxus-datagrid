//! Server-side data: the query a grid sends, the page it gets back, and the
//! bookkeeping that keeps a slow, outdated response from overwriting a newer
//! one.
//!
//! Nothing here performs I/O or knows about time. A renderer turns state changes
//! into [`GridQuery`] values, asks a [`DataSource`] for a [`Page`], and uses
//! [`RequestTracker`] to decide whether an arriving response still matters.

use crate::{ColumnId, GridState, SortState};
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
    /// Column filters, each a case-insensitive substring by convention. Never
    /// contains empty filter text.
    pub column_filters: Vec<(ColumnId, String)>,
    /// Global search term, if any. Never empty.
    pub search: Option<String>,
    /// Zero-based page index.
    pub page: usize,
    /// Rows per page. Never zero.
    pub page_size: usize,
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
            search: state.search.clone().filter(|text| !text.is_empty()),
            page,
            page_size,
        }
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
    }

    /// Index of the first row on the requested page, across all pages.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.page.saturating_mul(self.page_size)
    }
}

/// One page of rows from a server, and how many rows match in total.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Page<T> {
    /// The rows of the requested page, already filtered and sorted.
    pub rows: Vec<T>,
    /// How many rows match the query across every page.
    pub total: usize,
}

impl<T> Page<T> {
    /// A page holding `rows` out of `total` matching rows.
    #[must_use]
    pub const fn new(rows: Vec<T>, total: usize) -> Self {
        Self { rows, total }
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
