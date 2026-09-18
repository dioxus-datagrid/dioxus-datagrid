//! Typed filters: every operator against a naive reference, the filter bar's
//! shortcuts, AND and OR, and the value list.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{
    CellValue, ColumnFilter, ColumnId, ColumnSpec, Condition, FilterOp, FilterValue, GridState,
    ValueKind, compute_view, distinct_values,
};
use proptest::prelude::*;

#[derive(Clone, Debug)]
struct Row {
    number: Option<i64>,
    ratio: Option<f64>,
    text: Option<String>,
    flag: Option<bool>,
}

fn columns() -> Vec<ColumnSpec<Row>> {
    vec![
        ColumnSpec::new("number").value_of(|row: &Row| row.number),
        ColumnSpec::new("ratio").value_of(|row: &Row| row.ratio),
        ColumnSpec::new("text")
            .value(|row: &Row| row.text.as_deref().map_or(CellValue::None, CellValue::Text)),
        ColumnSpec::new("flag").value_of(|row: &Row| row.flag),
    ]
}

fn arb_rows() -> impl Strategy<Value = Vec<Row>> {
    prop::collection::vec(
        (
            prop::option::of(-5_i64..5),
            prop::option::of(prop_oneof![(-2.0_f64..2.0), Just(0.5), Just(-0.0)]),
            prop::option::of("[aAbB ]{0,3}"),
            prop::option::of(any::<bool>()),
        )
            .prop_map(|(number, ratio, text, flag)| Row {
                number,
                ratio,
                text,
                flag,
            }),
        0..30,
    )
}

/// Which rows pass `filter` on `column`, by `compute_view`.
fn filtered(rows: &[Row], column: &'static str, filter: ColumnFilter) -> Vec<usize> {
    let mut state = GridState::new();
    state.set_column_filter(column, filter);
    let mut indices = compute_view(rows, &columns(), &state).indices;
    indices.sort_unstable();
    indices
}

/// Which rows satisfy `predicate`, written out the obvious way.
fn reference(rows: &[Row], predicate: impl Fn(&Row) -> bool) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| predicate(row))
        .map(|(index, _)| index)
        .collect()
}

fn numeric_ops() -> impl Strategy<Value = FilterOp> {
    prop::sample::select(vec![
        FilterOp::Equals,
        FilterOp::NotEquals,
        FilterOp::Less,
        FilterOp::LessOrEqual,
        FilterOp::Greater,
        FilterOp::GreaterOrEqual,
        FilterOp::Between,
        FilterOp::IsEmpty,
        FilterOp::IsNotEmpty,
    ])
}

fn text_ops() -> impl Strategy<Value = FilterOp> {
    prop::sample::select(vec![
        FilterOp::Contains,
        FilterOp::StartsWith,
        FilterOp::EndsWith,
        FilterOp::Equals,
        FilterOp::NotEquals,
        FilterOp::IsEmpty,
        FilterOp::IsNotEmpty,
    ])
}

/// The reference for a numeric operator on an optional number.
fn numeric_reference(op: FilterOp, cell: Option<f64>, a: f64, b: f64) -> bool {
    match (op, cell) {
        (FilterOp::IsEmpty, cell) => cell.is_none(),
        (FilterOp::IsNotEmpty, cell) => cell.is_some(),
        (_, None) => false,
        (FilterOp::Equals, Some(x)) => x.total_cmp(&a).is_eq(),
        (FilterOp::NotEquals, Some(x)) => !x.total_cmp(&a).is_eq(),
        (FilterOp::Less, Some(x)) => x.total_cmp(&a).is_lt(),
        (FilterOp::LessOrEqual, Some(x)) => x.total_cmp(&a).is_le(),
        (FilterOp::Greater, Some(x)) => x.total_cmp(&a).is_gt(),
        (FilterOp::GreaterOrEqual, Some(x)) => x.total_cmp(&a).is_ge(),
        (FilterOp::Between, Some(x)) => {
            let (low, high) = if a.total_cmp(&b).is_le() {
                (a, b)
            } else {
                (b, a)
            };
            x.total_cmp(&low).is_ge() && x.total_cmp(&high).is_le()
        }
        _ => unreachable!("not a numeric operator"),
    }
}

fn condition(op: FilterOp, a: FilterValue, b: FilterValue) -> Condition {
    let values = match op.operands() {
        Some(0) => Vec::new(),
        Some(2) => vec![a, b],
        _ => vec![a],
    };
    Condition::new(op, values)
}

proptest! {
    /// Every numeric operator on an integer column, with typed operands.
    #[test]
    fn integer_operators_match_the_reference(
        rows in arb_rows(),
        op in numeric_ops(),
        a in -6_i64..6,
        b in -6_i64..6,
    ) {
        let got = filtered(&rows, "number", condition(op, a.into(), b.into()).into());
        #[allow(clippy::cast_precision_loss)]
        let expected = reference(&rows, |row| {
            numeric_reference(op, row.number.map(|n| n as f64), a as f64, b as f64)
        });
        prop_assert_eq!(got, expected);
    }

    /// The same operators with operands typed as text, as the filter bar and a
    /// server that received text would give them. Floats with a decimal comma.
    #[test]
    fn text_operands_are_read_as_numbers(
        rows in arb_rows(),
        op in numeric_ops(),
        a in -2.0_f64..2.0,
        b in -2.0_f64..2.0,
    ) {
        let text = |value: f64| FilterValue::from(value.to_string().replace('.', ","));
        let got = filtered(&rows, "ratio", condition(op, text(a), text(b)).into());
        let expected = reference(&rows, |row| numeric_reference(op, row.ratio, a, b));
        prop_assert_eq!(got, expected);
    }

    /// Every text operator, ignoring case.
    #[test]
    fn text_operators_match_the_reference(
        rows in arb_rows(),
        op in text_ops(),
        needle in "[aAbB]{0,2}",
    ) {
        let got = filtered(&rows, "text", condition(op, needle.clone().into(), FilterValue::from("")).into());
        let needle_lower = needle.to_lowercase();
        let expected = reference(&rows, |row| {
            let cell = row.text.as_deref();
            let lower = cell.map(str::to_lowercase);
            match op {
                FilterOp::IsEmpty => cell.is_none_or(|text| text.trim().is_empty()),
                FilterOp::IsNotEmpty => cell.is_some_and(|text| !text.trim().is_empty()),
                FilterOp::Contains => lower.is_some_and(|text| text.contains(&needle_lower)),
                FilterOp::StartsWith => lower.is_some_and(|text| text.starts_with(&needle_lower)),
                FilterOp::EndsWith => lower.is_some_and(|text| text.ends_with(&needle_lower)),
                FilterOp::Equals => lower.is_some_and(|text| text == needle_lower),
                FilterOp::NotEquals => lower.is_some_and(|text| text != needle_lower),
                _ => unreachable!("not a text operator"),
            }
        });
        prop_assert_eq!(got, expected);
    }

    /// A value list keeps exactly the rows holding one of the ticked values.
    #[test]
    fn one_of_matches_the_reference(
        rows in arb_rows(),
        ticked in prop::collection::vec(-5_i64..5, 0..4),
    ) {
        let got = filtered(&rows, "number", Condition::one_of(ticked.clone()).into());
        let expected = reference(&rows, |row| row.number.is_some_and(|n| ticked.contains(&n)));
        prop_assert_eq!(got, expected);
    }

    /// Two conditions combined with AND and with OR.
    #[test]
    fn and_and_or_combine_conditions(
        rows in arb_rows(),
        low in -5_i64..5,
        high in -5_i64..5,
    ) {
        let and = ColumnFilter::new(Condition::greater_or_equal(low)).and(Condition::less_or_equal(high));
        let or = ColumnFilter::new(Condition::less(low)).or(Condition::greater(high));

        let got_and = filtered(&rows, "number", and);
        let got_or = filtered(&rows, "number", or);
        let expected_and = reference(&rows, |row| row.number.is_some_and(|n| n >= low && n <= high));
        let expected_or = reference(&rows, |row| row.number.is_some_and(|n| n < low || n > high));
        prop_assert_eq!(got_and, expected_and);
        prop_assert_eq!(got_or, expected_or);
    }

    /// Booleans compare by equality, and read `ja`, `nein` and friends.
    #[test]
    fn booleans_match_the_reference(rows in arb_rows(), wanted in any::<bool>()) {
        let word = if wanted { "ja" } else { "nein" };
        let got = filtered(&rows, "flag", Condition::equals(word).into());
        let expected = reference(&rows, |row| row.flag == Some(wanted));
        prop_assert_eq!(got, expected);
    }

    /// The value list counts every row that passes the other filters, once per
    /// value, and ignores the column's own filter.
    #[test]
    fn distinct_values_count_the_rows_other_filters_let_through(
        rows in arb_rows(),
        own in -5_i64..5,
        flag in any::<bool>(),
    ) {
        let mut state = GridState::new();
        state.set_column_filter("number", Condition::equals(own).into());
        state.set_column_filter("flag", Condition::equals(flag).into());

        let list = distinct_values(&rows, &columns(), &state, &ColumnId::from("number"), 100).unwrap();

        let passing: Vec<&Row> = rows.iter().filter(|row| row.flag == Some(flag)).collect();
        let mut expected: Vec<(FilterValue, usize)> = Vec::new();
        for n in -5_i64..5 {
            let count = passing.iter().filter(|row| row.number == Some(n)).count();
            if count > 0 {
                expected.push((FilterValue::Int(n), count));
            }
        }
        prop_assert_eq!(list.values, expected);
        prop_assert_eq!(list.empty, passing.iter().filter(|row| row.number.is_none()).count());
        prop_assert!(!list.truncated);
    }
}

#[test]
fn the_bar_reads_operators_ranges_and_plain_text() {
    let parse = |text: &str| Condition::from_text(text).map(|c| (c.op, c.values));

    assert_eq!(
        parse(">100"),
        Some((FilterOp::Greater, vec![FilterValue::from("100")]))
    );
    assert_eq!(
        parse(" >= 5 "),
        Some((FilterOp::GreaterOrEqual, vec![FilterValue::from("5")]))
    );
    assert_eq!(
        parse("!=Berlin"),
        Some((FilterOp::NotEquals, vec![FilterValue::from("Berlin")]))
    );
    assert_eq!(
        parse("10..20"),
        Some((
            FilterOp::Between,
            vec![FilterValue::from("10"), FilterValue::from("20")]
        ))
    );
    assert_eq!(
        parse("ber"),
        Some((FilterOp::Contains, vec![FilterValue::from("ber")]))
    );
    // Half-typed input filters nothing rather than everything away.
    assert_eq!(parse(">"), None);
    assert_eq!(parse("   "), None);
}

#[test]
fn the_bar_compares_numbers_and_leaves_text_as_substring() {
    let rows = vec![
        Row {
            number: Some(5),
            ratio: None,
            text: Some("Berlin".into()),
            flag: None,
        },
        Row {
            number: Some(150),
            ratio: None,
            text: Some("Bern".into()),
            flag: None,
        },
    ];
    let count = |column: &'static str, text: &str| {
        let mut state = GridState::new();
        state.set_filter(column, text);
        compute_view(&rows, &columns(), &state).filtered_len
    };

    assert_eq!(count("number", ">100"), 1);
    assert_eq!(
        count("number", "5"),
        1,
        "plain text on a number means equals"
    );
    assert_eq!(count("number", "1..200"), 2);
    assert_eq!(count("text", "ber"), 2);
    assert_eq!(count("text", "=berlin"), 1);
    // Not a number: compares nothing, so nothing passes.
    assert_eq!(count("number", ">abc"), 0);
}

#[test]
fn operators_follow_the_kind() {
    assert!(ValueKind::Text.operators().contains(&FilterOp::StartsWith));
    assert!(!ValueKind::Number.operators().contains(&FilterOp::Contains));
    assert!(ValueKind::Number.operators().contains(&FilterOp::Between));
    for op in ValueKind::Number.operators() {
        assert_eq!(FilterOp::from_name(op.as_str()), Some(*op));
    }
}

#[test]
fn the_kind_comes_from_the_rows_unless_declared() {
    let rows = vec![
        Row {
            number: None,
            ratio: None,
            text: None,
            flag: None,
        },
        Row {
            number: Some(1),
            ratio: None,
            text: None,
            flag: None,
        },
    ];
    let columns = columns();

    assert_eq!(columns[0].value_kind(&rows), Some(ValueKind::Number));
    assert_eq!(columns[2].value_kind(&rows), None);
    assert_eq!(
        columns[2].clone().kind(ValueKind::Text).value_kind(&rows),
        Some(ValueKind::Text)
    );
}

#[test]
fn a_long_value_list_is_truncated_in_sort_order() {
    let rows: Vec<Row> = (0..10)
        .map(|n| Row {
            number: Some(9 - n),
            ratio: None,
            text: None,
            flag: None,
        })
        .collect();
    let list = distinct_values(
        &rows,
        &columns(),
        &GridState::new(),
        &ColumnId::from("number"),
        3,
    )
    .unwrap();

    assert_eq!(
        list.values,
        vec![
            (FilterValue::Int(0), 1),
            (FilterValue::Int(1), 1),
            (FilterValue::Int(2), 1)
        ]
    );
    assert!(list.truncated);
}

#[cfg(feature = "chrono")]
mod dates {
    use super::*;
    use chrono::NaiveDate;

    #[derive(Clone)]
    struct Event {
        on: NaiveDate,
    }

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, d).unwrap()
    }

    #[test]
    fn dates_compare_and_read_german_and_iso_text() {
        let rows: Vec<Event> = (1..=30).map(|d| Event { on: day(d) }).collect();
        let columns = vec![ColumnSpec::new("on").value_of(|event: &Event| event.on)];
        let count = |filter: ColumnFilter| {
            let mut state = GridState::new();
            state.set_column_filter("on", filter);
            compute_view(&rows, &columns, &state).filtered_len
        };

        assert_eq!(count(Condition::less("10.09.2026").into()), 9);
        assert_eq!(count(Condition::greater_or_equal("2026-09-25").into()), 6);
        assert_eq!(count(Condition::between(day(10), day(12)).into()), 3);
        assert_eq!(count(Condition::equals("09/18/2026").into()), 1);

        let mut state = GridState::new();
        state.set_filter("on", "<05.09.2026");
        assert_eq!(compute_view(&rows, &columns, &state).filtered_len, 4);
    }
}
