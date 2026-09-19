//! Primitives for grouped rows, aggregates and the group panel.

use crate::GridHandle;
use crate::primitives::{use_focus_pull, use_focus_pull_where};
use datagrid_core::{AggregateValue, CellFocus, ColumnId, GridRow as GridRowKey};
use dioxus::prelude::*;

/// Where a row of aggregates takes them from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateSource {
    /// Every filtered row: the grid's footer.
    Totals,
    /// One group, by its index into [`View::groups`](datagrid_core::View::groups).
    Group(usize),
}

impl AggregateSource {
    /// The aggregates of `column` from this source, in the order the column
    /// declares them.
    fn read<T: GridRowKey>(self, grid: &GridHandle<T>, column: &ColumnId) -> Vec<AggregateValue> {
        let view = grid.view();
        let view = view.read();
        let all = match self {
            Self::Totals => &view.totals,
            Self::Group(group) => match view.groups.get(group) {
                Some(group) => &group.aggregates,
                None => return Vec::new(),
            },
        };
        all.iter()
            .filter(|aggregate| &aggregate.column == column)
            .cloned()
            .collect()
    }
}

/// The row that heads a group: one cell across every column, with the
/// grouped column, the group's value and how many rows it holds. A
/// collapsed group also shows its aggregates here, since its footer is
/// hidden with its rows.
///
/// [`GridRow`](crate::primitives::GridRow) renders this for a group's header
/// by itself; use it directly only in a body of your own.
///
/// Carries `aria-level`, `aria-expanded`, `aria-posinset` and `aria-setsize`
/// as the treegrid pattern asks. Clicking it, or `Enter`, `Space`,
/// `ArrowRight` and `ArrowLeft` on it, expand and collapse the group.
///
/// Renders `data-group-row` and `data-expanded` on the row, `data-group-cell`
/// on the cell, and inside it `data-group-toggle` (empty, for an icon),
/// `data-group-caption`, `data-group-count` and `data-group-aggregate`. The
/// cell sets `--dg-group-level` to the group's depth, starting at `0`, for
/// indenting.
#[component]
pub fn GridGroupRow<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position within the current page, zero-based.
    row_index: usize,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let focus_row = row_index + 1;
    // The one cell stands for every column of its row.
    let onmounted = use_focus_pull_where(grid, move |focus| focus.row == focus_row);
    let Some(group) = grid.group_at(row_index) else {
        return rsx! {};
    };
    let aria_row_index = grid.view().read().row_offset + row_index + 2;
    let column_count = grid.visible_column_count();
    let focused = grid.focus().row == focus_row;

    let locale = grid.locale();
    let locale = locale.read();
    let columns = grid.columns();
    let columns = columns.read();
    let spec = columns
        .iter()
        .find(|column| column.id() == &group.column)
        .map(|column| column.spec());
    let label = columns
        .iter()
        .find(|column| column.id() == &group.column)
        .map_or(group.column.as_str(), |column| column.label());
    let value = match (&group.value, spec) {
        (Some(value), Some(spec)) => locale.format(&value.as_cell(), &spec.format),
        (Some(value), None) => locale.format(&value.as_cell(), &Default::default()),
        (None, _) => locale.filter_empty_value.to_string(),
    };
    let caption = locale.group_caption(label, &value);
    let count = locale.row_count(group.count);
    // Of the visible columns only, as the footers show them.
    let visible = grid.visible_columns();
    let aggregates: Vec<String> = if group.expanded {
        Vec::new()
    } else {
        group
            .aggregates
            .iter()
            .filter(|aggregate| aggregate.value.is_some())
            .filter_map(|aggregate| {
                let column = visible
                    .iter()
                    .find(|column| column.id() == &aggregate.column)?;
                let value = locale.format_aggregate(aggregate, &column.spec().format);
                Some(locale.aggregate_of(&aggregate.kind, column.label(), &value))
            })
            .collect()
    };

    let key = group.key.clone();
    let onclick = move |_| {
        let col = grid.focus().col;
        grid.set_focus(CellFocus::new(focus_row, col));
        grid.toggle_group(&key);
    };

    rsx! {
        div {
            role: "row",
            aria_rowindex: "{aria_row_index}",
            aria_level: "{group.level + 1}",
            aria_expanded: if group.expanded { "true" } else { "false" },
            aria_posinset: "{group.position}",
            aria_setsize: "{group.siblings}",
            "data-group-row": "",
            "data-group-level": "{group.level}",
            "data-expanded": "{group.expanded}",
            ..attributes,
            div {
                role: "gridcell",
                aria_colindex: "1",
                aria_colspan: "{column_count}",
                tabindex: if focused { "0" } else { "-1" },
                style: "--dg-group-level: {group.level};",
                "data-group-cell": "",
                onmounted,
                onclick,
                span { "data-group-toggle": "", aria_hidden: "true" }
                span { "data-group-caption": "", "{caption}" }
                span { "data-group-count": "", "{count}" }
                for aggregate in aggregates {
                    span { "data-group-aggregate": "", "{aggregate}" }
                }
            }
        }
    }
}

/// The row below an expanded group's rows: its aggregates, each under its
/// column.
///
/// [`GridRow`](crate::primitives::GridRow) renders this for a group's footer
/// by itself. Renders `data-group-footer` and `data-group-level`.
#[component]
pub fn GridGroupFooterRow<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position within the current page, zero-based.
    row_index: usize,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let (aria_row_index, group) = {
        let view = grid.view();
        let view = view.read();
        let group = view
            .row(row_index)
            .and_then(|row| row.group_index())
            .and_then(|index| Some((index, view.groups.get(index)?.level)));
        (view.row_offset + row_index + 2, group)
    };
    let Some((group, level)) = group else {
        return rsx! {};
    };
    let columns = grid.visible_column_count();

    rsx! {
        div {
            role: "row",
            aria_rowindex: "{aria_row_index}",
            // Inside its group, alongside the group's rows.
            aria_level: "{level + 2}",
            "data-group-footer": "",
            "data-group-level": "{level}",
            ..attributes,
            for column_index in 0..columns {
                GridAggregateCell {
                    key: "{column_index}",
                    grid,
                    source: AggregateSource::Group(group),
                    focus_row: row_index + 1,
                    column_index,
                }
            }
        }
    }
}

/// One cell of aggregates: every aggregate its column has, from `source`.
///
/// Each aggregate is a `data-aggregate` element holding a
/// `data-aggregate-name` and a `data-aggregate-value`; put a colon between
/// them in CSS if you want one.
#[component]
pub fn GridAggregateCell<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Which aggregates to show.
    source: AggregateSource,
    /// The row this cell is in, as [`CellFocus::row`] counts.
    focus_row: usize,
    /// Position among the visible columns, zero-based.
    column_index: usize,
    /// Shown when the column has no aggregate, such as "Totals" in the first
    /// cell of a footer.
    #[props(default)]
    label: Option<String>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let at = CellFocus::new(focus_row, column_index);
    let onmounted = use_focus_pull(grid, at);
    let columns = grid.visible_columns();
    let Some(column) = columns.get(column_index) else {
        return rsx! {};
    };
    let focused = grid.focus() == at;
    let locale = grid.locale();
    let locale = locale.read();
    let aggregates: Vec<(&str, String, String)> = source
        .read(&grid, column.id())
        .iter()
        .map(|aggregate| {
            (
                aggregate.kind.as_str(),
                locale.aggregate_name(&aggregate.kind).to_owned(),
                locale.format_aggregate(aggregate, &column.spec().format),
            )
        })
        .collect();
    let label = label.filter(|_| aggregates.is_empty());
    // All of it, for when a fixed row height cuts some of it off.
    let title = (!aggregates.is_empty()).then(|| {
        aggregates
            .iter()
            .map(|(_, name, value)| format!("{name} {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    });

    rsx! {
        div {
            role: "gridcell",
            aria_colindex: "{column_index + 1}",
            tabindex: if focused { "0" } else { "-1" },
            title,
            "data-align": column.spec().effective_align().as_str(),
            onmounted,
            onclick: move |_| grid.set_focus(at),
            ..attributes,
            if let Some(label) = label {
                span { "data-aggregate-label": "", "{label}" }
            }
            for (kind , name , value) in aggregates {
                span { "data-aggregate": "{kind}",
                    span { "data-aggregate-name": "", "{name}" }
                    " "
                    span { "data-aggregate-value": "", "{value}" }
                }
            }
        }
    }
}

/// A footer row with every column's aggregates over all filtered rows,
/// across every page. Renders nothing when no column has an aggregate.
///
/// Put it inside [`GridRoot`](crate::primitives::GridRoot), after the body.
/// The keyboard reaches it after the last row; `aria-rowindex` makes it the
/// last row of the grid. Renders `data-footer` on its row group.
///
/// Made sticky at the bottom of a virtualized grid, it reports its height so
/// that rows scrolled into view by the keyboard do not end up beneath it.
#[component]
pub fn GridFooter<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let shown = grid.has_aggregates();
    // Compared before writing, so registering in render cannot loop.
    grid.set_footer(shown);
    use_drop(move || {
        grid.set_footer(false);
        grid.record_footer_height(0.0);
    });
    if !shown {
        grid.record_footer_height(0.0);
        return rsx! {};
    }

    let (focus_row, aria_row_index, grouped) = {
        let view = grid.view();
        let view = view.read();
        (view.len() + 1, view.row_count + 2, view.group_levels > 0)
    };
    let columns = grid.visible_column_count();
    let totals = grid.locale().read().totals.to_string();

    rsx! {
        div {
            role: "rowgroup",
            "data-footer": "",
            // A footer that sticks to the bottom hides the rows beneath it,
            // which a virtualized body has to know.
            onresize: move |event| {
                if let Ok(size) = event.data().get_border_box_size() {
                    grid.record_footer_height(size.height);
                }
            },
            ..attributes,
            div {
                role: "row",
                aria_rowindex: "{aria_row_index}",
                aria_level: grouped.then_some("1"),
                for column_index in 0..columns {
                    GridAggregateCell {
                        key: "{column_index}",
                        grid,
                        source: AggregateSource::Totals,
                        focus_row,
                        column_index,
                        label: (column_index == 0).then(|| totals.clone()),
                    }
                }
            }
        }
    }
}

/// Where rows are grouped: the grouped columns in order, a list to group by
/// another column, and buttons to expand or collapse every group. While it is
/// mounted, column headers can be dragged onto it.
///
/// Everything dragging does is also possible with the keyboard: the list
/// adds a column, each grouped column has a button to move it one level out
/// and one to remove it.
///
/// Renders `data-group-panel`, `data-drop-target` while a header is dragged
/// and `data-drag-over` while it is over the panel; inside, `data-group-hint`,
/// `data-group-chip` with `data-group-move` and `data-group-remove`,
/// `data-group-add`, `data-group-expand` and `data-group-collapse`.
#[component]
pub fn GridGroupPanel<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    grid.set_group_panel(true);
    use_drop(move || grid.set_group_panel(false));
    let mut over = use_signal(|| false);

    let locale = grid.locale();
    let locale = locale.read().clone();
    let grouped: Vec<(ColumnId, String)> = grid
        .group_by()
        .into_iter()
        .filter_map(|id| {
            let label = grid.column_label(&id)?;
            Some((id, label))
        })
        .collect();
    let available = grid.groupable_columns();
    let dragging = grid.dragged_column().is_some();

    rsx! {
        div {
            role: "group",
            aria_label: "{locale.group_panel}",
            "data-group-panel": "",
            "data-drop-target": dragging.then_some("true"),
            "data-drag-over": (dragging && over()).then_some("true"),
            ondragover: move |event: DragEvent| {
                // Accepting the drop is what preventing the default does here.
                if grid.dragged_column().is_some() {
                    event.prevent_default();
                    if !*over.peek() {
                        over.set(true);
                    }
                }
            },
            ondragleave: move |_| over.set(false),
            ondrop: move |event: DragEvent| {
                event.prevent_default();
                over.set(false);
                grid.drop_dragged_column(None);
            },
            ..attributes,
            if grouped.is_empty() {
                span { "data-group-hint": "", "{locale.group_drop_hint}" }
            } else {
                ol { "data-group-chips": "",
                    for (position , (id , label)) in grouped.into_iter().enumerate() {
                        li { key: "{id}", "data-group-chip": "",
                            span { "{label}" }
                            if position > 0 {
                                button {
                                    r#type: "button",
                                    "data-group-move": "",
                                    aria_label: "{locale.group_move_out(&label)}",
                                    onclick: {
                                        let id = id.clone();
                                        move |_| grid.group_by_column(id.clone(), Some(position - 1))
                                    },
                                    "‹"
                                }
                            }
                            button {
                                r#type: "button",
                                "data-group-remove": "",
                                aria_label: "{locale.group_remove(&label)}",
                                onclick: move |_| grid.ungroup_column(&id),
                                "×"
                            }
                        }
                    }
                }
            }
            if !available.is_empty() {
                select {
                    "data-group-add": "",
                    aria_label: "{locale.group_by}",
                    value: "",
                    onchange: move |event| {
                        let column = event.value();
                        if !column.is_empty() {
                            grid.group_by_column(ColumnId::new(column), None);
                        }
                    },
                    option { value: "", "{locale.group_by}" }
                    for (id , label) in available {
                        option { key: "{id}", value: "{id}", "{label}" }
                    }
                }
            }
            if grid.is_grouped() {
                button {
                    r#type: "button",
                    "data-group-expand": "",
                    onclick: move |_| grid.set_all_groups_expanded(true),
                    "{locale.group_expand_all}"
                }
                button {
                    r#type: "button",
                    "data-group-collapse": "",
                    onclick: move |_| grid.set_all_groups_expanded(false),
                    "{locale.group_collapse_all}"
                }
            }
        }
    }
}
