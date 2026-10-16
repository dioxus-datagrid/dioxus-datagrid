//! What an export takes from a grid: the visible columns in the order the grid
//! lays them out, the rows as sorted, filtered and grouped, typed and formatted.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use datagrid_core::{
    Aggregate, AggregateKind, CellFormat, ColumnId, ColumnSpec, ExportRow, ExportScope,
    ExportTable, GridLocale, GridState, Pinned, SortDirection, Value, export_table, layout_order,
};

#[derive(Clone, Debug)]
struct Row {
    name: &'static str,
    city: &'static str,
    salary: Option<f64>,
    age: i64,
}

fn rows() -> Vec<Row> {
    vec![
        Row {
            name: "Zoe",
            city: "Berlin",
            salary: Some(4200.5),
            age: 31,
        },
        Row {
            name: "Adi",
            city: "Hamburg",
            salary: None,
            age: 27,
        },
        Row {
            name: "Max",
            city: "Berlin",
            salary: Some(3100.0),
            age: 45,
        },
        Row {
            name: "Eva",
            city: "Hamburg",
            salary: Some(5000.0),
            age: 38,
        },
    ]
}

fn columns() -> Vec<ColumnSpec<Row>> {
    vec![
        ColumnSpec::new("name").value_text(|row: &Row| row.name),
        ColumnSpec::new("city")
            .value_text(|row: &Row| row.city)
            .filter_by(|row: &Row| row.city.to_owned()),
        ColumnSpec::new("salary")
            .value_of(|row: &Row| row.salary)
            .format(CellFormat::Currency {
                symbol: "€".into(),
                decimals: 2,
            })
            .aggregate(Aggregate::Sum),
        ColumnSpec::new("age").value_of(|row: &Row| row.age),
    ]
}

fn label(id: &ColumnId) -> String {
    match id.as_str() {
        "name" => "Name".into(),
        "city" => "City".into(),
        "salary" => "Salary".into(),
        other => other.to_owned(),
    }
}

fn table(columns: &[ColumnSpec<Row>], state: &GridState, scope: ExportScope) -> ExportTable {
    export_table(
        &rows(),
        columns,
        label,
        state,
        &GridLocale::english(),
        scope,
    )
}

fn data(table: &ExportTable) -> Vec<Vec<String>> {
    table
        .rows
        .iter()
        .filter_map(|row| match row {
            ExportRow::Data(cells) => Some(cells.iter().map(|cell| cell.text.clone()).collect()),
            _ => None,
        })
        .collect()
}

fn names(table: &ExportTable) -> Vec<String> {
    data(table).into_iter().map(|row| row[0].clone()).collect()
}

mod columns {
    use super::*;

    #[test]
    fn follow_the_layout_of_the_grid() {
        let mut state = GridState::new();
        state.hidden_columns = vec![ColumnId::from("city")];
        state.column_order = vec!["age".into(), "name".into()];
        let table = table(&columns(), &state, ExportScope::Filtered);

        let ids: Vec<&str> = table
            .columns
            .iter()
            .map(|column| column.id.as_str())
            .collect();
        assert_eq!(ids, ["age", "name", "salary"]);
        assert_eq!(table.columns[1].label, "Name");
    }

    #[test]
    fn gather_the_pinned_ones_at_their_edges() {
        let specs = vec![
            ColumnSpec::<Row>::new("a").pin(Pinned::End),
            ColumnSpec::new("b"),
            ColumnSpec::new("c").pin(Pinned::Start),
        ];
        assert_eq!(layout_order(&specs, &GridState::new()), [2, 1, 0]);
    }

    #[test]
    fn leave_out_the_chrome() {
        let mut specs = columns();
        specs.insert(0, ColumnSpec::new("pick").checkbox());
        specs.insert(1, ColumnSpec::new("open").expander());
        specs.insert(2, ColumnSpec::new("move").drag_handle());
        let table = table(&specs, &GridState::new(), ExportScope::Filtered);
        assert_eq!(table.columns.len(), 4);
        assert_eq!(table.columns[0].id.as_str(), "name");
    }
}

mod rows {
    use super::*;

    #[test]
    fn come_sorted() {
        let mut state = GridState::new();
        state.set_sort("age", SortDirection::Desc);
        let table = table(&columns(), &state, ExportScope::Filtered);
        assert_eq!(names(&table), ["Max", "Eva", "Zoe", "Adi"]);
    }

    #[test]
    fn come_filtered() {
        let mut state = GridState::new();
        state.set_search("hamburg");
        let table = table(&columns(), &state, ExportScope::Filtered);
        assert_eq!(names(&table), ["Adi", "Eva"]);
    }

    #[test]
    fn view_is_the_page_and_filtered_is_every_page() {
        let state = GridState::paged(3);
        let page = table(&columns(), &state, ExportScope::View);
        let all = table(&columns(), &state, ExportScope::Filtered);
        assert_eq!(page.data_rows(), 3);
        assert_eq!(all.data_rows(), 4);
    }

    #[test]
    fn view_follows_the_page_the_state_is_on() {
        let mut state = GridState::paged(3);
        state.page.as_mut().unwrap().index = 1;
        assert_eq!(
            names(&table(&columns(), &state, ExportScope::View)),
            ["Eva"]
        );
    }
}

mod cells {
    use super::*;

    #[test]
    fn carry_the_value_and_the_text_the_grid_shows() {
        let table = table(&columns(), &GridState::new(), ExportScope::Filtered);
        let ExportRow::Data(cells) = &table.rows[0] else {
            panic!("a data row first");
        };
        assert_eq!(cells[2].value, Some(Value::Float(4200.5)));
        assert_eq!(cells[2].text, "€4,200.50");
        assert_eq!(cells[3].value, Some(Value::Int(31)));
        assert_eq!(cells[0].value, Some(Value::Text("Zoe".into())));
    }

    #[test]
    fn are_empty_where_the_row_has_no_value() {
        let table = table(&columns(), &GridState::new(), ExportScope::Filtered);
        let ExportRow::Data(cells) = &table.rows[1] else {
            panic!("a data row");
        };
        assert_eq!(cells[2].value, None);
        assert_eq!(cells[2].text, "");
    }

    #[test]
    fn are_empty_for_a_column_that_reads_nothing() {
        let specs = vec![ColumnSpec::<Row>::new("shown")];
        let table = table(&specs, &GridState::new(), ExportScope::Filtered);
        assert_eq!(data(&table)[0], [""]);
    }

    #[test]
    fn follow_the_locale() {
        let german = export_table(
            &rows(),
            &columns(),
            label,
            &GridState::new(),
            &GridLocale::german(),
            ExportScope::Filtered,
        );
        assert_eq!(data(&german)[0][2], "4.200,50\u{a0}€");
    }
}

mod groups {
    use super::*;

    fn grouped() -> GridState {
        let mut state = GridState::new();
        state.group_by_column("city", None);
        state
    }

    #[test]
    fn put_a_heading_before_their_rows_and_the_aggregates_after() {
        let table = table(&columns(), &grouped(), ExportScope::Filtered);
        let shape: Vec<&str> = table
            .rows
            .iter()
            .map(|row| match row {
                ExportRow::Data(_) => "data",
                ExportRow::GroupHeader { .. } => "header",
                ExportRow::Aggregates { level: Some(_), .. } => "footer",
                ExportRow::Aggregates { level: None, .. } => "totals",
                _ => "other",
            })
            .collect();
        assert_eq!(
            shape,
            [
                "header", "data", "data", "footer", "header", "data", "data", "footer", "totals"
            ]
        );

        let ExportRow::GroupHeader {
            caption,
            count,
            level,
            expanded,
            ..
        } = &table.rows[0]
        else {
            panic!("a heading first");
        };
        assert_eq!(
            (caption.as_str(), *count, *level, *expanded),
            ("City: Berlin", 2, 0, true)
        );
    }

    #[test]
    fn carry_the_aggregates_of_their_rows() {
        let table = table(&columns(), &grouped(), ExportScope::Filtered);
        let ExportRow::Aggregates { cells, .. } = &table.rows[3] else {
            panic!("a footer");
        };
        // Berlin: 4200.5 + 3100.
        let salary = &cells[2][0];
        assert_eq!(salary.kind, AggregateKind::Sum);
        assert_eq!(salary.name, "Sum");
        assert_eq!(salary.value, Some(Value::Float(7300.5)));
        assert_eq!(salary.text, "€7,300.50");
        assert!(cells[0].is_empty());
    }

    #[test]
    fn end_with_the_totals_over_every_row() {
        let table = table(&columns(), &grouped(), ExportScope::Filtered);
        let Some(ExportRow::Aggregates {
            level: None,
            label,
            cells,
        }) = table.rows.last()
        else {
            panic!("totals last");
        };
        assert_eq!(label.as_deref(), Some("Totals"));
        assert_eq!(cells[2][0].value, Some(Value::Float(12300.5)));
    }

    #[test]
    fn a_closed_group_is_open_in_a_filtered_export_and_closed_in_the_view() {
        let mut state = grouped();
        state.groups_collapsed = true;

        let all = table(&columns(), &state, ExportScope::Filtered);
        assert_eq!(all.data_rows(), 4);

        let shown = table(&columns(), &state, ExportScope::View);
        assert_eq!(shown.data_rows(), 0);
        let ExportRow::GroupHeader {
            expanded,
            aggregates,
            ..
        } = &shown.rows[0]
        else {
            panic!("a heading");
        };
        // With no footer to carry them, a closed group says its aggregates itself.
        assert!(!expanded);
        assert_eq!(aggregates[2][0].text, "€7,300.50");
    }
}

#[test]
fn no_rows_make_a_table_with_columns_and_nothing_else() {
    let table = export_table(
        &Vec::<Row>::new(),
        &columns(),
        label,
        &GridState::new(),
        &GridLocale::english(),
        ExportScope::View,
    );
    assert_eq!(table.columns.len(), 4);
    assert!(table.rows.is_empty());
}
