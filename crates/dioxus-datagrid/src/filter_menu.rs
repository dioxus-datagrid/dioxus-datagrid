//! The filter menu: a button that opens a panel for one column's typed filter,
//! either as conditions or as a list of values to tick.

use crate::GridHandle;
use datagrid_core::{
    ColumnFilter, ColumnId, Condition, DistinctValues, FilterOp, GridRow as GridRowKey, Value,
    ValueKind,
};
use dioxus::prelude::*;
use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

/// How many values a value list shows unless told otherwise.
pub const DEFAULT_VALUE_LIMIT: usize = 1_000;

thread_local! {
    static NEXT_ID: Cell<u64> = const { Cell::new(0) };
}

/// A page-unique id for a panel, for `aria-controls`.
pub(crate) fn next_id() -> u64 {
    NEXT_ID.with(|next| {
        let id = next.get();
        next.set(id.wrapping_add(1));
        id
    })
}

/// Which half of the panel is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Conditions,
    Values,
}

/// One condition as the form holds it: text as typed, not yet read.
#[derive(Clone, Debug, Default, PartialEq)]
struct DraftCondition {
    op: Option<FilterOp>,
    first: String,
    second: String,
}

impl DraftCondition {
    fn from_condition(condition: &Condition) -> Self {
        Self {
            op: Some(condition.op),
            first: condition.values.first().map(edit_text).unwrap_or_default(),
            second: condition.values.get(1).map(edit_text).unwrap_or_default(),
        }
    }

    /// The condition, or `None` if it is incomplete: no operator, or an
    /// operand missing.
    fn build(&self) -> Option<Condition> {
        let op = self.op?;
        let values = match op.operands() {
            Some(0) => Vec::new(),
            Some(2) => {
                if self.first.trim().is_empty() || self.second.trim().is_empty() {
                    return None;
                }
                vec![
                    Value::from(self.first.trim()),
                    Value::from(self.second.trim()),
                ]
            }
            _ => {
                if self.first.trim().is_empty() {
                    return None;
                }
                vec![Value::from(self.first.trim())]
            }
        };
        Some(Condition::new(op, values))
    }
}

/// Everything the panel lets the user change before applying it.
#[derive(Clone, Debug, PartialEq)]
struct Draft {
    mode: Mode,
    first: DraftCondition,
    second: DraftCondition,
    any: bool,
    /// The ticked values, or `None` for "every value", which is no filter.
    ticked: Option<HashSet<Value>>,
    /// Whether rows without a value are ticked.
    empty: bool,
    search: String,
}

impl Draft {
    /// The form for a column's current filter.
    fn from_filter(filter: Option<&ColumnFilter>, kind: ValueKind) -> Self {
        let mut draft = Self {
            mode: Mode::Conditions,
            first: DraftCondition {
                op: kind.operators().first().copied(),
                ..DraftCondition::default()
            },
            second: DraftCondition::default(),
            any: false,
            ticked: None,
            empty: true,
            search: String::new(),
        };
        let Some(filter) = filter else {
            return draft;
        };

        // A value list: one-of, possibly or-ed with "is empty".
        let one_of = filter
            .conditions
            .iter()
            .find(|condition| condition.op == FilterOp::OneOf);
        if let Some(one_of) = one_of {
            draft.mode = Mode::Values;
            draft.ticked = Some(one_of.values.iter().cloned().collect());
            draft.empty = filter.any
                && filter
                    .conditions
                    .iter()
                    .any(|condition| condition.op == FilterOp::IsEmpty);
            return draft;
        }

        if let Some(first) = filter.conditions.first() {
            draft.first = DraftCondition::from_condition(first);
        }
        if let Some(second) = filter.conditions.get(1) {
            draft.second = DraftCondition::from_condition(second);
        }
        draft.any = filter.any;
        draft
    }

    /// The filter to apply. Empty means "no filter".
    fn build(&self) -> ColumnFilter {
        match self.mode {
            Mode::Conditions => {
                let conditions: Vec<Condition> = [&self.first, &self.second]
                    .into_iter()
                    .filter_map(DraftCondition::build)
                    .collect();
                ColumnFilter {
                    any: self.any && conditions.len() > 1,
                    conditions,
                }
            }
            Mode::Values => match &self.ticked {
                None if self.empty => ColumnFilter::default(),
                // Every value, but not the empty rows.
                None => ColumnFilter::new(Condition::is_not_empty()),
                Some(ticked) => {
                    let mut values: Vec<Value> = ticked.iter().cloned().collect();
                    values.sort_by(|a, b| a.as_cell().cmp(&b.as_cell()));
                    let one_of = Condition::one_of(values);
                    if self.empty {
                        ColumnFilter::new(one_of).or(Condition::is_empty())
                    } else {
                        ColumnFilter::new(one_of)
                    }
                }
            },
        }
    }

    /// [`build`](Draft::build), but a value list with every value ticked is no
    /// filter at all, rather than a list naming each of them. Only when the
    /// whole list is known, so not for a truncated one.
    fn build_against(&self, list: Option<&DistinctValues>) -> ColumnFilter {
        if let (Mode::Values, Some(ticked), Some(list)) = (self.mode, &self.ticked, list) {
            let everything = !list.truncated
                && (self.empty || list.empty == 0)
                && list.values.iter().all(|(value, _)| ticked.contains(value));
            if everything {
                return ColumnFilter::default();
            }
        }
        self.build()
    }
}

/// Space kept between a moved panel and the viewport's edge, in px.
const VIEWPORT_MARGIN: f64 = 8.0;

/// How far to move a panel sideways from where it sits unmoved, so it stays
/// inside the viewport: left if it sticks out on the right, right if it sticks
/// out on the left. A panel wider than the viewport keeps its start visible.
fn fit_shift(left: f64, width: f64, viewport_left: f64, viewport_right: f64) -> f64 {
    let overflow = left + width - (viewport_right - VIEWPORT_MARGIN);
    let shift = if overflow > 0.0 { -overflow } else { 0.0 };
    let min_left = viewport_left + VIEWPORT_MARGIN;
    if left + shift < min_left {
        min_left - left
    } else {
        shift
    }
}

/// How an operand reads in an input.
fn edit_text(value: &Value) -> String {
    value.edit_text()
}

/// The input type for operands of `kind`.
const fn input_type(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::Date => "date",
        ValueKind::DateTime => "datetime-local",
        ValueKind::Text | ValueKind::Number | ValueKind::Bool => "text",
    }
}

/// A button that opens a filter menu for one column.
///
/// The menu is a non-modal `role="dialog"` with two halves: **conditions**,
/// one or two operator-and-value pairs joined by *and* or *or*, with the
/// operators that suit the column's [`ValueKind`]; and **values**, the
/// column's distinct values with their counts to tick, searchable, as in a
/// spreadsheet. *Apply* sets the column's
/// [typed filter](GridHandle::set_column_filter), *Clear* removes all of its
/// filters.
///
/// `Escape` or a click outside closes the menu and returns focus to the
/// button. The button reports `aria-expanded`, and `data-filtered` when the
/// column is filtered. Everything is unstyled: the root, button, panel and
/// the panel's parts carry `data-filter-*` attributes to style them by. The
/// click-outside layer is `position: fixed` and covers the page; give the panel
/// a higher `z-index` and position it. If the panel then sticks out of the
/// viewport at the side, as it does on a phone below a button near the edge,
/// it is moved back inside with an inline `translate`.
///
/// Renders nothing for a column that cannot be filtered.
#[component]
pub fn GridFilterMenu<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position among the visible columns, zero-based.
    column_index: usize,
    /// Most values the value list shows.
    #[props(default = DEFAULT_VALUE_LIMIT)]
    value_limit: usize,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let mut open = use_signal(|| false);
    let mut draft = use_signal(|| None::<Draft>);
    let mut list = use_signal(|| None::<Result<DistinctValues, String>>);
    let mut trigger = use_signal(|| None::<Rc<MountedData>>);
    let mut backdrop = use_signal(|| None::<Rc<MountedData>>);
    let mut panel = use_signal(|| None::<Rc<MountedData>>);
    // How far the panel is moved sideways to stay inside the viewport, in px.
    let mut shift = use_signal(|| 0.0_f64);
    let panel_id = use_hook(|| format!("dg-filter-menu-{}", next_id()));

    let columns = grid.visible_columns();
    let Some(column) = columns.get(column_index).cloned() else {
        return rsx! {};
    };
    if !column.spec().is_filterable() {
        return rsx! {};
    }

    let id: ColumnId = column.id().clone();
    let kind = grid.value_kind(&id).unwrap_or(ValueKind::Text);
    let locale = grid.locale();
    let locale = locale.read().clone();
    let label = locale.filter_menu(column.label());
    let filtered = grid.is_filtered(&id);
    let has_values = column.spec().value.is_some();

    let mut close = move || {
        open.set(false);
        if let Some(trigger) = trigger.peek().clone() {
            spawn(async move {
                let _ = trigger.set_focus(true).await;
            });
        }
    };

    let open_menu = {
        let id = id.clone();
        move || {
            let current = grid.column_filter(&id);
            draft.set(Some(Draft::from_filter(current.as_ref(), kind)));
            list.set(None);
            shift.set(0.0);
            open.set(true);
            if has_values {
                let load = grid.distinct_values(id.clone(), value_limit);
                spawn(async move {
                    list.set(Some(load.await));
                });
            }
        }
    };

    let apply = {
        let id = id.clone();
        move |_| {
            if let Some(draft) = draft.peek().as_ref() {
                let known = list
                    .peek()
                    .as_ref()
                    .and_then(|list| list.as_ref().ok().cloned());
                grid.set_column_filter(id.clone(), draft.build_against(known.as_ref()));
            }
            close();
        }
    };

    let clear = {
        let id = id.clone();
        move |_| {
            grid.clear_column_filters(&id);
            close();
        }
    };

    let is_open = open();
    let current = draft.read().clone();

    rsx! {
        div {
            "data-filter-menu": "",
            "data-open": "{is_open}",
            "data-filtered": "{filtered}",
            ..attributes,
            button {
                r#type: "button",
                "data-filter-trigger": "",
                aria_label: "{label}",
                aria_haspopup: "dialog",
                aria_expanded: "{is_open}",
                aria_controls: is_open.then(|| panel_id.clone()),
                onmounted: move |event| trigger.set(Some(event.data())),
                onclick: {
                    let mut open_menu = open_menu.clone();
                    move |_| {
                        if open() {
                            close();
                        } else {
                            open_menu();
                        }
                    }
                },
                span { aria_hidden: "true", "▾" }
            }
            if let (true, Some(current)) = (is_open, current) {
                div {
                    "data-filter-backdrop": "",
                    aria_hidden: "true",
                    style: "position: fixed; inset: 0;",
                    onmounted: move |event| backdrop.set(Some(event.data())),
                    onclick: move |_| close(),
                }
                div {
                    id: "{panel_id}",
                    role: "dialog",
                    aria_label: "{label}",
                    "data-filter-panel": "",
                    style: (shift() != 0.0).then(|| format!("translate: {}px 0;", shift())),
                    onmounted: move |event| panel.set(Some(event.data())),
                    // Measured whenever the panel changes size, as when the
                    // value list arrives. The backdrop covers the viewport.
                    onresize: move |_| {
                        spawn(async move {
                            let (Some(viewport), Some(own)) = (backdrop.peek().clone(), panel.peek().clone()) else {
                                return;
                            };
                            let (Ok(viewport), Ok(own)) = (viewport.get_client_rect().await, own.get_client_rect().await) else {
                                return;
                            };
                            let current = *shift.peek();
                            let next = fit_shift(own.min_x() - current, own.width(), viewport.min_x(), viewport.max_x());
                            if (next - current).abs() > 0.5 {
                                shift.set(next);
                            }
                        });
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.prevent_default();
                            event.stop_propagation();
                            close();
                        }
                    },
                    if has_values {
                        div { role: "group", "data-filter-modes": "",
                            for (mode , text) in [
                                (Mode::Conditions, locale.filter_by_condition.to_string()),
                                (Mode::Values, locale.filter_by_values.to_string()),
                            ]
                            {
                                button {
                                    key: "{text}",
                                    r#type: "button",
                                    aria_pressed: "{current.mode == mode}",
                                    onclick: move |_| {
                                        if let Some(draft) = draft.write().as_mut() {
                                            draft.mode = mode;
                                        }
                                    },
                                    "{text}"
                                }
                            }
                        }
                    }
                    {match current.mode {
                        Mode::Conditions => rsx! {
                            ConditionFields {
                                draft,
                                second: false,
                                kind,
                                locale: locale.clone(),
                            }
                            div { role: "radiogroup", aria_label: "{locale.filter_and} / {locale.filter_or}", "data-filter-join": "",
                                for (any , text) in [(false, locale.filter_and.to_string()), (true, locale.filter_or.to_string())] {
                                    label { key: "{text}",
                                        input {
                                            r#type: "radio",
                                            name: "{panel_id}-join",
                                            checked: current.any == any,
                                            onchange: move |_| {
                                                if let Some(draft) = draft.write().as_mut() {
                                                    draft.any = any;
                                                }
                                            },
                                        }
                                        "{text}"
                                    }
                                }
                            }
                            ConditionFields {
                                draft,
                                second: true,
                                kind,
                                locale: locale.clone(),
                            }
                        },
                        Mode::Values => rsx! {
                            ValueList {
                                draft,
                                list,
                                format: column.spec().format.clone(),
                                locale: locale.clone(),
                            }
                        },
                    }}
                    div { "data-filter-actions": "",
                        button { r#type: "button", "data-filter-apply": "", onclick: apply, "{locale.filter_apply}" }
                        button { r#type: "button", "data-filter-clear": "", onclick: clear, "{locale.filter_clear}" }
                    }
                }
            }
        }
    }
}

/// An operator choice and its operand inputs.
#[component]
fn ConditionFields(
    draft: Signal<Option<Draft>>,
    /// The second condition, whose operator may be "none".
    second: bool,
    kind: ValueKind,
    locale: datagrid_core::GridLocale,
) -> Element {
    let mut draft = draft;
    let Some(current) = draft.read().as_ref().map(|draft| {
        if second {
            draft.second.clone()
        } else {
            draft.first.clone()
        }
    }) else {
        return rsx! {};
    };

    let mut edit = move |change: &dyn Fn(&mut DraftCondition)| {
        if let Some(draft) = draft.write().as_mut() {
            change(if second {
                &mut draft.second
            } else {
                &mut draft.first
            });
        }
    };

    let operands = current.op.and_then(FilterOp::operands).unwrap_or(0);
    let selected = current.op.map_or("", FilterOp::as_str);

    rsx! {
        div { "data-filter-condition": if second { "second" } else { "first" },
            select {
                aria_label: "{locale.filter_operator}",
                // Focus lands here when the menu opens.
                onmounted: move |event| {
                    if !second {
                        spawn(async move {
                            let _ = event.data().set_focus(true).await;
                        });
                    }
                },
                onchange: move |event| {
                    let op = FilterOp::from_name(&event.value());
                    edit(&|condition| condition.op = op);
                },
                if second {
                    option { value: "", selected: selected.is_empty(), "{locale.filter_none}" }
                }
                for op in kind.operators() {
                    option {
                        key: "{op.as_str()}",
                        value: "{op.as_str()}",
                        selected: selected == op.as_str(),
                        "{locale.operator(*op)}"
                    }
                }
            }
            if operands >= 1 {
                OperandInput {
                    value: current.first.clone(),
                    kind,
                    label: locale.filter_value.to_string(),
                    locale: locale.clone(),
                    onchange: move |text: String| edit(&|condition| condition.first = text.clone()),
                }
            }
            if operands >= 2 {
                OperandInput {
                    value: current.second.clone(),
                    kind,
                    label: locale.filter_value_to.to_string(),
                    locale: locale.clone(),
                    onchange: move |text: String| edit(&|condition| condition.second = text.clone()),
                }
            }
        }
    }
}

/// One operand: a yes/no choice for booleans, otherwise an input of the type
/// that suits the kind.
#[component]
fn OperandInput(
    value: String,
    kind: ValueKind,
    label: String,
    locale: datagrid_core::GridLocale,
    onchange: EventHandler<String>,
) -> Element {
    if kind == ValueKind::Bool {
        return rsx! {
            select {
                aria_label: "{label}",
                onchange: move |event| onchange.call(event.value()),
                option { value: "", selected: value.is_empty(), "" }
                option { value: "true", selected: value == "true", "{locale.yes}" }
                option { value: "false", selected: value == "false", "{locale.no}" }
            }
        };
    }

    rsx! {
        input {
            r#type: input_type(kind),
            aria_label: "{label}",
            placeholder: "{label}",
            inputmode: (kind == ValueKind::Number).then_some("decimal"),
            value: "{value}",
            oninput: move |event| onchange.call(event.value()),
        }
    }
}

/// The distinct values with their counts, a search box and "select all".
#[component]
fn ValueList(
    draft: Signal<Option<Draft>>,
    list: Signal<Option<Result<DistinctValues, String>>>,
    format: datagrid_core::CellFormat,
    locale: datagrid_core::GridLocale,
) -> Element {
    let mut draft = draft;
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let loaded = list.read().clone();

    let body = match loaded {
        None => rsx! {
            p { role: "status", "data-filter-loading": "", "{locale.loading}" }
        },
        Some(Err(error)) => rsx! {
            p { role: "alert", "{error}" }
        },
        Some(Ok(values)) => {
            let needle = current.search.trim().to_lowercase();
            let entries: Vec<(Value, String, usize)> = values
                .values
                .iter()
                .map(|(value, count)| {
                    (
                        value.clone(),
                        locale.format(&value.as_cell(), &format),
                        *count,
                    )
                })
                .filter(|(_, text, _)| needle.is_empty() || text.to_lowercase().contains(&needle))
                .collect();
            let is_ticked = |value: &Value| {
                current
                    .ticked
                    .as_ref()
                    .is_none_or(|ticked| ticked.contains(value))
            };
            let all_ticked = entries.iter().all(|(value, ..)| is_ticked(value))
                && (values.empty == 0 || current.empty || !needle.is_empty());
            let visible: Vec<Value> = entries.iter().map(|(value, ..)| value.clone()).collect();
            let all_values: Vec<Value> = values
                .values
                .iter()
                .map(|(value, _)| value.clone())
                .collect();
            let show_empty = values.empty > 0 && needle.is_empty();

            rsx! {
                label { "data-filter-select-all": "",
                    input {
                        r#type: "checkbox",
                        checked: all_ticked,
                        onchange: move |event| {
                            let tick = event.checked();
                            if let Some(draft) = draft.write().as_mut() {
                                let ticked = draft
                                    .ticked
                                    .get_or_insert_with(|| all_values.iter().cloned().collect());
                                for value in &visible {
                                    if tick {
                                        ticked.insert(value.clone());
                                    } else {
                                        ticked.remove(value);
                                    }
                                }
                                if draft.search.trim().is_empty() {
                                    draft.empty = tick;
                                }
                            }
                        },
                    }
                    "{locale.filter_select_all}"
                }
                ul { "data-filter-values": "",
                    if show_empty {
                        li {
                            label {
                                input {
                                    r#type: "checkbox",
                                    checked: current.empty,
                                    onchange: move |event| {
                                        if let Some(draft) = draft.write().as_mut() {
                                            draft.empty = event.checked();
                                        }
                                    },
                                }
                                "{locale.filter_empty_value}"
                                span { "data-filter-count": "", " ({locale.integer(values.empty)})" }
                            }
                        }
                    }
                    for (index , (value , text , count)) in entries.into_iter().enumerate() {
                        li { key: "{index}-{text}",
                            label {
                                input {
                                    r#type: "checkbox",
                                    checked: is_ticked(&value),
                                    onchange: {
                                        let all_values = values.values.iter().map(|(value, _)| value.clone()).collect::<Vec<_>>();
                                        move |event: FormEvent| {
                                            let tick = event.checked();
                                            if let Some(draft) = draft.write().as_mut() {
                                                let ticked = draft
                                                    .ticked
                                                    .get_or_insert_with(|| all_values.iter().cloned().collect());
                                                if tick {
                                                    ticked.insert(value.clone());
                                                } else {
                                                    ticked.remove(&value);
                                                }
                                            }
                                        }
                                    },
                                }
                                "{text}"
                                span { "data-filter-count": "", " ({locale.integer(count)})" }
                            }
                        }
                    }
                }
                if values.truncated {
                    p { "data-filter-truncated": "", "{locale.filter_more_values(values.values.len())}" }
                }
            }
        }
    };

    rsx! {
        input {
            r#type: "search",
            aria_label: "{locale.filter_search_values}",
            placeholder: "{locale.filter_search_values}",
            value: "{current.search}",
            oninput: move |event| {
                if let Some(draft) = draft.write().as_mut() {
                    draft.search = event.value();
                }
            },
        }
        {body}
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn draft(kind: ValueKind) -> Draft {
        Draft::from_filter(None, kind)
    }

    #[test]
    fn a_panel_moves_back_inside_the_viewport() {
        // Fits: stays.
        assert_eq!(fit_shift(20.0, 300.0, 0.0, 400.0), 0.0);
        // Sticks out 100 px on the right of a 360 px phone: moves 108 left.
        assert_eq!(fit_shift(160.0, 300.0, 0.0, 360.0), -108.0);
        // Sticks out on the left, as a right-to-left panel can.
        assert_eq!(fit_shift(-50.0, 300.0, 0.0, 400.0), 58.0);
        // Wider than the viewport: its start stays visible.
        assert_eq!(fit_shift(100.0, 500.0, 0.0, 400.0), -92.0);
    }

    #[test]
    fn a_fresh_form_is_no_filter() {
        let form = draft(ValueKind::Number);
        assert_eq!(form.first.op, Some(FilterOp::Equals));
        assert!(form.build().is_empty());
    }

    #[test]
    fn conditions_need_their_operands() {
        let mut form = draft(ValueKind::Number);
        form.first = DraftCondition {
            op: Some(FilterOp::Between),
            first: "10".into(),
            second: String::new(),
        };
        assert!(form.build().is_empty(), "a range without its end");

        form.first.second = "20".into();
        form.second = DraftCondition {
            op: Some(FilterOp::IsEmpty),
            ..DraftCondition::default()
        };
        form.any = true;
        let filter = form.build();
        assert_eq!(
            filter,
            ColumnFilter::new(Condition::between("10", "20")).or(Condition::is_empty())
        );
    }

    #[test]
    fn or_with_one_condition_is_plain() {
        let mut form = draft(ValueKind::Text);
        form.first.first = "ber".into();
        form.any = true;
        assert_eq!(form.build(), ColumnFilter::new(Condition::contains("ber")));
    }

    #[test]
    fn a_filter_round_trips_through_the_form() {
        let filter = ColumnFilter::new(Condition::greater(5)).or(Condition::less(-5));
        let form = Draft::from_filter(Some(&filter), ValueKind::Number);
        assert_eq!(form.mode, Mode::Conditions);
        assert_eq!(
            form.build(),
            ColumnFilter::new(Condition::greater("5")).or(Condition::less("-5"))
        );

        let list = ColumnFilter::new(Condition::one_of([1, 2])).or(Condition::is_empty());
        let form = Draft::from_filter(Some(&list), ValueKind::Number);
        assert_eq!(form.mode, Mode::Values);
        assert!(form.empty);
        assert_eq!(form.build(), list);
    }

    #[test]
    fn ticking_every_value_is_no_filter() {
        let known = DistinctValues {
            values: vec![(Value::Int(1), 3), (Value::Int(2), 1)],
            empty: 2,
            truncated: false,
        };
        let mut form = draft(ValueKind::Number);
        form.mode = Mode::Values;
        form.ticked = Some([Value::Int(1), Value::Int(2)].into_iter().collect());

        assert!(form.build_against(Some(&known)).is_empty());

        form.empty = false;
        assert_eq!(
            form.build_against(Some(&known)),
            ColumnFilter::new(Condition::one_of([1, 2]))
        );

        form.ticked = None;
        assert_eq!(
            form.build_against(Some(&known)),
            ColumnFilter::new(Condition::is_not_empty())
        );
    }
}
