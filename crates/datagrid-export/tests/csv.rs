//! CSV from a real grid's table: what a spreadsheet will meet.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use datagrid_core::{
    Aggregate, CellFormat, ColumnId, ColumnSpec, ExportCell, ExportColumn, ExportRow, ExportScope,
    ExportTable, GridLocale, GridState, Value, export_table,
};
use datagrid_export::{CsvOptions, CsvValues, LineEnding, to_csv};

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
            age: -27,
        },
        Row {
            name: "Max",
            city: "Berlin",
            salary: Some(3100.0),
            age: 45,
        },
    ]
}

fn columns() -> Vec<ColumnSpec<Row>> {
    vec![
        ColumnSpec::new("name").value_text(|row: &Row| row.name),
        ColumnSpec::new("city").value_text(|row: &Row| row.city),
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
    id.as_str().to_owned()
}

fn csv_of(rows: &[Row], state: &GridState, options: &CsvOptions) -> String {
    let table = export_table(
        rows,
        &columns(),
        label,
        state,
        &GridLocale::english(),
        ExportScope::Filtered,
    );
    to_csv(&table, options)
}

fn text_table(cells: &[&str]) -> ExportTable {
    ExportTable {
        columns: cells
            .iter()
            .map(|c| ExportColumn::new((*c).to_owned(), *c))
            .collect(),
        rows: vec![ExportRow::Data(
            cells
                .iter()
                .map(|c| ExportCell {
                    value: Some(Value::Text((*c).into())),
                    text: (*c).into(),
                })
                .collect(),
        )],
    }
}

#[test]
fn writes_a_header_and_a_line_per_row_with_values_not_text() {
    let csv = csv_of(&rows(), &GridState::new(), &CsvOptions::default());
    assert_eq!(
        csv,
        "name,city,salary,age\r\n\
         Zoe,Berlin,4200.5,31\r\n\
         Adi,Hamburg,,-27\r\n\
         Max,Berlin,3100,45\r\n\
         Totals,,\"Sum €7,300.50\",\r\n"
    );
}

#[test]
fn can_write_what_the_grid_shows_instead() {
    let options = CsvOptions {
        values: CsvValues::Shown,
        ..CsvOptions::default()
    };
    let csv = csv_of(&rows(), &GridState::new(), &options);
    // The thousands separator is a comma here, so the field is quoted.
    assert!(csv.contains("Zoe,Berlin,\"€4,200.50\",31\r\n"), "{csv}");
}

#[test]
fn a_semicolon_delimiter_leaves_a_decimal_comma_alone() {
    let options = CsvOptions {
        delimiter: ';',
        ..CsvOptions::default()
    };
    let table = export_table(
        &rows(),
        &columns(),
        label,
        &GridState::new(),
        &GridLocale::german(),
        ExportScope::Filtered,
    );
    let csv = to_csv(
        &table,
        &CsvOptions {
            values: CsvValues::Shown,
            ..options
        },
    );
    assert!(csv.contains("Zoe;Berlin;4.200,50\u{a0}€;31\r\n"), "{csv}");
}

#[test]
fn quotes_what_would_break_a_line() {
    let table = text_table(&["a,b", "say \"hi\"", "two\nlines", "plain"]);
    let csv = to_csv(&table, &CsvOptions::default());
    assert_eq!(
        csv,
        "\"a,b\",\"say \"\"hi\"\"\",\"two\nlines\",plain\r\n\
         \"a,b\",\"say \"\"hi\"\"\",\"two\nlines\",plain\r\n"
    );
}

#[test]
fn line_ending_and_header_are_choices() {
    let options = CsvOptions {
        line_ending: LineEnding::Lf,
        header: false,
        ..CsvOptions::default()
    };
    let csv = csv_of(&rows()[..1], &GridState::new(), &options);
    assert_eq!(csv, "Zoe,Berlin,4200.5,31\nTotals,,\"Sum €4,200.50\",\n");
}

#[test]
fn a_byte_order_mark_is_asked_for() {
    let options = CsvOptions {
        bom: true,
        ..CsvOptions::default()
    };
    assert!(csv_of(&rows(), &GridState::new(), &options).starts_with('\u{feff}'));
    assert!(!csv_of(&rows(), &GridState::new(), &CsvOptions::default()).starts_with('\u{feff}'));
}

mod formulas {
    use super::*;

    #[test]
    fn text_a_spreadsheet_would_run_is_defused() {
        let table = text_table(&["=SUM(A1:A9)", "+1+1", "-2+3", "@cmd", "\tx", "fine"]);
        let csv = to_csv(&table, &CsvOptions::default());
        let line = csv.lines().nth(1).unwrap();
        assert_eq!(line, "'=SUM(A1:A9),'+1+1,'-2+3,'@cmd,'	x,fine");
    }

    #[test]
    fn the_header_is_guarded_too() {
        let table = text_table(&["=evil"]);
        assert!(to_csv(&table, &CsvOptions::default()).starts_with("'=evil\r\n"));
    }

    #[test]
    fn numbers_stay_numbers() {
        let csv = csv_of(&rows(), &GridState::new(), &CsvOptions::default());
        assert!(
            csv.contains(",-27\r\n"),
            "a negative number is not a formula: {csv}"
        );
    }

    #[test]
    fn can_be_switched_off() {
        let table = text_table(&["=A1"]);
        let options = CsvOptions {
            guard_formulas: false,
            ..CsvOptions::default()
        };
        assert!(to_csv(&table, &options).starts_with("=A1\r\n"));
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
    fn a_heading_line_before_the_rows_and_aggregates_after() {
        let csv = csv_of(&rows(), &grouped(), &CsvOptions::default());
        assert_eq!(
            csv,
            "name,city,salary,age\r\n\
             city: Berlin,,,\r\n\
             Zoe,Berlin,4200.5,31\r\n\
             Max,Berlin,3100,45\r\n\
             ,,\"Sum €7,300.50\",\r\n\
             city: Hamburg,,,\r\n\
             Adi,Hamburg,,-27\r\n\
             ,,,\r\n\
             Totals,,\"Sum €7,300.50\",\r\n"
        );
    }
}

#[test]
fn every_line_has_a_field_per_column() {
    let csv = csv_of(&rows(), &GridState::new(), &CsvOptions::default());
    for line in csv.lines() {
        // Commas inside quotes belong to a field; only the others separate.
        let outside: String = line.split('"').step_by(2).collect();
        assert_eq!(outside.matches(',').count(), 3, "{line}");
    }
}

#[test]
fn no_columns_write_nothing() {
    let table = ExportTable::default();
    assert_eq!(to_csv(&table, &CsvOptions::default()), "");
}
