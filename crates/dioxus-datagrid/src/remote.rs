//! The `use_grid_remote` hook: a grid whose rows come from a server.

use crate::grid::{DistinctSource, GridHandle, GridOptions, IntoReadSignal, use_grid_base};
use crate::{Column, DEFAULT_DEBOUNCE, timer};
use datagrid_core::{
    DEFAULT_REMOTE_PAGE_SIZE, DataSource, GridQuery, GridRow, RequestTracker, View,
};
use dioxus::core::Task;
use dioxus::prelude::*;
use std::rc::Rc;

/// Creates a grid whose rows come from `source`, one page at a time.
///
/// Returns the same [`GridHandle`] as [`use_grid`](crate::use_grid), so every
/// primitive works unchanged. The difference is where sorting, filtering,
/// search and paging happen: each change to the grid's state becomes a
/// [`GridQuery`], and `source` answers it with a [`Page`](datagrid_core::Page).
///
/// - **Paging is always on.** Without a page size in `options`,
///   [`DEFAULT_REMOTE_PAGE_SIZE`] is used.
/// - **Typing is debounced.** A change to search or a column filter waits for
///   typing to pause — [`DEFAULT_DEBOUNCE`] unless [`GridOptions::debounce`]
///   says otherwise. Sorting and paging load at once.
/// - **Stale responses are dropped.** A new request cancels the one in flight,
///   and a response is only applied if no newer request was issued since, so a
///   slow answer to an old query never overwrites a newer one.
/// - **Loading and errors** are exposed through
///   [`is_loading`](GridHandle::is_loading), [`load_error`](GridHandle::load_error)
///   and [`reload`](GridHandle::reload), and rendered by
///   [`GridStatus`](crate::primitives::GridStatus). The previous page stays
///   visible while the next one loads.
///
/// Columns still need sort and filter closures to be offered as sortable or
/// filterable; the grid does not call them, the server does the work.
///
/// `source` is captured on the first render. To query something else, render
/// the grid again under a different `key`.
///
/// The debounce needs a timer: `gloo-timers` on the web and Tokio elsewhere, as
/// every Dioxus renderer provides. With a zero debounce no timer is used.
///
/// ```
/// use datagrid_core::{DataSource, GridQuery, GridRow, Page};
/// use dioxus::prelude::*;
/// use dioxus_datagrid::primitives::{GridBody, GridHeader, GridPagination, GridRoot, GridStatus};
/// use dioxus_datagrid::{Column, GridOptions, use_grid_remote};
///
/// #[derive(Clone, PartialEq)]
/// struct User {
///     id: u32,
///     name: String,
/// }
///
/// impl GridRow for User {
///     type Key = u32;
///     fn key(&self) -> u32 {
///         self.id
///     }
/// }
///
/// struct Api;
///
/// impl DataSource<User> for Api {
///     type Error = String;
///
///     async fn fetch(&self, query: GridQuery) -> Result<Page<User>, String> {
///         // Send `query` to your server here.
///         let _ = query;
///         Ok(Page::new(Vec::new(), 0))
///     }
/// }
///
/// #[component]
/// fn Users() -> Element {
///     let columns = use_hook(|| {
///         vec![
///             Column::new("name", "Name")
///                 .cell(|user: &User| rsx! { "{user.name}" })
///                 .sort_by_text(|user: &User| user.name.as_str()),
///         ]
///     });
///     let grid = use_grid_remote(Api, columns, GridOptions::paged(50));
///
///     rsx! {
///         GridStatus { grid }
///         GridRoot { grid,
///             GridHeader { grid }
///             GridBody { grid }
///         }
///         GridPagination { grid }
///     }
/// }
/// ```
pub fn use_grid_remote<T, S>(
    source: S,
    columns: impl IntoReadSignal<Vec<Column<T>>> + 'static,
    options: GridOptions,
) -> GridHandle<T>
where
    T: GridRow + PartialEq + 'static,
    S: DataSource<T> + 'static,
{
    let source = use_hook(move || Rc::new(source));
    let columns = use_hook(move || columns.into_read_signal());

    let page_size = options.page_size.unwrap_or(DEFAULT_REMOTE_PAGE_SIZE);
    let debounce = options.debounce.unwrap_or(DEFAULT_DEBOUNCE);
    let options = GridOptions {
        page_size: Some(page_size),
        ..options
    };

    let mut rows = use_signal(Vec::<T>::new);
    let mut total = use_signal(|| 0_usize);
    let data = use_hook(move || ReadSignal::new(rows));

    // Value lists ask the same source, type-erased so the handle stays generic
    // over the row type only.
    let distinct = {
        let source = Rc::clone(&source);
        let answer: DistinctSource = Rc::new(move |column, query, limit| {
            let source = Rc::clone(&source);
            Box::pin(async move {
                source
                    .distinct_values(column, query, limit)
                    .await
                    .map_err(|error| error.to_string())
            })
        });
        answer
    };
    let base = use_grid_base(data, columns, options, Some(distinct));
    let mut state = base.state;
    let mut loading = base.loading;
    let mut load_error = base.load_error;
    let reload_nonce = base.reload_nonce;

    // The server already filtered, sorted and paged: the view is the page as
    // received, with the server's total standing in for the filtered length.
    let view = use_memo(move || {
        let received = rows.read().len();
        let total = total();
        let size = state.read().page.map_or(page_size, |page| page.size).max(1);
        View {
            indices: (0..received).collect(),
            filtered_len: total,
            page_count: total.div_ceil(size),
        }
    });

    let query = use_memo(move || GridQuery::from_state(&state.read(), page_size));
    let mut tracker = use_signal(RequestTracker::new);
    let mut sent = use_signal(|| None::<GridQuery>);
    let mut in_flight = use_signal(|| None::<Task>);

    use_effect(move || {
        let query = query();
        // Read so that `reload` re-runs this effect with an unchanged query.
        let _ = reload_nonce();

        let wait = sent
            .peek()
            .as_ref()
            .is_some_and(|previous| query.is_typing_change(previous));
        sent.set(Some(query.clone()));

        let id = tracker.write().begin();
        // Dropping the old future aborts its request where the source supports
        // that. The tracker check below covers work that cannot be cancelled.
        if let Some(previous) = in_flight.write().take() {
            previous.cancel();
        }

        let source = Rc::clone(&source);
        let task = spawn(async move {
            if wait && !debounce.is_zero() {
                timer::sleep(debounce).await;
                if !tracker.peek().is_latest(id) {
                    return;
                }
            }

            loading.set(true);
            let result = source.fetch(query.clone()).await;
            if !tracker.peek().is_latest(id) {
                return;
            }

            match result {
                Ok(page) => {
                    let last_page = page.page_count(query.page_size).saturating_sub(1);
                    let past_the_end = page.rows.is_empty() && query.page > last_page;
                    rows.set(page.rows);
                    total.set(page.total);
                    load_error.set(None);
                    // The data shrank, and the requested page no longer exists.
                    // Go to the last one that does, which is a new query.
                    if past_the_end && page.total > 0 {
                        state.write().set_page(last_page);
                    }
                }
                Err(error) => load_error.set(Some(error.to_string())),
            }
            loading.set(false);
        });
        in_flight.set(Some(task));
    });

    base.with_view(view)
}
