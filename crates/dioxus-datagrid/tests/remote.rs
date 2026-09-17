//! `use_grid_remote` driven through a real `VirtualDom` on a Tokio runtime with
//! paused time, so latencies are simulated exactly and the tests run instantly.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use dioxus::core::NoOpMutations;
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{GridBody, GridHeader, GridRoot, GridStatus};
use dioxus_datagrid::{
    Column, DataSource, GridHandle, GridOptions, GridQuery, GridRow, Page, use_grid_remote,
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

thread_local! {
    /// Every query the source received, in order. Tokio's current-thread test
    /// runtime keeps each test on its own thread.
    static REQUESTS: RefCell<Vec<GridQuery>> = const { RefCell::new(Vec::new()) };
    /// How many upcoming requests should fail.
    static FAILURES: RefCell<usize> = const { RefCell::new(0) };
}

fn requests() -> Vec<GridQuery> {
    REQUESTS.with(|log| log.borrow().clone())
}

/// A server whose latency depends on the search term, so a test can make an
/// older request slower than a newer one.
struct Server;

impl DataSource<Item> for Server {
    type Error = String;

    async fn fetch(&self, query: GridQuery) -> Result<Page<Item>, String> {
        REQUESTS.with(|log| log.borrow_mut().push(query.clone()));
        let term = query.search.clone().unwrap_or_else(|| "all".to_owned());
        let latency = match term.as_str() {
            "slow" => 500,
            "fast" => 20,
            _ => 10,
        };
        sleep(Duration::from_millis(latency)).await;

        let fail = FAILURES.with(|failures| {
            let mut failures = failures.borrow_mut();
            let fail = *failures > 0;
            *failures = failures.saturating_sub(1);
            fail
        });
        if fail {
            return Err("server unavailable".to_owned());
        }

        let rows = (0..3)
            .map(|index| Item {
                id: index,
                label: format!("{term}-p{}-{index}", query.page),
            })
            .collect();
        Ok(Page::new(rows, 30))
    }
}

type Script = fn(GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>>;

#[derive(Clone, Props)]
struct HarnessProps {
    script: Script,
    debounce: Duration,
}

// Props need PartialEq. Comparing function pointers by address is what the
// derive would do, spelled out so it is deliberate.
impl PartialEq for HarnessProps {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::fn_addr_eq(self.script, other.script) && self.debounce == other.debounce
    }
}

#[component]
fn Harness(props: HarnessProps) -> Element {
    let columns = use_hook(|| {
        vec![
            Column::new("label", "Label")
                .cell(|item: &Item| rsx! { "{item.label}" })
                .sort_by_text(|item: &Item| item.label.as_str())
                .filter_by(|item: &Item| item.label.clone()),
        ]
    });
    let grid = use_grid_remote(
        Server,
        columns,
        GridOptions::paged(3).debounce(props.debounce),
    );

    let script = props.script;
    use_hook(move || spawn(script(grid)));

    rsx! {
        GridStatus { grid }
        GridRoot { grid,
            GridHeader { grid }
            GridBody { grid }
        }
    }
}

/// Runs the app for `duration` of simulated time and returns the final HTML.
async fn run(script: Script, debounce: Duration, duration: Duration) -> String {
    let mut dom = VirtualDom::new_with_props(Harness, HarnessProps { script, debounce });
    dom.rebuild_in_place();

    let deadline = Instant::now() + duration;
    loop {
        tokio::select! {
            () = dom.wait_for_work() => dom.render_immediate(&mut NoOpMutations),
            () = sleep_until(deadline) => break,
        }
    }
    dioxus_ssr::render(&dom)
}

fn nothing(_: GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>> {
    Box::pin(async {})
}

#[tokio::test(start_paused = true)]
async fn the_first_page_loads_on_mount() {
    let html = run(nothing, Duration::ZERO, Duration::from_millis(100)).await;

    assert_eq!(requests().len(), 1);
    assert_eq!(requests()[0].page_size, 3);
    assert!(html.contains(">all-p0-0<"), "{html}");
    // The server's total drives the counts, not the three rows received.
    assert!(html.contains(r#"aria-rowcount="31""#), "{html}");
    assert!(!html.contains("aria-busy"), "{html}");
}

/// Phase 6 acceptance: a slower response to an older query must not overwrite
/// the response to a newer one.
#[tokio::test(start_paused = true)]
async fn a_slower_older_response_does_not_overwrite_a_newer_one() {
    fn script(mut grid: GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            sleep(Duration::from_millis(50)).await;
            grid.set_search("slow"); // answers after 500ms
            sleep(Duration::from_millis(50)).await;
            grid.set_search("fast"); // answers after 20ms, long before "slow"
        })
    }

    // Long enough for the slow response to have arrived, had it not been dropped.
    let html = run(script, Duration::ZERO, Duration::from_secs(2)).await;

    let terms: Vec<_> = requests()
        .iter()
        .map(|query| query.search.clone())
        .collect();
    assert_eq!(
        terms,
        vec![None, Some("slow".to_owned()), Some("fast".to_owned())],
        "both requests went out"
    );
    assert!(html.contains(">fast-p0-0<"), "{html}");
    assert!(
        !html.contains("slow-"),
        "the stale response was applied: {html}"
    );
    assert!(!html.contains("aria-busy"), "{html}");
}

#[tokio::test(start_paused = true)]
async fn typing_is_debounced_into_one_request() {
    fn script(mut grid: GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            sleep(Duration::from_millis(50)).await;
            for term in ["a", "ab", "abc"] {
                grid.set_search(term);
                sleep(Duration::from_millis(100)).await;
            }
        })
    }

    let html = run(script, Duration::from_millis(300), Duration::from_secs(2)).await;

    let terms: Vec<_> = requests()
        .iter()
        .map(|query| query.search.clone())
        .collect();
    assert_eq!(terms, vec![None, Some("abc".to_owned())]);
    assert!(html.contains(">abc-p0-0<"), "{html}");
}

#[tokio::test(start_paused = true)]
async fn paging_loads_without_waiting_for_the_debounce() {
    fn script(mut grid: GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            sleep(Duration::from_millis(50)).await;
            grid.next_page();
        })
    }

    // Far shorter than the 300ms debounce: only an immediate request can finish.
    let html = run(
        script,
        Duration::from_millis(300),
        Duration::from_millis(120),
    )
    .await;

    assert_eq!(requests().len(), 2);
    assert_eq!(requests()[1].page, 1);
    assert!(html.contains(">all-p1-0<"), "{html}");
    // Page two of a three-row page starts at row 5, counting the header.
    assert!(html.contains(r#"aria-rowindex="5""#), "{html}");
}

#[tokio::test(start_paused = true)]
async fn a_failed_request_shows_the_error_and_reload_recovers() {
    FAILURES.with(|failures| *failures.borrow_mut() = 1);

    fn script(mut grid: GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            sleep(Duration::from_millis(100)).await;
            assert_eq!(grid.load_error().as_deref(), Some("server unavailable"));
            grid.reload();
        })
    }

    let html = run(script, Duration::ZERO, Duration::from_millis(300)).await;

    assert_eq!(requests().len(), 2, "reload repeats the request");
    assert!(html.contains(r#"data-state="idle""#), "{html}");
    assert!(html.contains(">all-p0-0<"), "{html}");
}

#[tokio::test(start_paused = true)]
async fn the_status_shows_the_error_with_a_retry_button() {
    FAILURES.with(|failures| *failures.borrow_mut() = 1);
    let html = run(nothing, Duration::ZERO, Duration::from_millis(100)).await;

    assert!(html.contains(r#"data-state="error""#), "{html}");
    assert!(html.contains("server unavailable"), "{html}");
    assert!(html.contains(">Retry</button>"), "{html}");
}

#[tokio::test(start_paused = true)]
async fn the_grid_is_busy_while_a_request_is_in_flight() {
    fn script(mut grid: GridHandle<Item>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move {
            sleep(Duration::from_millis(50)).await;
            grid.set_search("slow");
        })
    }

    // Stopped while "slow" is still on its way.
    let html = run(script, Duration::ZERO, Duration::from_millis(200)).await;

    assert!(html.contains(r#"aria-busy="true""#), "{html}");
    assert!(html.contains(r#"data-state="loading""#), "{html}");
    assert!(html.contains("Loading…"), "{html}");
    // The previous page stays visible meanwhile.
    assert!(html.contains(">all-p0-0<"), "{html}");
}
