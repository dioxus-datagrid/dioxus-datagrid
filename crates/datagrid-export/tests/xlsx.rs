#![cfg(feature = "xlsx")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

//! What `to_xlsx` writes, read back out of the file it makes: an `.xlsx` is a zip of
//! XML, so the tests unzip it and look at the cells, the styles and the sheet.

use datagrid_core::{
    AggregateKind, CellAlign, CellFormat, ExportAggregate, ExportCell, ExportColumn, ExportRow,
    ExportTable, Value,
};
use datagrid_export::{XlsxOptions, to_xlsx};
use std::io::Read;

/// One file of the archive, as text.
fn entry(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut file = archive.by_name(name).unwrap();
    let mut text = String::new();
    file.read_to_string(&mut text).unwrap();
    text
}

fn sheet(bytes: &[u8]) -> String {
    entry(bytes, "xl/worksheets/sheet1.xml")
}

fn styles(bytes: &[u8]) -> String {
    entry(bytes, "xl/styles.xml")
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// The shared strings, in order.
fn strings(bytes: &[u8]) -> Vec<String> {
    let xml = entry(bytes, "xl/sharedStrings.xml");
    xml.split("<si>")
        .skip(1)
        .map(|item| {
            let start = item.find("<t").unwrap();
            let start = item[start..].find('>').unwrap() + start + 1;
            let end = item.find("</t>").unwrap();
            unescape(&item[start..end])
        })
        .collect()
}

/// What a cell holds: its type attribute and its raw value, or `None` for a cell
/// the sheet does not have.
fn cell(bytes: &[u8], reference: &str) -> Option<(String, String)> {
    let xml = sheet(bytes);
    let at = xml.find(&format!("<c r=\"{reference}\""))?;
    let rest = &xml[at..];
    let tag_end = rest.find('>').unwrap();
    let tag = &rest[..tag_end];
    let kind = tag
        .split(" t=\"")
        .nth(1)
        .and_then(|t| t.split('"').next())
        .unwrap_or("n")
        .to_owned();
    if tag.ends_with('/') {
        return Some((kind, String::new()));
    }
    let end = rest.find("</c>").unwrap();
    let body = &rest[tag_end + 1..end];
    let value = body
        .split("<v>")
        .nth(1)
        .and_then(|v| v.split("</v>").next())
        .unwrap_or("")
        .to_owned();
    Some((kind, value))
}

/// The text of a cell, looking a shared string up.
fn text(bytes: &[u8], reference: &str) -> String {
    let (kind, value) = cell(bytes, reference).unwrap();
    assert_eq!(kind, "s", "{reference} is not text");
    strings(bytes)[value.parse::<usize>().unwrap()].clone()
}

fn data(cells: Vec<ExportCell>) -> ExportRow {
    ExportRow::Data(cells)
}

fn int(value: i64) -> ExportCell {
    ExportCell {
        value: Some(Value::Int(value)),
        text: value.to_string(),
    }
}

fn float(value: f64, shown: &str) -> ExportCell {
    ExportCell {
        value: Some(Value::Float(value)),
        text: shown.to_owned(),
    }
}

fn words(value: &str) -> ExportCell {
    ExportCell {
        value: Some(Value::Text(value.to_owned())),
        text: value.to_owned(),
    }
}

fn column(id: &str, format: CellFormat) -> ExportColumn {
    ExportColumn {
        format,
        ..ExportColumn::new(id.to_owned(), id.to_uppercase())
    }
}

fn aggregate(kind: AggregateKind, name: &str, value: Value, shown: &str) -> ExportAggregate {
    ExportAggregate {
        kind,
        name: name.to_owned(),
        value: Some(value),
        text: shown.to_owned(),
    }
}

fn write(table: &ExportTable) -> Vec<u8> {
    to_xlsx(table, &XlsxOptions::default()).unwrap()
}

#[test]
fn a_workbook_is_a_zip_with_one_sheet() {
    let bytes = write(&ExportTable {
        columns: vec![column("a", CellFormat::Plain)],
        rows: vec![data(vec![int(1)])],
    });
    assert_eq!(&bytes[..2], b"PK");
    assert!(entry(&bytes, "xl/workbook.xml").contains("<sheet name=\"Export\""));
}

#[test]
fn cells_are_typed() {
    let bytes = write(&ExportTable {
        columns: vec![
            column("name", CellFormat::Plain),
            column("age", CellFormat::Plain),
            column("score", CellFormat::Plain),
            column("active", CellFormat::Plain),
        ],
        rows: vec![data(vec![
            words("Zoe"),
            int(31),
            float(4200.5, "4,200.5"),
            ExportCell {
                value: Some(Value::Bool(true)),
                text: "Yes".into(),
            },
        ])],
    });
    // The header is on row 1, the data on row 2.
    assert_eq!(text(&bytes, "A1"), "NAME");
    assert_eq!(text(&bytes, "A2"), "Zoe");
    assert_eq!(cell(&bytes, "B2"), Some(("n".into(), "31".into())));
    assert_eq!(cell(&bytes, "C2"), Some(("n".into(), "4200.5".into())));
    assert_eq!(cell(&bytes, "D2"), Some(("b".into(), "1".into())));
}

#[test]
fn an_empty_cell_is_left_out() {
    let bytes = write(&ExportTable {
        columns: vec![
            column("a", CellFormat::Plain),
            column("b", CellFormat::Plain),
        ],
        rows: vec![data(vec![ExportCell::default(), words("x")])],
    });
    assert_eq!(cell(&bytes, "A2"), None);
    assert_eq!(text(&bytes, "B2"), "x");
}

#[test]
fn text_that_looks_like_a_formula_is_text() {
    let bytes = write(&ExportTable {
        columns: vec![column("note", CellFormat::Plain)],
        rows: vec![
            data(vec![words("=SUM(1,2)")]),
            data(vec![words("@cmd|' /C calc'!A0")]),
            data(vec![words("-5+3")]),
        ],
    });
    assert_eq!(text(&bytes, "A2"), "=SUM(1,2)");
    assert_eq!(text(&bytes, "A3"), "@cmd|' /C calc'!A0");
    assert_eq!(text(&bytes, "A4"), "-5+3");
    // No cell is a formula.
    assert!(!sheet(&bytes).contains("<f>"));
}

#[test]
fn markup_in_a_cell_is_escaped() {
    let bytes = write(&ExportTable {
        columns: vec![column("note", CellFormat::Plain)],
        rows: vec![data(vec![words("<script>alert(1)</script> & \"x\"")])],
    });
    assert_eq!(text(&bytes, "A2"), "<script>alert(1)</script> & \"x\"");
    assert!(!entry(&bytes, "xl/sharedStrings.xml").contains("<script>"));
}

#[test]
fn a_number_beyond_what_excel_holds_is_the_text_shown() {
    let big = (1_i64 << 53) + 1;
    let bytes = write(&ExportTable {
        columns: vec![
            column("id", CellFormat::Plain),
            column("f", CellFormat::Plain),
        ],
        rows: vec![data(vec![
            int(big),
            ExportCell {
                value: Some(Value::Float(f64::NAN)),
                text: "NaN".into(),
            },
        ])],
    });
    assert_eq!(text(&bytes, "A2"), big.to_string());
    assert_eq!(text(&bytes, "B2"), "NaN");
}

#[test]
fn a_text_longer_than_a_cell_holds_is_cut() {
    let long = "x".repeat(40_000);
    let bytes = write(&ExportTable {
        columns: vec![column("note", CellFormat::Plain)],
        rows: vec![data(vec![words(&long)])],
    });
    assert_eq!(text(&bytes, "A2").chars().count(), 32_767);
}

#[test]
fn number_formats_follow_the_column() {
    let table = ExportTable {
        columns: vec![
            column("n", CellFormat::number(2)),
            column("whole", CellFormat::number(0)),
            column("p", CellFormat::Percent { decimals: 1 }),
            column("price", CellFormat::currency("€", 2)),
        ],
        rows: vec![data(vec![
            float(1234.5, "1,234.50"),
            int(1234),
            float(0.256, "25.6%"),
            // The grid wrote the symbol first.
            float(9.9, "€9.90"),
        ])],
    };
    let styles = styles(&write(&table));
    assert!(styles.contains("formatCode=\"#,##0.00\""), "{styles}");
    assert!(styles.contains("formatCode=\"#,##0\""), "{styles}");
    assert!(styles.contains("formatCode=\"0.0%\""), "{styles}");
    assert!(
        styles.contains("formatCode=\"&quot;€&quot;#,##0.00\""),
        "{styles}"
    );
}

#[test]
fn a_currency_symbol_goes_where_the_grid_put_it() {
    let table = ExportTable {
        columns: vec![column("price", CellFormat::currency("€", 2))],
        rows: vec![data(vec![float(9.9, "9,90 €")])],
    };
    let styles = styles(&write(&table));
    assert!(
        styles.contains("formatCode=\"#,##0.00 &quot;€&quot;\""),
        "{styles}"
    );
}

#[test]
fn a_column_is_aligned_as_the_grid_aligns_it() {
    let mut right = column("n", CellFormat::Plain);
    right.align = CellAlign::End;
    let table = ExportTable {
        columns: vec![right],
        rows: vec![data(vec![int(1)])],
    };
    assert!(styles(&write(&table)).contains("horizontal=\"right\""));
}

#[test]
fn the_header_is_bold_and_stays_in_view() {
    let table = ExportTable {
        columns: vec![column("a", CellFormat::Plain)],
        rows: vec![data(vec![int(1)])],
    };
    let bytes = write(&table);
    assert!(styles(&bytes).contains("<b/>"));
    assert!(sheet(&bytes).contains("state=\"frozen\""));
    assert!(sheet(&bytes).contains("ySplit=\"1\""));

    let plain = to_xlsx(
        &table,
        &XlsxOptions {
            header: false,
            ..XlsxOptions::default()
        },
    )
    .unwrap();
    // No labels: the first row is data, and nothing is frozen.
    assert_eq!(cell(&plain, "A1"), Some(("n".into(), "1".into())));
    assert!(!sheet(&plain).contains("state=\"frozen\""));
}

#[test]
fn a_plain_table_gets_a_filter_and_one_with_groups_does_not() {
    let plain = ExportTable {
        columns: vec![column("a", CellFormat::Plain)],
        rows: vec![data(vec![int(1)]), data(vec![int(2)])],
    };
    assert!(sheet(&write(&plain)).contains("<autoFilter ref=\"A1:A3\""));

    let grouped = ExportTable {
        columns: vec![column("a", CellFormat::Plain)],
        rows: vec![
            ExportRow::GroupHeader {
                level: 0,
                caption: "G".into(),
                count: 1,
                expanded: true,
                aggregates: vec![],
            },
            data(vec![int(1)]),
        ],
    };
    assert!(!sheet(&write(&grouped)).contains("autoFilter"));

    let off = to_xlsx(
        &plain,
        &XlsxOptions {
            autofilter: false,
            ..XlsxOptions::default()
        },
    )
    .unwrap();
    assert!(!sheet(&off).contains("autoFilter"));
}

#[test]
fn columns_are_as_wide_as_what_they_hold() {
    let table = ExportTable {
        columns: vec![
            column("a", CellFormat::Plain),
            column("b", CellFormat::Plain),
        ],
        rows: vec![data(vec![words("x"), words(&"y".repeat(30))])],
    };
    let xml = sheet(&write(&table));
    // `a` has a one-character cell under a one-character label: the floor.
    assert!(xml.contains("min=\"1\" max=\"1\" width=\"8"), "{xml}");
    // `b` has thirty characters and a little room.
    assert!(xml.contains("min=\"2\" max=\"2\" width=\"32"), "{xml}");
}

#[test]
fn groups_are_headings_with_their_rows_in_an_outline() {
    let table = ExportTable {
        columns: vec![
            column("city", CellFormat::Plain),
            column("n", CellFormat::Plain),
        ],
        rows: vec![
            ExportRow::GroupHeader {
                level: 0,
                caption: "City: Berlin".into(),
                count: 2,
                expanded: true,
                aggregates: vec![],
            },
            data(vec![words("Berlin"), int(1)]),
            data(vec![words("Berlin"), int(2)]),
            ExportRow::Aggregates {
                level: Some(0),
                label: None,
                cells: vec![
                    vec![],
                    vec![aggregate(AggregateKind::Sum, "Sum", Value::Int(3), "3")],
                ],
            },
            ExportRow::GroupHeader {
                level: 0,
                caption: "City: Bonn".into(),
                count: 1,
                expanded: true,
                aggregates: vec![],
            },
            data(vec![words("Bonn"), int(5)]),
        ],
    };
    let bytes = write(&table);
    let xml = sheet(&bytes);
    assert_eq!(text(&bytes, "A2"), "City: Berlin");
    assert_eq!(text(&bytes, "A6"), "City: Bonn");
    // The rows of Berlin, with its aggregates, are one outline; Bonn's one row is
    // another.
    assert!(
        xml.contains("<row r=\"3\" spans=\"1:2\" outlineLevel=\"1\""),
        "{xml}"
    );
    assert!(
        xml.contains("<row r=\"5\" spans=\"1:2\" outlineLevel=\"1\""),
        "{xml}"
    );
    assert!(
        xml.contains("<row r=\"7\" spans=\"1:2\" outlineLevel=\"1\""),
        "{xml}"
    );
    assert!(
        !xml.contains("<row r=\"2\" spans=\"1:2\" outlineLevel"),
        "{xml}"
    );
    assert!(
        !xml.contains("<row r=\"6\" spans=\"1:2\" outlineLevel"),
        "{xml}"
    );
    // A heading is indented by its level, and the sum is a number.
    assert_eq!(cell(&bytes, "B5"), Some(("n".into(), "3".into())));
    // The button sits on the heading, above the rows.
    assert!(xml.contains("summaryBelow=\"0\""), "{xml}");
}

#[test]
fn nested_groups_nest_their_outlines() {
    let header = |level: usize, caption: &str| ExportRow::GroupHeader {
        level,
        caption: caption.into(),
        count: 1,
        expanded: true,
        aggregates: vec![],
    };
    let table = ExportTable {
        columns: vec![column("a", CellFormat::Plain)],
        rows: vec![header(0, "Outer"), header(1, "Inner"), data(vec![int(1)])],
    };
    let xml = sheet(&write(&table));
    // Row 3 is inside both; row 4 is the data row inside both.
    assert!(
        xml.contains("<row r=\"3\" spans=\"1:1\" outlineLevel=\"1\""),
        "{xml}"
    );
    assert!(
        xml.contains("<row r=\"4\" spans=\"1:1\" outlineLevel=\"2\""),
        "{xml}"
    );
}

#[test]
fn a_closed_group_carries_its_aggregates_on_its_heading() {
    let table = ExportTable {
        columns: vec![
            column("city", CellFormat::Plain),
            column("n", CellFormat::number(1)),
        ],
        rows: vec![ExportRow::GroupHeader {
            level: 0,
            caption: "City: Berlin".into(),
            count: 2,
            expanded: false,
            aggregates: vec![
                vec![],
                vec![aggregate(
                    AggregateKind::Sum,
                    "Sum",
                    Value::Float(7.5),
                    "7.5",
                )],
            ],
        }],
    };
    let bytes = write(&table);
    assert_eq!(text(&bytes, "A2"), "City: Berlin");
    assert_eq!(cell(&bytes, "B2"), Some(("n".into(), "7.5".into())));
    // A closed group has no rows, so no outline.
    assert!(!sheet(&bytes).contains("outlineLevel"));
}

#[test]
fn the_totals_row_names_itself_and_a_count_is_whole() {
    let table = ExportTable {
        columns: vec![
            column("name", CellFormat::Plain),
            column("amount", CellFormat::currency("€", 2)),
            column("n", CellFormat::number(2)),
        ],
        rows: vec![
            data(vec![words("a"), float(1.5, "€1.50"), int(1)]),
            ExportRow::Aggregates {
                level: None,
                label: Some("Totals".into()),
                cells: vec![
                    vec![],
                    vec![aggregate(
                        AggregateKind::Sum,
                        "Sum",
                        Value::Float(1.5),
                        "€1.50",
                    )],
                    vec![aggregate(AggregateKind::Count, "Count", Value::Int(1), "1")],
                ],
            },
        ],
    };
    let bytes = write(&table);
    assert_eq!(text(&bytes, "A3"), "Totals");
    assert_eq!(cell(&bytes, "B3"), Some(("n".into(), "1.5".into())));
    assert_eq!(cell(&bytes, "C3"), Some(("n".into(), "1".into())));
    // The count has a format of its own: a whole number, not two decimals.
    assert!(
        styles(&bytes).contains("formatCode=\"0\""),
        "{}",
        styles(&bytes)
    );
}

#[test]
fn several_aggregates_in_a_column_are_text() {
    let table = ExportTable {
        columns: vec![column("n", CellFormat::Plain)],
        rows: vec![ExportRow::Aggregates {
            level: None,
            label: None,
            cells: vec![vec![
                aggregate(AggregateKind::Sum, "Sum", Value::Int(7), "7"),
                aggregate(AggregateKind::Count, "Count", Value::Int(2), "2"),
            ]],
        }],
    };
    assert_eq!(text(&write(&table), "A2"), "Sum 7; Count 2");
}

#[test]
fn the_totals_label_and_a_first_column_aggregate_share_the_cell() {
    let table = ExportTable {
        columns: vec![column("n", CellFormat::Plain)],
        rows: vec![ExportRow::Aggregates {
            level: None,
            label: Some("Totals".into()),
            cells: vec![vec![aggregate(
                AggregateKind::Sum,
                "Sum",
                Value::Int(7),
                "7",
            )]],
        }],
    };
    assert_eq!(text(&write(&table), "A2"), "Totals: Sum 7");
}

#[test]
fn columns_under_a_group_get_a_merged_header_row() {
    let mut a = column("a", CellFormat::Plain);
    a.group_path = vec!["Person".into()];
    let mut b = column("b", CellFormat::Plain);
    b.group_path = vec!["Person".into()];
    let c = column("c", CellFormat::Plain);
    let table = ExportTable {
        columns: vec![a, b, c],
        rows: vec![data(vec![int(1), int(2), int(3)])],
    };
    let bytes = write(&table);
    assert_eq!(text(&bytes, "A1"), "Person");
    assert!(sheet(&bytes).contains("<mergeCell ref=\"A1:B1\""));
    // The labels are on the second row and the data on the third, under a pane
    // frozen below the labels.
    assert_eq!(text(&bytes, "A2"), "A");
    assert_eq!(text(&bytes, "C2"), "C");
    assert_eq!(cell(&bytes, "A3"), Some(("n".into(), "1".into())));
    assert!(sheet(&bytes).contains("ySplit=\"2\""));
}

#[test]
fn two_separate_groups_with_the_same_name_are_not_merged() {
    let mut a = column("a", CellFormat::Plain);
    a.group_path = vec!["G".into()];
    let b = column("b", CellFormat::Plain);
    let mut c = column("c", CellFormat::Plain);
    c.group_path = vec!["G".into()];
    let table = ExportTable {
        columns: vec![a, b, c],
        rows: vec![],
    };
    assert!(!sheet(&write(&table)).contains("mergeCell"));
}

#[test]
fn the_sheet_name_is_made_to_fit() {
    let name = |wanted: &str| {
        let bytes = to_xlsx(
            &ExportTable::default(),
            &XlsxOptions {
                sheet_name: wanted.to_owned(),
                ..XlsxOptions::default()
            },
        )
        .unwrap();
        let xml = entry(&bytes, "xl/workbook.xml");
        let start = xml.find("<sheet name=\"").unwrap() + "<sheet name=\"".len();
        xml[start..start + xml[start..].find('"').unwrap()].to_owned()
    };
    assert_eq!(name("people"), "people");
    assert_eq!(name("a/b:c*d?e[f]g\\h"), "a_b_c_d_e_f_g_h");
    assert_eq!(name(&"x".repeat(40)).chars().count(), 31);
    assert_eq!(name(""), "Sheet1");
    assert_eq!(name("history"), "History_");
}

#[test]
fn an_empty_table_is_a_workbook() {
    let bytes = write(&ExportTable::default());
    assert_eq!(&bytes[..2], b"PK");
}

#[cfg(feature = "chrono")]
mod dates {
    use super::*;
    use chrono::{NaiveDate, NaiveDateTime};

    fn date_cell(date: NaiveDate) -> ExportCell {
        ExportCell {
            value: Some(Value::Date(date)),
            text: date.to_string(),
        }
    }

    #[test]
    fn a_date_is_a_date() {
        let table = ExportTable {
            columns: vec![column("d", CellFormat::Plain)],
            rows: vec![data(vec![date_cell(
                NaiveDate::from_ymd_opt(2026, 10, 16).unwrap(),
            )])],
        };
        let bytes = write(&table);
        // Excel counts days from 1900: 46311 is the 16th of October 2026.
        assert_eq!(cell(&bytes, "A2"), Some(("n".into(), "46311".into())));
        assert!(styles(&bytes).contains("formatCode=\"yyyy-mm-dd\""));
    }

    #[test]
    fn a_date_and_time_keeps_the_time() {
        let at = NaiveDate::from_ymd_opt(2026, 10, 16)
            .unwrap()
            .and_hms_opt(18, 0, 0)
            .unwrap();
        let table = ExportTable {
            columns: vec![column("d", CellFormat::Plain)],
            rows: vec![data(vec![ExportCell {
                value: Some(Value::DateTime(at)),
                text: at.to_string(),
            }])],
        };
        let bytes = write(&table);
        assert_eq!(cell(&bytes, "A2"), Some(("n".into(), "46311.75".into())));
        assert!(styles(&bytes).contains("formatCode=\"yyyy-mm-dd hh:mm:ss\""));
        let _: NaiveDateTime = at;
    }

    #[test]
    fn a_pattern_becomes_an_excel_format() {
        let table = ExportTable {
            columns: vec![column("d", CellFormat::DatePattern("%d.%m.%Y".into()))],
            rows: vec![data(vec![date_cell(
                NaiveDate::from_ymd_opt(2026, 10, 16).unwrap(),
            )])],
        };
        assert!(styles(&write(&table)).contains("formatCode=\"dd.mm.yyyy\""));
    }

    #[test]
    fn a_pattern_with_words_quotes_them() {
        let table = ExportTable {
            columns: vec![column("d", CellFormat::DatePattern("%A, %e %B %Y".into()))],
            rows: vec![data(vec![date_cell(
                NaiveDate::from_ymd_opt(2026, 10, 16).unwrap(),
            )])],
        };
        assert!(styles(&write(&table)).contains("formatCode=\"dddd, d mmmm yyyy\""));
    }

    #[test]
    fn a_pattern_excel_cannot_say_falls_back_to_iso() {
        let table = ExportTable {
            columns: vec![column("d", CellFormat::DatePattern("%j".into()))],
            rows: vec![data(vec![date_cell(
                NaiveDate::from_ymd_opt(2026, 10, 16).unwrap(),
            )])],
        };
        assert!(styles(&write(&table)).contains("formatCode=\"yyyy-mm-dd\""));
    }

    #[test]
    fn a_date_before_excel_begins_is_the_text_shown() {
        let table = ExportTable {
            columns: vec![column("d", CellFormat::Plain)],
            rows: vec![data(vec![date_cell(
                NaiveDate::from_ymd_opt(1850, 1, 1).unwrap(),
            )])],
        };
        assert_eq!(text(&write(&table), "A2"), "1850-01-01");
    }
}

#[test]
fn what_the_first_column_aggregates_is_text_that_names_it() {
    let table = ExportTable {
        columns: vec![
            column("name", CellFormat::Plain),
            column("n", CellFormat::Plain),
        ],
        rows: vec![ExportRow::Aggregates {
            level: Some(0),
            label: None,
            cells: vec![
                vec![aggregate(AggregateKind::Count, "Count", Value::Int(3), "3")],
                vec![aggregate(AggregateKind::Sum, "Sum", Value::Int(7), "7")],
            ],
        }],
    };
    let bytes = write(&table);
    // A bare 3 under "NAME" would not say that it is a count.
    assert_eq!(text(&bytes, "A2"), "Count 3");
    // Beside it, in a column of numbers, the sum is a number.
    assert_eq!(cell(&bytes, "B2"), Some(("n".into(), "7".into())));
}
