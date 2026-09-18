//! The filter menu as rendered, typed filters through the handle, and value
//! lists from a local grid and from a data source.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{DataSource, GridQuery, GridRow, Page};
use dioxus::core::NoOpMutations;
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{GridBody, GridFilterMenu, GridHeader, GridRoot};
use dioxus_datagrid::{
    Column, ColumnFilter, ColumnId, Condition, DistinctValues, FilterValue, GridLocale,
    GridOptions, use_grid, use_grid_remote,
};
use std::time::Duration;
use tokio::time::{Instant, sleep_until};

#[derive(Clone, PartialEq)]
struct City {
    id: u32,
    name: &'static str,
    people: u32,
}

impl GridRow for City {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

fn cities() -> Vec<City> {
    [
        ("Berlin", 3_700_000),
        ("Bern", 134_000),
        ("Hamburg", 1_900_000),
        ("Bonn", 330_000),
    ]
    .into_iter()
    .zip(1..)
    .map(|((name, people), id)| City { id, name, people })
    .collect()
}

fn columns() -> Vec<Column<City>> {
    vec![
        Column::new("name", "Name").value_text(|city: &City| city.name),
        Column::new("people", "People").value_of(|city: &City| city.people),
        // Neither a value nor filter text: nothing to filter.
        Column::new("badge", "Badge").cell(|_: &City| rsx! { "★" }),
    ]
}

#[component]
fn Menus(german: bool) -> Element {
    let rows = use_signal(cities);
    let cols = use_hook(columns);
    let locale = if german {
        GridLocale::german()
    } else {
        GridLocale::english()
    };
    let mut grid = use_grid(rows, cols, GridOptions::default().locale(locale));
    use_hook(move || {
        grid.set_column_filter("people", ColumnFilter::new(Condition::greater(1_000_000)));
    });

    rsx! {
        for index in 0..3 {
            GridFilterMenu { key: "{index}", grid, column_index: index }
        }
        GridRoot { grid,
            GridHeader { grid }
            GridBody { grid }
        }
    }
}

fn render(german: bool) -> String {
    let mut dom = VirtualDom::new_with_props(Menus, MenusProps { german });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn a_closed_menu_is_a_labelled_button_that_announces_a_dialog() {
    let html = render(false);

    assert!(
        html.contains(r#"aria-label="Filter options for Name""#),
        "{html}"
    );
    assert!(html.contains(r#"aria-haspopup="dialog""#), "{html}");
    assert!(html.contains(r#"aria-expanded="false""#), "{html}");
    assert!(!html.contains(r#"role="dialog""#), "{html}");
    // Nothing to filter the badge column by, so it gets no menu.
    assert!(!html.contains("Filter options for Badge"), "{html}");
}

#[test]
fn the_menu_button_follows_the_locale() {
    assert!(render(true).contains(r#"aria-label="Filteroptionen für Name""#));
}

#[test]
fn a_typed_filter_narrows_the_rows_and_marks_the_column() {
    let html = render(false);

    assert!(
        html.contains(r#"aria-rowcount="3""#),
        "two cities over a million: {html}"
    );
    assert_eq!(html.matches(r#"data-filtered="true""#).count(), 2, "{html}");
}

#[component]
fn LocalList() -> Element {
    let rows = use_signal(cities);
    let cols = use_hook(columns);
    let mut grid = use_grid(rows, cols, GridOptions::default());
    let mut list = use_signal(|| None::<DistinctValues>);
    use_hook(move || {
        grid.set_filter("people", ">200000");
        let load = grid.distinct_values(ColumnId::from("name"), 10);
        spawn(async move { list.set(load.await.ok()) });
    });

    let names: Vec<String> = list
        .read()
        .iter()
        .flat_map(|list| list.values.iter())
        .map(|(value, count)| match value {
            FilterValue::Text(name) => format!("{name}={count}"),
            other => format!("{other:?}"),
        })
        .collect();
    let names = names.join(",");
    rsx! { output { "{names}" } }
}

#[derive(Clone, Copy)]
struct Server;

impl DataSource<City> for Server {
    type Error = String;

    async fn fetch(&self, query: GridQuery) -> Result<Page<City>, String> {
        let rows: Vec<City> = cities().into_iter().take(query.page_size).collect();
        Ok(Page::new(rows, 4))
    }

    async fn distinct_values(
        &self,
        column: ColumnId,
        query: GridQuery,
        limit: usize,
    ) -> Result<DistinctValues, String> {
        // Echo what arrived, so the test can see it.
        let text = format!("{column}|{}|{limit}", query.column_filters.len());
        Ok(DistinctValues {
            values: vec![(FilterValue::Text(text), 1)],
            ..DistinctValues::default()
        })
    }
}

#[component]
fn RemoteList() -> Element {
    let cols = use_hook(columns);
    let mut grid = use_grid_remote(Server, cols, GridOptions::paged(2));
    let mut list = use_signal(|| None::<DistinctValues>);
    use_hook(move || {
        grid.set_filter("people", ">200000");
        let load = grid.distinct_values(ColumnId::from("name"), 7);
        spawn(async move { list.set(load.await.ok()) });
    });

    let echoed = list
        .read()
        .as_ref()
        .and_then(|list| list.values.first().map(|(value, _)| format!("{value:?}")))
        .unwrap_or_default();
    rsx! { output { "{echoed}" } }
}

async fn settle(dom: &mut VirtualDom) -> String {
    let deadline = Instant::now() + Duration::from_millis(50);
    loop {
        tokio::select! {
            () = dom.wait_for_work() => dom.render_immediate(&mut NoOpMutations),
            () = sleep_until(deadline) => break,
        }
    }
    dioxus_ssr::render(dom)
}

#[tokio::test(start_paused = true)]
async fn a_local_value_list_counts_what_the_other_filters_let_through() {
    let mut dom = VirtualDom::new(LocalList);
    dom.rebuild_in_place();
    let html = settle(&mut dom).await;

    // Bern has too few people; the rest are listed in order.
    assert!(
        html.contains("<output>Berlin=1,Bonn=1,Hamburg=1</output>"),
        "{html}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_remote_value_list_asks_the_data_source_with_the_filters() {
    let mut dom = VirtualDom::new(RemoteList);
    dom.rebuild_in_place();
    let html = settle(&mut dom).await;

    assert!(html.contains("name|1|7"), "{html}");
}
