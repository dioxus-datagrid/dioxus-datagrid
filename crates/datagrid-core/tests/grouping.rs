//! Grouping and aggregates: `ROADMAP.md` phase 10.
//!
//! The property tests check what the phase's acceptance asks for: groups cover
//! the filtered rows completely and without overlap, and aggregates match a
//! naive computation.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp)]

use datagrid_core::{
    Aggregate, AggregateKind, ColumnId, ColumnSpec, GridQuery, GridState, GroupKey, GroupSummary,
    GroupedPage, Page, PageState, SortState, Value, ViewRow, compute_view,
};
use proptest::prelude::*;
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq)]
struct Row {
    id: usize,
    city: String,
    team: Option<i64>,
    score: Option<i64>,
    weight: f64,
}

fn columns() -> Vec<ColumnSpec<Row>> {
    vec![
        ColumnSpec::new("city")
            .value_text(|row: &Row| row.city.as_str())
            .filter_by(|row: &Row| row.city.clone())
            .aggregate(Aggregate::Count)
            .aggregate(Aggregate::Min)
            .aggregate(Aggregate::Max),
        ColumnSpec::new("team").value_of(|row: &Row| row.team),
        ColumnSpec::new("score")
            .value_of(|row: &Row| row.score)
            .aggregate(Aggregate::Sum)
            .aggregate(Aggregate::Average)
            .aggregate(Aggregate::Min)
            .aggregate(Aggregate::Max)
            .aggregate(Aggregate::Count),
        ColumnSpec::new("weight")
            .value_of(|row: &Row| row.weight)
            .aggregate(Aggregate::Sum)
            .aggregate(Aggregate::custom("rows", |rows: &[&Row]| {
                Some(Value::Int(i64::try_from(rows.len()).unwrap()))
            })),
    ]
}

fn arb_rows(max: usize) -> impl Strategy<Value = Vec<Row>> {
    prop::collection::vec(
        (
            "[aAbB]{0,2}",
            prop::option::of(0_i64..3),
            prop::option::of(-5_i64..5),
            -4_i32..4,
        ),
        0..=max,
    )
    .prop_map(|items| {
        items
            .into_iter()
            .enumerate()
            .map(|(id, (city, team, score, weight))| Row {
                id,
                city,
                team,
                score,
                weight: f64::from(weight) / 2.0,
            })
            .collect()
    })
}

fn arb_group_by() -> impl Strategy<Value = Vec<ColumnId>> {
    prop::sample::select(vec![
        vec!["city"],
        vec!["team"],
        vec!["city", "team"],
        vec!["team", "city"],
        vec!["team", "unknown"],
    ])
    .prop_map(|ids| ids.into_iter().map(ColumnId::from).collect())
}

/// Every group's rows, as the view's data rows under its header, when every
/// group is expanded and nothing is paged.
fn members(view: &datagrid_core::View) -> Vec<(usize, Vec<usize>)> {
    let mut open: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut done = Vec::new();
    for row in &view.rows {
        match *row {
            ViewRow::GroupHeader(group) => {
                let level = view.groups[group].level;
                while open.len() > level {
                    done.push(open.pop().unwrap());
                }
                open.push((group, Vec::new()));
            }
            ViewRow::Data(index) => {
                for (_, rows) in &mut open {
                    rows.push(index);
                }
            }
            ViewRow::GroupFooter(_) | _ => {}
        }
    }
    done.extend(open);
    done
}

fn naive_sum(values: &[i64]) -> Option<Value> {
    (!values.is_empty()).then(|| Value::Int(values.iter().sum()))
}

proptest! {
    /// Each level of groups splits the filtered rows into parts that cover
    /// them all and share none, and every row in a group holds its value.
    #[test]
    fn groups_cover_the_filtered_rows_without_overlap(
        rows in arb_rows(40),
        group_by in arb_group_by(),
        needle in "[ab]{0,1}",
    ) {
        let mut state = GridState::new();
        state.set_filter("city", needle);
        state.set_group_by(group_by);
        let columns = columns();
        let view = compute_view(&rows, &columns, &state);

        let filtered: HashSet<usize> = compute_view(
            &rows,
            &columns,
            &GridState { group_by: Vec::new(), ..state.clone() },
        )
        .indices
        .into_iter()
        .collect();
        prop_assert_eq!(view.filtered_len, filtered.len());
        prop_assert_eq!(view.indices.len(), filtered.len());
        prop_assert_eq!(view.indices.iter().copied().collect::<HashSet<_>>(), filtered.clone());

        let groups = members(&view);
        for level in 0..view.group_levels {
            let mut seen = HashSet::new();
            for (group, rows_in_group) in groups.iter().filter(|(g, _)| view.groups[*g].level == level) {
                let spec = &view.groups[*group];
                prop_assert_eq!(spec.count, rows_in_group.len());
                prop_assert!(spec.count > 0);
                let column = columns.iter().find(|c| c.id == spec.column).unwrap();
                for &index in rows_in_group {
                    prop_assert!(seen.insert(index), "row {} in two groups", index);
                    let value = Value::from_cell(&column.read(&rows[index]));
                    prop_assert_eq!(&value, &spec.value);
                }
            }
            prop_assert_eq!(&seen, &filtered);
        }

        // Siblings are numbered 1..=n, and their keys are distinct.
        let keys: HashSet<&GroupKey> = view.groups.iter().map(|group| &group.key).collect();
        prop_assert_eq!(keys.len(), view.groups.len());
        for group in &view.groups {
            prop_assert!(group.position >= 1 && group.position <= group.siblings);
            prop_assert_eq!(group.key.depth(), group.level + 1);
        }
    }

    /// Group and grid aggregates equal the same computation done by hand.
    #[test]
    fn aggregates_match_a_naive_computation(rows in arb_rows(40), group_by in arb_group_by()) {
        let mut state = GridState::new();
        state.set_group_by(group_by);
        let view = compute_view(&rows, &columns(), &state);

        let mut parts: Vec<(Vec<usize>, Vec<datagrid_core::AggregateValue>)> = members(&view)
            .into_iter()
            .map(|(group, members)| (members, view.groups[group].aggregates.clone()))
            .collect();
        parts.push(((0..rows.len()).collect(), view.totals.clone()));

        for (members, aggregates) in parts {
            let find = |column: &str, kind: AggregateKind| {
                aggregates
                    .iter()
                    .find(|a| a.column.as_str() == column && a.kind == kind)
                    .unwrap()
                    .value
                    .clone()
            };
            let scores: Vec<i64> = members.iter().filter_map(|&i| rows[i].score).collect();
            prop_assert_eq!(find("score", AggregateKind::Sum), naive_sum(&scores));
            prop_assert_eq!(find("score", AggregateKind::Min), scores.iter().min().map(|&v| Value::Int(v)));
            prop_assert_eq!(find("score", AggregateKind::Max), scores.iter().max().map(|&v| Value::Int(v)));
            prop_assert_eq!(find("score", AggregateKind::Count), Some(Value::Int(scores.len() as i64)));
            #[allow(clippy::cast_precision_loss)]
            let mean = (!scores.is_empty())
                .then(|| Value::Float(scores.iter().sum::<i64>() as f64 / scores.len() as f64));
            prop_assert_eq!(find("score", AggregateKind::Average), mean);

            let weights: f64 = members.iter().map(|&i| rows[i].weight).sum();
            let expected = (!members.is_empty()).then_some(Value::Float(weights));
            prop_assert_eq!(find("weight", AggregateKind::Sum), expected);
            prop_assert_eq!(
                find("weight", AggregateKind::Custom("rows".into())),
                Some(Value::Int(members.len() as i64))
            );

            // Text: the smallest and largest in the column's case-insensitive order.
            let mut cities: Vec<&str> = members.iter().map(|&i| rows[i].city.as_str()).collect();
            cities.sort_by(|a, b| {
                a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
            });
            prop_assert_eq!(find("city", AggregateKind::Min), cities.first().map(|c| Value::Text((*c).to_owned())));
            prop_assert_eq!(find("city", AggregateKind::Max), cities.last().map(|c| Value::Text((*c).to_owned())));
        }
    }

    /// Walking every page of a grouped view yields the unpaged rows once each.
    #[test]
    fn grouped_pages_cover_every_row_once(
        rows in arb_rows(40),
        group_by in arb_group_by(),
        page_size in 1_usize..8,
    ) {
        let mut state = GridState::new();
        state.set_group_by(group_by);
        let unpaged = compute_view(&rows, &columns(), &state);

        state.page = Some(PageState::new(page_size));
        let first = compute_view(&rows, &columns(), &state);
        prop_assert_eq!(first.row_count, unpaged.rows.len());
        prop_assert_eq!(first.page_count, unpaged.rows.len().div_ceil(page_size));

        let mut walked = Vec::new();
        for page in 0..first.page_count {
            state.set_page(page);
            let view = compute_view(&rows, &columns(), &state);
            prop_assert_eq!(view.row_offset, walked.len());
            prop_assert!(view.len() <= page_size);
            walked.extend(view.rows);
        }
        prop_assert_eq!(walked, unpaged.rows);
    }
}

fn rows() -> Vec<Row> {
    let row = |id, city: &str, team, score| Row {
        id,
        city: city.into(),
        team,
        score,
        weight: 1.0,
    };
    vec![
        row(0, "Berlin", Some(1), Some(10)),
        row(1, "Hamburg", Some(2), Some(5)),
        row(2, "Berlin", Some(2), None),
        row(3, "Köln", None, Some(7)),
        row(4, "Hamburg", Some(1), Some(1)),
    ]
}

fn header_values(view: &datagrid_core::View) -> Vec<(usize, Option<Value>, usize)> {
    view.rows
        .iter()
        .filter_map(|row| match *row {
            ViewRow::GroupHeader(group) => {
                let group = &view.groups[group];
                Some((group.level, group.value.clone(), group.count))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn groups_follow_the_sort_direction_of_their_column() {
    let mut state = GridState::new();
    state.set_group_by(vec!["city".into()]);
    let view = compute_view(&rows(), &columns(), &state);
    let text = |city: &str| Some(Value::Text(city.into()));
    assert_eq!(
        header_values(&view),
        [
            (0, text("Berlin"), 2),
            (0, text("Hamburg"), 2),
            (0, text("Köln"), 1)
        ]
    );

    state.sort = vec![SortState::desc("city"), SortState::asc("score")];
    let view = compute_view(&rows(), &columns(), &state);
    assert_eq!(
        header_values(&view),
        [
            (0, text("Köln"), 1),
            (0, text("Hamburg"), 2),
            (0, text("Berlin"), 2)
        ]
    );
    // Inside a group, the rest of the sort applies: Hamburg by score.
    let hamburg = view
        .rows
        .iter()
        .skip_while(|row| **row != ViewRow::GroupHeader(1))
        .skip(1)
        .take(2)
        .map(|row| row.data_index().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(hamburg, [4, 1]);
}

#[test]
fn rows_without_a_value_form_the_last_group() {
    let mut state = GridState::new();
    state.set_group_by(vec!["team".into()]);
    let view = compute_view(&rows(), &columns(), &state);
    assert_eq!(
        header_values(&view),
        [
            (0, Some(Value::Int(1)), 2),
            (0, Some(Value::Int(2)), 2),
            (0, None, 1)
        ]
    );
}

#[test]
fn a_collapsed_group_is_its_header_alone() {
    let mut state = GridState::new();
    state.set_group_by(vec!["city".into()]);
    let berlin = GroupKey(vec![Some(Value::Text("Berlin".into()))]);
    state.toggle_group(&berlin);
    assert!(!state.is_group_expanded(&berlin));

    let view = compute_view(&rows(), &columns(), &state);
    assert_eq!(view.rows[0], ViewRow::GroupHeader(0));
    assert!(!view.groups[0].expanded);
    // Straight on to Hamburg's header: no rows, no footer.
    assert_eq!(view.rows[1], ViewRow::GroupHeader(1));
    // The collapsed group still counts its rows and aggregates them.
    assert_eq!(view.groups[0].count, 2);
    assert_eq!(
        view.groups[0]
            .aggregate(&"score".into(), &AggregateKind::Sum)
            .unwrap()
            .value,
        Some(Value::Int(10))
    );
    assert_eq!(view.filtered_len, 5);
    assert_eq!(view.indices, [1, 4, 3]);

    // Collapsing everything leaves the headers; expanding it all again brings
    // every row back, Berlin included.
    state.set_all_groups_expanded(false);
    let view = compute_view(&rows(), &columns(), &state);
    assert!(
        view.rows
            .iter()
            .all(|row| matches!(row, ViewRow::GroupHeader(_)))
    );
    state.toggle_group(&berlin);
    let view = compute_view(&rows(), &columns(), &state);
    assert_eq!(view.indices, [0, 2]);
    state.set_all_groups_expanded(true);
    let view = compute_view(&rows(), &columns(), &state);
    assert_eq!(view.indices.len(), 5);
}

#[test]
fn footers_follow_expanded_groups_when_a_column_aggregates() {
    let mut state = GridState::new();
    state.set_group_by(vec!["city".into(), "team".into()]);
    let view = compute_view(&rows(), &columns(), &state);
    // Berlin: header, team 1 (header, row, footer), team 2 (header, row,
    // footer), footer.
    assert_eq!(
        view.rows[..8],
        [
            ViewRow::GroupHeader(0),
            ViewRow::GroupHeader(1),
            ViewRow::Data(0),
            ViewRow::GroupFooter(1),
            ViewRow::GroupHeader(2),
            ViewRow::Data(2),
            ViewRow::GroupFooter(2),
            ViewRow::GroupFooter(0),
        ]
    );
    assert_eq!(
        view.groups[1].key.parent(),
        Some(view.groups[0].key.clone())
    );
    assert_eq!(view.group_levels, 2);

    // Without aggregates there is nothing for a footer to show.
    let plain: Vec<ColumnSpec<Row>> =
        vec![ColumnSpec::new("city").value_text(|row: &Row| row.city.as_str())];
    let view = compute_view(&rows(), &plain, &state);
    assert!(
        !view
            .rows
            .iter()
            .any(|row| matches!(row, ViewRow::GroupFooter(_)))
    );
    assert!(view.totals.is_empty());
}

#[test]
fn changing_the_grouping_expands_everything_and_returns_to_the_first_page() {
    let mut state = GridState::paged(2);
    state.set_group_by(vec!["city".into()]);
    state.set_all_groups_expanded(false);
    state.set_page(1);
    state.group_by_column("team", Some(0));
    assert_eq!(
        state.group_by,
        [ColumnId::from("team"), ColumnId::from("city")]
    );
    assert!(!state.groups_collapsed);
    assert_eq!(state.page.unwrap().index, 0);
    state.ungroup_column(&"team".into());
    assert_eq!(state.group_by, [ColumnId::from("city")]);
}

#[test]
fn sums_turn_to_floats_on_overflow_and_mixed_numbers() {
    #[derive(Clone)]
    struct N(Option<f64>, i64);
    let columns = vec![
        ColumnSpec::new("int")
            .value_of(|n: &N| n.1)
            .aggregate(Aggregate::Sum),
        ColumnSpec::new("float")
            .value_of(|n: &N| n.0)
            .aggregate(Aggregate::Sum)
            .aggregate(Aggregate::Average)
            .aggregate(Aggregate::Count),
    ];
    let rows = vec![N(Some(1.5), i64::MAX), N(None, 1)];
    let view = compute_view(&rows, &columns, &GridState::new());
    let value = |column: &str, kind| {
        view.totals
            .iter()
            .find(|a| a.column.as_str() == column && a.kind == kind)
            .unwrap()
            .value
            .clone()
    };
    #[allow(clippy::cast_precision_loss)]
    let overflowed = i64::MAX as f64 + 1.0;
    assert_eq!(
        value("int", AggregateKind::Sum),
        Some(Value::Float(overflowed))
    );
    assert_eq!(value("float", AggregateKind::Sum), Some(Value::Float(1.5)));
    assert_eq!(
        value("float", AggregateKind::Average),
        Some(Value::Float(1.5))
    );
    assert_eq!(value("float", AggregateKind::Count), Some(Value::Int(1)));

    // Nothing to add up is no sum at all, not zero.
    let view = compute_view(&[N(None, 0)], &columns, &GridState::new());
    assert_eq!(view.totals[1].value, None);
}

#[cfg(feature = "serde")]
#[test]
fn grouping_state_round_trips() {
    let mut state = GridState::new();
    state.set_group_by(vec!["city".into(), "team".into()]);
    state.toggle_group(&GroupKey(vec![Some(Value::Text("Berlin".into())), None]));
    let json = serde_json::to_string(&state).unwrap();
    let back: GridState = serde_json::from_str(&json).unwrap();
    assert_eq!(back, state);

    // State saved before grouping existed still loads.
    let old: GridState = serde_json::from_str(r#"{"sort":[]}"#).unwrap();
    assert!(old.group_by.is_empty());
}

/// The groups of each level with their counts and aggregates, as a server
/// would count them, in display order; taken from the local grouping with
/// every group expanded.
fn summaries(
    rows: &[Row],
    columns: &[ColumnSpec<Row>],
    state: &GridState,
) -> Vec<Vec<GroupSummary>> {
    let all = GridState {
        page: None,
        groups_collapsed: false,
        toggled_groups: Vec::new(),
        ..state.clone()
    };
    let view = compute_view(rows, columns, &all);
    (0..view.group_levels)
        .map(|level| {
            view.groups
                .iter()
                .filter(|group| group.level == level)
                .map(|group| GroupSummary {
                    key: group.key.clone(),
                    count: group.count,
                    aggregates: group.aggregates.clone(),
                })
                .collect()
        })
        .collect()
}

/// A group's rows in display order, as a server would select them.
fn rows_of(
    rows: &[Row],
    columns: &[ColumnSpec<Row>],
    state: &GridState,
    key: &GroupKey,
) -> Vec<Row> {
    let flat = GridState {
        page: None,
        groups_collapsed: false,
        toggled_groups: Vec::new(),
        ..state.clone()
    };
    let view = compute_view(rows, columns, &flat);
    let path = |row: &Row| {
        GroupKey(
            state
                .group_by
                .iter()
                .filter_map(|id| columns.iter().find(|column| &column.id == id))
                .map(|column| Value::from_cell(&column.read(row)))
                .collect(),
        )
    };
    view.indices
        .iter()
        .map(|&index| &rows[index])
        .filter(|row| &path(row) == key)
        .cloned()
        .collect()
}

proptest! {
    /// A server that counts groups and fetches only the rows a page shows
    /// answers exactly like the grid would locally, headers, footers and
    /// totals included.
    #[test]
    fn a_planned_page_equals_the_local_one(
        rows in arb_rows(30),
        group_by in prop::sample::select(vec![vec!["city"], vec!["team"], vec!["city", "team"], vec!["team", "city"]]),
        page_size in 1_usize..9,
        page in 0_usize..12,
        collapse in prop::collection::vec(any::<bool>(), 6),
        all_collapsed in any::<bool>(),
        aggregates in any::<bool>(),
    ) {
        let columns = if aggregates {
            columns()
        } else {
            columns().into_iter().map(|mut column| { column.aggregates.clear(); column }).collect()
        };
        let mut state = GridState::paged(page_size);
        state.set_group_by(group_by.into_iter().map(ColumnId::from).collect());
        state.set_all_groups_expanded(!all_collapsed);
        // Toggle some of the groups there are.
        let unpaged = compute_view(&rows, &columns, &GridState { page: None, ..state.clone() });
        for (group, toggle) in unpaged.groups.iter().zip(&collapse) {
            if *toggle {
                state.toggle_group(&group.key);
            }
        }
        state.set_page(page);

        let local = compute_view(&rows, &columns, &state);
        let expected = Page::from_view(&local, &rows);

        let query = GridQuery::from_state(&state, page_size).with_aggregates(&columns);
        let plan = GroupedPage::plan(&summaries(&rows, &columns, &state), &query);
        let fetched: Vec<Vec<Row>> = plan
            .fetches()
            .map(|(key, offset, limit)| {
                rows_of(&rows, &columns, &state, key).into_iter().skip(offset).take(limit).collect()
            })
            .collect();
        let page = plan.into_page(fetched, local.totals.clone());

        prop_assert_eq!(page, expected);
    }
}

#[test]
fn a_page_from_a_view_numbers_its_rows_and_groups_from_zero() {
    let mut state = GridState::paged(4);
    state.set_group_by(vec!["city".into()]);
    state.set_page(1);
    let view = compute_view(&rows(), &columns(), &state);
    let page = Page::from_view(&view, &rows());
    // Berlin fills the first page: header, two rows, footer. The second is
    // Hamburg's, numbered from zero again.
    assert_eq!(
        page.layout,
        [
            ViewRow::GroupHeader(0),
            ViewRow::Data(0),
            ViewRow::Data(1),
            ViewRow::GroupFooter(0),
        ]
    );
    assert_eq!(page.groups[0].value, Some(Value::Text("Hamburg".into())));
    assert_eq!(page.row_count, Some(11));
    assert_eq!(page.total, 5);
    assert_eq!(
        page.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        [1, 4]
    );

    // Without grouping, a page is its rows, as before.
    let plain = Page::from_view(
        &compute_view(&rows(), &columns(), &GridState::paged(2)),
        &rows(),
    );
    assert!(plain.layout.is_empty());
    assert_eq!(plain.row_count, None);
}
