//! The typed cell value: sorting through it matches the order from before it
//! existed, and formatting renders it per locale.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{
    CellAlign, CellFormat, CellValue, ColumnSpec, GridLocale, GridState, SortDirection, SortState,
    TextCollation, compute_view,
};
use proptest::prelude::*;
use std::cmp::Ordering;

/// A value as a row stores it, owning its text.
#[derive(Clone, Debug)]
enum Stored {
    Missing,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

impl Stored {
    fn cell(&self) -> CellValue<'_> {
        match self {
            Self::Missing => CellValue::None,
            Self::Bool(value) => CellValue::Bool(*value),
            Self::Int(value) => CellValue::Int(*value),
            Self::Float(value) => CellValue::Float(*value),
            Self::Text(value) => CellValue::Text(value),
        }
    }
}

/// The order of 0.4.0, written out independently as the reference: `Bool` <
/// numbers < `Text` < missing, numbers compared across `Int` and `Float`, text
/// case-insensitively with a case-sensitive tiebreak.
fn reference_cmp(a: &Stored, b: &Stored, collation: TextCollation) -> Ordering {
    fn rank(value: &Stored) -> u8 {
        match value {
            Stored::Bool(_) => 0,
            Stored::Int(_) | Stored::Float(_) => 1,
            Stored::Text(_) => 2,
            Stored::Missing => 3,
        }
    }
    #[allow(clippy::cast_precision_loss)]
    fn number(value: &Stored) -> f64 {
        match value {
            Stored::Int(value) => *value as f64,
            Stored::Float(value) => *value,
            _ => 0.0,
        }
    }

    match (a, b) {
        (Stored::Bool(a), Stored::Bool(b)) => a.cmp(b),
        (Stored::Int(a), Stored::Int(b)) => a.cmp(b),
        (Stored::Int(_) | Stored::Float(_), Stored::Int(_) | Stored::Float(_)) => {
            number(a).total_cmp(&number(b))
        }
        (Stored::Text(a), Stored::Text(b)) => match collation {
            TextCollation::CaseSensitive => a.cmp(b),
            TextCollation::CaseInsensitive => a
                .to_lowercase()
                .cmp(&b.to_lowercase())
                .then_with(|| a.cmp(b)),
        },
        _ => rank(a).cmp(&rank(b)),
    }
}

#[derive(Clone, Debug)]
struct Row {
    first: Stored,
    second: Stored,
}

fn arb_stored() -> impl Strategy<Value = Stored> {
    prop_oneof![
        Just(Stored::Missing),
        any::<bool>().prop_map(Stored::Bool),
        (-3_i64..3).prop_map(Stored::Int),
        prop_oneof![
            (-3.0_f64..3.0),
            Just(f64::NAN),
            Just(f64::INFINITY),
            Just(-0.0)
        ]
        .prop_map(Stored::Float),
        "[aAbBäÄ]{0,3}".prop_map(Stored::Text),
    ]
}

fn columns(collation: TextCollation) -> Vec<ColumnSpec<Row>> {
    vec![
        ColumnSpec::new("first")
            .value(|row: &Row| row.first.cell())
            .collation(collation),
        ColumnSpec::new("second")
            .value(|row: &Row| row.second.cell())
            .collation(collation),
    ]
}

proptest! {
    /// Sorting through `CellValue` puts rows in exactly the order the 0.4.0
    /// sort did, for every mix of variants, directions and collations.
    #[test]
    fn sorting_through_cell_value_matches_the_previous_order(
        rows in prop::collection::vec((arb_stored(), arb_stored()), 0..40),
        directions in (any::<bool>(), any::<bool>()),
        case_sensitive in any::<bool>(),
    ) {
        let rows: Vec<Row> = rows
            .into_iter()
            .map(|(first, second)| Row { first, second })
            .collect();
        let collation = if case_sensitive {
            TextCollation::CaseSensitive
        } else {
            TextCollation::CaseInsensitive
        };
        let direction = |desc| if desc { SortDirection::Desc } else { SortDirection::Asc };
        let (first, second) = (direction(directions.0), direction(directions.1));
        let state = GridState {
            sort: vec![SortState::new("first", first), SortState::new("second", second)],
            ..GridState::new()
        };

        let got = compute_view(&rows, &columns(collation), &state).indices;

        let mut expected: Vec<usize> = (0..rows.len()).collect();
        expected.sort_by(|&a, &b| {
            first
                .apply(reference_cmp(&rows[a].first, &rows[b].first, collation))
                .then_with(|| second.apply(reference_cmp(&rows[a].second, &rows[b].second, collation)))
        });

        prop_assert_eq!(got, expected);
    }
}

#[test]
fn the_sort_shortcuts_set_the_value() {
    struct User {
        name: String,
        age: u32,
    }
    let user = User {
        name: "Ada".into(),
        age: 36,
    };

    let name = ColumnSpec::new("name").sort_by_text(|user: &User| user.name.as_str());
    let age = ColumnSpec::new("age").sort_by_value(|user: &User| user.age);

    assert_eq!(name.read(&user), CellValue::Text("Ada"));
    assert_eq!(age.read(&user), CellValue::Int(36));
    assert!(name.is_sortable() && age.is_sortable());
}

#[test]
fn a_value_can_be_kept_out_of_sorting() {
    let column = ColumnSpec::new("n").value_of(|n: &i64| *n).sortable(false);
    let state = GridState {
        sort: vec![SortState::asc("n")],
        ..GridState::new()
    };

    assert!(!column.is_sortable());
    let rows = vec![3_i64, 1, 2];
    assert_eq!(compute_view(&rows, &[column], &state).indices, [0, 1, 2]);
}

#[test]
fn a_column_without_value_reads_none() {
    let column = ColumnSpec::<u8>::new("empty");
    assert!(column.read(&1).is_none());
    assert_eq!(column.display_text(&1, &GridLocale::english()), None);
}

fn format(locale: &GridLocale, value: CellValue<'_>, format: &CellFormat) -> String {
    locale.format(&value, format)
}

#[test]
fn numbers_are_grouped_and_rounded_per_locale() {
    let (en, de) = (GridLocale::english(), GridLocale::german());
    let two = CellFormat::number(2);

    assert_eq!(format(&en, CellValue::Float(1234.5), &two), "1,234.50");
    assert_eq!(format(&de, CellValue::Float(1234.5), &two), "1.234,50");
    assert_eq!(
        format(&en, CellValue::Int(-1_234_567), &two),
        "-1,234,567.00"
    );
    assert_eq!(
        format(&en, CellValue::Int(999), &CellFormat::number(0)),
        "999"
    );
    assert_eq!(
        format(&en, CellValue::Float(0.125), &CellFormat::number(1)),
        "0.1"
    );
    // Rounding to zero drops the sign.
    assert_eq!(format(&en, CellValue::Float(-0.001), &two), "0.00");
}

#[test]
fn plain_keeps_the_value_and_localizes_only_the_decimal_separator() {
    let (en, de) = (GridLocale::english(), GridLocale::german());

    assert_eq!(
        format(&en, CellValue::Int(12345), &CellFormat::Plain),
        "12345"
    );
    assert_eq!(
        format(&de, CellValue::Float(1.5), &CellFormat::Plain),
        "1,5"
    );
    assert_eq!(format(&de, CellValue::Bool(true), &CellFormat::Plain), "Ja");
    assert_eq!(
        format(&en, CellValue::Bool(false), &CellFormat::Plain),
        "No"
    );
    assert_eq!(format(&en, CellValue::None, &CellFormat::number(2)), "");
    // Text under a number format stays text.
    assert_eq!(
        format(&en, CellValue::Text("n/a"), &CellFormat::number(2)),
        "n/a"
    );
}

#[test]
fn currency_and_percent_follow_the_locale() {
    let (en, de) = (GridLocale::english(), GridLocale::german());
    let euro = CellFormat::currency("€", 2);
    let dollar = CellFormat::currency("$", 2);

    assert_eq!(
        format(&de, CellValue::Float(1234.5), &euro),
        "1.234,50\u{a0}€"
    );
    assert_eq!(format(&en, CellValue::Float(-3.0), &dollar), "-$3.00");
    assert_eq!(
        format(&en, CellValue::Int(5), &CellFormat::currency("CHF", 0)),
        "CHF\u{a0}5"
    );
    assert_eq!(
        format(&en, CellValue::Float(0.256), &CellFormat::percent(1)),
        "25.6%"
    );
    assert_eq!(
        format(&de, CellValue::Float(0.25), &CellFormat::percent(0)),
        "25\u{a0}%"
    );
}

#[test]
fn numeric_formats_align_at_the_end() {
    let plain = ColumnSpec::<u8>::new("a");
    let money = ColumnSpec::<u8>::new("b").format(CellFormat::currency("€", 2));
    let centered = ColumnSpec::<u8>::new("c")
        .format(CellFormat::number(0))
        .align(CellAlign::Center);

    assert_eq!(plain.effective_align(), CellAlign::Start);
    assert_eq!(money.effective_align(), CellAlign::End);
    assert_eq!(centered.effective_align(), CellAlign::Center);
}

#[test]
fn texts_fill_their_placeholders() {
    let (en, de) = (GridLocale::english(), GridLocale::german());

    assert_eq!(en.page_of(2, 5), "Page 2 of 5");
    assert_eq!(de.page_of(1, 1200), "Seite 1 von 1.200");
    assert_eq!(en.row_count(1), "1 row");
    assert_eq!(en.row_count(100_000), "100,000 rows");
    assert_eq!(de.row_count(0), "0 Zeilen");
    assert_eq!(de.selected_count(3), "3 ausgewählt");
    assert_eq!(de.filter_column("Name"), "Name filtern");
}

#[cfg(feature = "chrono")]
mod dates {
    use super::format;
    use chrono::{NaiveDate, NaiveDateTime};
    use datagrid_core::{CellFormat, CellValue, GridLocale};

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 18).unwrap()
    }

    fn date_time() -> NaiveDateTime {
        date().and_hms_opt(14, 5, 0).unwrap()
    }

    #[test]
    fn dates_use_the_locale_pattern() {
        let (en, de) = (GridLocale::english(), GridLocale::german());

        assert_eq!(
            format(&en, CellValue::Date(date()), &CellFormat::Plain),
            "09/18/2026"
        );
        assert_eq!(
            format(&de, CellValue::Date(date()), &CellFormat::Plain),
            "18.09.2026"
        );
        assert_eq!(
            format(&de, CellValue::DateTime(date_time()), &CellFormat::Plain),
            "18.09.2026 14:05"
        );
        assert_eq!(
            format(&de, CellValue::DateTime(date_time()), &CellFormat::Date),
            "18.09.2026"
        );
        assert_eq!(
            format(
                &en,
                CellValue::Date(date()),
                &CellFormat::date_pattern("%Y-%m-%d")
            ),
            "2026-09-18"
        );
    }

    #[test]
    fn an_invalid_pattern_falls_back_to_iso() {
        let en = GridLocale::english();
        assert_eq!(
            format(
                &en,
                CellValue::Date(date()),
                &CellFormat::date_pattern("%Q")
            ),
            "2026-09-18"
        );
    }

    #[test]
    fn dates_and_date_times_order_chronologically() {
        let midnight = date().and_hms_opt(0, 0, 0).unwrap();
        assert_eq!(CellValue::Date(date()), CellValue::DateTime(midnight));
        assert!(CellValue::Date(date()) < CellValue::DateTime(date_time()));
        assert!(CellValue::DateTime(date_time()) < CellValue::Text("a"));
        assert!(CellValue::Int(i64::MAX) < CellValue::Date(date()));
    }
}
