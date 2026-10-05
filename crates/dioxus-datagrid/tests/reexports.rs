//! That an application needs this crate and nothing else: the types it has to
//! name to build a state, implement a data source or read a selection all have
//! a name here. Compiling is most of the test — it fails by not building.
//!
//! Nothing here reaches through `datagrid_core`, on purpose.

#![allow(clippy::unwrap_used)]

use dioxus_datagrid::{
    CellFocus, CellRange, ColumnFilter, ColumnId, ColumnSpec, ColumnWidth, Condition, DataSource,
    FilterOp, GridLocale, GridQuery, GridRow, GridState, Page, PageState, Pinned, Selection,
    SelectionExtent, SelectionMode, SortDirection, SortState, Value, from_tsv, move_row, to_tsv,
};

#[derive(Clone, PartialEq)]
struct Row {
    id: u32,
}

impl GridRow for Row {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

#[test]
fn a_state_can_be_built_by_hand() {
    // What `initial_state` is given, and what `on_state_change` hands back.
    let state = GridState {
        sort: vec![SortState::desc("name"), SortState::asc("city")],
        page: Some(PageState::new(25)),
        column_filters: vec![(ColumnId::new("city"), "Ber".to_owned())],
        filters: vec![(
            ColumnId::new("age"),
            ColumnFilter::new(Condition::new(FilterOp::Greater, vec![Value::from(30.0)])),
        )],
        pinned_columns: vec![(ColumnId::new("name"), Pinned::Start)],
        ..GridState::new()
    };

    assert_eq!(
        state.sort_direction(&ColumnId::new("name")),
        Some(SortDirection::Desc)
    );
    assert!(state.is_filtered(&ColumnId::new("age")));
}

#[test]
fn a_data_source_can_be_written() {
    struct Remote;

    impl DataSource<Row> for Remote {
        type Error = String;

        async fn fetch(&self, query: GridQuery) -> Result<Page<Row>, String> {
            // The reason this test exists: reading the sort order means naming
            // `SortState`, which had no name here before 0.10.
            let order: Vec<(ColumnId, SortDirection)> = query
                .sort
                .iter()
                .map(|sort: &SortState| (sort.column.clone(), sort.direction))
                .collect();
            Ok(Page::new(vec![Row { id: 1 }], order.len()))
        }
    }

    let query = GridQuery {
        sort: vec![SortState::asc("name")],
        page_size: 25,
        ..GridQuery::default()
    };

    // Not awaited: a runtime is beside the point, and the types are the test.
    let _pending = Remote.fetch(query);
}

#[test]
fn a_selection_can_be_read() {
    let mut selection = Selection::new();
    selection.toggle(1_u32, SelectionMode::Multi);

    assert_eq!(selection.extent(&[1, 2]), SelectionExtent::Partial);
    assert_eq!(
        CellRange::single(CellFocus::new(1, 0))
            .extended_to(CellFocus::new(3, 2))
            .cell_count(),
        9
    );
}

#[test]
fn the_odd_jobs_are_reachable_too() {
    let mut rows = vec![1, 2, 3];
    assert!(move_row(&mut rows, 0, 2));
    assert_eq!(rows, [2, 3, 1]);

    let text = to_tsv(&[vec!["a".to_owned(), "b".to_owned()]]);
    assert_eq!(from_tsv(&text), [["a", "b"]]);

    let spec: ColumnSpec<Row> = ColumnSpec::new("id");
    assert_eq!(spec.width, ColumnWidth::Auto);
    assert_eq!(GridLocale::default().no_rows, GridLocale::english().no_rows);
}

#[test]
fn the_core_crate_is_reachable_whole() {
    // The escape hatch for anything the list above has not caught up with.
    let column = dioxus_datagrid::datagrid_core::ColumnId::new("name");

    assert_eq!(column.as_str(), "name");
}
