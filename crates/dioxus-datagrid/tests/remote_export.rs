//! `GridHandle::export_all` against a server that pages: every row, in order,
//! with the sort and search the grid shows, a limit, and an honest failure.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use datagrid_core::{ExportRow, ExportTable, SortDirection};
use dioxus::core::NoOpMutations;
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{GridBody, GridHeader, GridRoot};
use dioxus_datagrid::{
    Column, DataSource, EXPORT_PAGE_SIZE, ExportError, GridHandle, GridOptions, GridQuery, GridRow,
    Page, use_grid, use_grid_remote,
};
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tokio::time::{Instant, sleep, sleep_until};

#[derive(Clone, PartialEq)]
struct Item {
    id: u32,
    label: String,
}

impl GridRow for Item {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

const TOTAL: u32 = 2_500;

thread_local! {
    /// Every query the source received, in order.
    static REQUESTS: RefCell<Vec<GridQuery>> = const { RefCell::new(Vec::new()) };
    /// Fail the request with this ordinal (counting from 1), if set.
    static FAIL_ON: RefCell<Option<usize>> = const { RefCell::new(None) };
    /// What the script found out.
    static OUTCOME: RefCell<Option<Result<ExportTable, ExportError>>> = const { RefCell::new(None) };
}

/// The queries an export sent, which are the ones asking for a whole export
/// page, not the grid's own.
fn export_requests() -> Vec<GridQuery> {
    REQUESTS.with(|log| {
        log.borrow()
            .iter()
            .filter(|query| query.page_size == EXPORT_PAGE_SIZE)
            .cloned()
            .collect()
    })
}

fn outcome() -> Result<ExportTable, ExportError> {
    OUTCOME.with(|slot| slot.borrow().clone().expect("the script finished"))
}

/// Items 1 to 2500 with the label `item-<id>`, filtered by a substring search
/// and sorted by id, descending when asked.
struct Server;

impl DataSource<Item> for Server {
    type Error = String;

    async fn fetch(&self, query: GridQuery) -> Result<Page<Item>, String> {
        REQUESTS.with(|log| log.borrow_mut().push(query.clone()));
        sleep(Duration::from_millis(10)).await;

        if query.page_size == EXPORT_PAGE_SIZE {
            let ordinal = export_requests().len();
            if FAIL_ON.with(|slot| *slot.borrow()) == Some(ordinal) {
                return Err("server unavailable".to_owned());
            }
        }

        let mut items: Vec<Item> = (1..=TOTAL)
            .map(|id| Item {
                id,
                label: format!("item-{id}"),
            })
            .filter(|item| {
                query
                    .search
                    .as_ref()
                    .is_none_or(|term| item.label.contains(term.as_str()))
            })
            .collect();
        if query
            .sort
            .first()
            .is_some_and(|sort| sort.direction == SortDirection::Desc)
        {
            items.reverse();
        }
        let total = items.len();
        let rows = items
            .into_iter()
            .skip(query.offset())
            .take(query.page_size)
            .collect();
        Ok(Page::new(rows, total))
    }
}

type Script = fn(GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>>;

#[derive(Clone, Props)]
struct HarnessProps {
    script: Script,
    local: bool,
}

impl PartialEq for HarnessProps {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::fn_addr_eq(self.script, other.script) && self.local == other.local
    }
}

fn columns() -> Vec<Column<Item>> {
    vec![
        Column::new("label", "Label")
            .value_text(|item: &Item| item.label.as_str())
            .cell(|item: &Item| rsx! { "{item.label}" })
            .sort_by_text(|item: &Item| item.label.as_str())
            .filter_by(|item: &Item| item.label.clone()),
    ]
}

#[component]
fn Harness(props: HarnessProps) -> Element {
    let columns = use_hook(columns);
    let grid = if props.local {
        let rows = use_signal(|| {
            (1..=300)
                .map(|id| Item {
                    id,
                    label: format!("item-{id}"),
                })
                .collect::<Vec<_>>()
        });
        use_grid(rows, columns, GridOptions::paged(10))
    } else {
        use_grid_remote(
            Server,
            columns,
            GridOptions::paged(10).debounce(Duration::ZERO),
        )
    };

    let script = props.script;
    use_hook(move || spawn(script(grid)));

    rsx! {
        GridRoot { grid,
            GridHeader { grid }
            GridBody { grid }
        }
    }
}

async fn run(script: Script, local: bool) {
    REQUESTS.with(|log| log.borrow_mut().clear());
    FAIL_ON.with(|slot| *slot.borrow_mut() = None);
    OUTCOME.with(|slot| *slot.borrow_mut() = None);

    let mut dom = VirtualDom::new_with_props(Harness, HarnessProps { script, local });
    dom.rebuild_in_place();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        tokio::select! {
            () = dom.wait_for_work() => dom.render_immediate(&mut NoOpMutations),
            () = sleep_until(deadline) => break,
        }
    }
}

fn labels(table: &ExportTable) -> Vec<String> {
    table
        .rows
        .iter()
        .filter_map(|row| match row {
            ExportRow::Data(cells) => Some(cells[0].text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn takes_every_row_page_by_page_in_order() {
    fn script(grid: GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            sleep(Duration::from_millis(50)).await;
            let result = grid.export_all(10_000).await;
            OUTCOME.with(|slot| *slot.borrow_mut() = Some(result));
        })
    }
    run(script, false).await;

    let table = outcome().unwrap();
    let labels = labels(&table);
    assert_eq!(labels.len(), 2_500);
    assert_eq!(labels.first().map(String::as_str), Some("item-1"));
    assert_eq!(labels.last().map(String::as_str), Some("item-2500"));
    // Three requests of a thousand, pages 0 to 2.
    let pages: Vec<usize> = export_requests().iter().map(|query| query.page).collect();
    assert_eq!(pages, [0, 1, 2]);
}

#[tokio::test(start_paused = true)]
async fn asks_with_the_sort_and_the_search_the_grid_shows() {
    fn script(mut grid: GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            sleep(Duration::from_millis(50)).await;
            grid.set_search("item-12");
            grid.set_sort("label", SortDirection::Desc);
            sleep(Duration::from_millis(100)).await;
            let result = grid.export_all(10_000).await;
            OUTCOME.with(|slot| *slot.borrow_mut() = Some(result));
        })
    }
    run(script, false).await;

    let table = outcome().unwrap();
    let labels = labels(&table);
    // item-12, item-120..129, item-1200..1299, item-12xx up to 2500: 111 rows.
    assert_eq!(labels.len(), 1 + 10 + 100);
    // Descending, as the sort says.
    assert_eq!(labels.first().map(String::as_str), Some("item-1299"));
    assert_eq!(labels.last().map(String::as_str), Some("item-12"));

    let query = &export_requests()[0];
    assert_eq!(query.search.as_deref(), Some("item-12"));
    assert_eq!(query.page, 0);
}

#[tokio::test(start_paused = true)]
async fn asks_for_rows_flat_whatever_the_grid_groups_by() {
    fn script(mut grid: GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            sleep(Duration::from_millis(50)).await;
            grid.group_by_column("label", None);
            sleep(Duration::from_millis(100)).await;
            let result = grid.export_all(10_000).await;
            OUTCOME.with(|slot| *slot.borrow_mut() = Some(result));
        })
    }
    run(script, false).await;

    assert!(
        export_requests()
            .iter()
            .all(|query| query.group_by.is_empty())
    );
    assert_eq!(labels(&outcome().unwrap()).len(), 2_500);
}

#[tokio::test(start_paused = true)]
async fn refuses_more_rows_than_it_may_take_after_one_page() {
    fn script(grid: GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            sleep(Duration::from_millis(50)).await;
            let result = grid.export_all(2_000).await;
            OUTCOME.with(|slot| *slot.borrow_mut() = Some(result));
        })
    }
    run(script, false).await;

    assert_eq!(
        outcome().unwrap_err(),
        ExportError::TooMany {
            total: 2_500,
            max: 2_000
        }
    );
    // Found out from the first page: nothing more was asked for.
    assert_eq!(export_requests().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_failed_page_means_no_table_at_all() {
    fn script(grid: GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            sleep(Duration::from_millis(50)).await;
            FAIL_ON.with(|slot| *slot.borrow_mut() = Some(2));
            let result = grid.export_all(10_000).await;
            OUTCOME.with(|slot| *slot.borrow_mut() = Some(result));
        })
    }
    run(script, false).await;

    assert_eq!(
        outcome().unwrap_err(),
        ExportError::Source("server unavailable".to_owned())
    );
    assert_eq!(export_requests().len(), 2, "it stopped at the failure");
}

#[tokio::test(start_paused = true)]
async fn the_grid_changing_meanwhile_changes_nothing_about_the_export() {
    fn script(mut grid: GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            sleep(Duration::from_millis(50)).await;
            let export = grid.export_all(10_000);
            // The grid is searched after the first page has come back and before
            // the second is asked for.
            let search = async {
                sleep(Duration::from_millis(15)).await;
                grid.set_search("item-9");
            };
            let (result, ()) = tokio::join!(export, search);
            OUTCOME.with(|slot| *slot.borrow_mut() = Some(result));
        })
    }
    run(script, false).await;

    let searches: Vec<Option<String>> = export_requests()
        .iter()
        .map(|query| query.search.clone())
        .collect();
    assert_eq!(searches, [None, None, None]);
    assert_eq!(labels(&outcome().unwrap()).len(), 2_500);
}

#[tokio::test(start_paused = true)]
async fn a_grid_with_every_row_answers_without_a_server() {
    fn script(grid: GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            let result = grid.export_all(10).await;
            OUTCOME.with(|slot| *slot.borrow_mut() = Some(result));
        })
    }
    run(script, true).await;

    // All 300 rows, though the limit says 10: the limit is for what a server
    // would have to send, and these are here already.
    assert_eq!(labels(&outcome().unwrap()).len(), 300);
    assert!(REQUESTS.with(|log| log.borrow().is_empty()));
}
