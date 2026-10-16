//! The HTML table: what a page will print, and that no cell can write markup.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use datagrid_core::{
    Aggregate, CellAlign, CellFormat, ColumnId, ColumnSpec, ExportCell, ExportColumn, ExportRow,
    ExportScope, ExportTable, GridLocale, GridState, export_table,
};
use datagrid_export::{HtmlOptions, to_html};

#[derive(Clone, Debug)]
struct Row {
    name: &'static str,
    city: &'static str,
    salary: Option<f64>,
}

fn rows() -> Vec<Row> {
    vec![
        Row {
            name: "Zoe",
            city: "Berlin",
            salary: Some(4200.5),
        },
        Row {
            name: "Adi",
            city: "Hamburg",
            salary: None,
        },
        Row {
            name: "Max",
            city: "Berlin",
            salary: Some(3100.0),
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
            .align(CellAlign::End)
            .aggregate(Aggregate::Sum),
    ]
}

fn table(state: &GridState) -> ExportTable {
    export_table(
        &rows(),
        &columns(),
        |id: &ColumnId| id.as_str().to_uppercase(),
        state,
        &GridLocale::english(),
        ExportScope::Filtered,
    )
}

fn hostile(text: &str) -> ExportTable {
    ExportTable {
        columns: vec![ExportColumn::new("a", text)],
        rows: vec![ExportRow::Data(vec![ExportCell {
            value: None,
            text: text.into(),
        }])],
    }
}

#[test]
fn no_cell_can_write_markup() {
    for text in [
        "<script>alert(1)</script>",
        "\"><img src=x onerror=alert(1)>",
        "'; DROP TABLE",
        "&lt;already&gt;",
    ] {
        let html = to_html(&hostile(text), &HtmlOptions::default());
        assert!(!html.contains("<script"), "{text}: {html}");
        assert!(!html.contains("<img"), "{text}: {html}");
        // Every `<` begins a tag the writer itself put there.
        let ours = ["table", "thead", "tbody", "tr", "th", "td"];
        for piece in html.split('<').skip(1) {
            let name: String = piece
                .trim_start_matches('/')
                .chars()
                .take_while(char::is_ascii_alphabetic)
                .collect();
            assert!(ours.contains(&name.as_str()), "{text}: stray <{piece}");
        }
    }
}

#[test]
fn a_caption_and_a_language_are_escaped_too() {
    let options = HtmlOptions {
        caption: Some("<b>Q1</b>".into()),
        document: true,
        lang: Some("\"de".into()),
        style: false,
    };
    let html = to_html(&hostile("x"), &options);
    assert!(
        html.contains("<caption>&lt;b&gt;Q1&lt;/b&gt;</caption>"),
        "{html}"
    );
    assert!(html.contains("lang=\"&quot;de\""), "{html}");
}

#[test]
fn a_header_over_a_row_per_data_row() {
    let html = to_html(&table(&GridState::new()), &HtmlOptions::default());
    assert_eq!(html.matches("<thead>").count(), 1);
    assert!(html.contains("<th scope=\"col\">NAME</th>"), "{html}");
    assert!(
        html.contains("<th scope=\"col\" class=\"end\">SALARY</th>"),
        "{html}"
    );
    assert!(html.contains("<td class=\"end\">€4,200.50</td>"), "{html}");
    // Three data rows, and the totals under them.
    assert_eq!(html.matches("<tr>").count(), 1 + 3);
    assert!(html.contains("<tr class=\"totals\">"), "{html}");
    assert!(html.contains("Sum €7,300.50"), "{html}");
}

#[test]
fn a_document_wraps_the_table_with_the_print_rules() {
    let options = HtmlOptions {
        document: true,
        lang: Some("en".into()),
        ..HtmlOptions::default()
    };
    let html = to_html(&table(&GridState::new()), &options);
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<html lang=\"en\">"));
    // The header repeats on every printed page.
    assert!(html.contains("thead { display: table-header-group; }"));
    assert!(html.trim_end().ends_with("</html>"));

    let fragment = to_html(&table(&GridState::new()), &HtmlOptions::default());
    assert!(fragment.starts_with("<table"));
}

#[test]
fn groups_are_rows_that_say_what_follows() {
    let mut state = GridState::new();
    state.group_by_column("city", None);
    let html = to_html(&table(&state), &HtmlOptions::default());
    assert!(
        html.contains("<th colspan=\"3\" scope=\"colgroup\" style=\"padding-inline-start:1em\">CITY: Berlin</th>"),
        "{html}"
    );
    assert_eq!(html.matches("class=\"group\"").count(), 2);
    assert!(html.contains("<tr class=\"aggregates\">"), "{html}");
}

#[test]
fn column_groups_become_header_rows_that_span_their_columns() {
    let mut columns: Vec<ExportColumn> = ["a", "b", "c", "d"]
        .iter()
        .map(|id| ExportColumn::new((*id).to_owned(), *id))
        .collect();
    columns[0].group_path = vec!["Contact".into()];
    columns[1].group_path = vec!["Contact".into()];
    columns[2].group_path = vec!["Work".into()];
    let table = ExportTable {
        columns,
        rows: Vec::new(),
    };

    let html = to_html(&table, &HtmlOptions::default());
    assert!(
        html.contains("<th colspan=\"2\" scope=\"colgroup\" class=\"center\">Contact</th>"),
        "{html}"
    );
    assert!(
        html.contains("<th colspan=\"1\" scope=\"colgroup\" class=\"center\">Work</th>"),
        "{html}"
    );
    // The column outside every group has an empty cell above it.
    assert!(
        html.contains("<th colspan=\"1\" scope=\"colgroup\" class=\"center\"></th>"),
        "{html}"
    );
    assert_eq!(html.matches("<thead>").count(), 1);
    assert_eq!(html.matches("<tr>").count(), 2);
}

#[test]
fn a_table_without_rows_is_still_a_table() {
    let html = to_html(&ExportTable::default(), &HtmlOptions::default());
    assert!(html.contains("<table"));
    assert!(html.contains("</table>"));
}

#[test]
fn a_fragment_brings_its_rules_only_when_asked() {
    let table = table(&GridState::new());
    let plain = to_html(&table, &HtmlOptions::default());
    let styled = to_html(
        &table,
        &HtmlOptions {
            style: true,
            ..HtmlOptions::default()
        },
    );
    assert!(!plain.contains("<style>"));
    assert!(styled.starts_with("<style>"));
    assert!(styled.contains("<table class=\"datagrid\">"));
}
