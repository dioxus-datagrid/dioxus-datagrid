//! What an export writes: the rows of a view as typed cells.
//!
//! A grid is shown in a window; an export is written whole. Both start from the
//! same place — the visible columns in the order the grid lays them out, the
//! rows after sorting, filtering and grouping, the values formatted by the
//! locale — so a file says what the screen said. The formats themselves (CSV,
//! Excel, PDF) are written from this table by `datagrid-export`, which keeps the
//! libraries they need out of this crate.
//!
//! Every cell keeps both the typed [`Value`] and the text the grid shows for it:
//! a spreadsheet wants the number and its format, a CSV for a program wants the
//! number alone, and a PDF wants the text.

use crate::{
    AggregateKind, AggregateValue, CellAlign, CellFormat, ColumnId, ColumnSpec, GridLocale,
    GridState, Pinned, Value, ValueKind, ViewRow, compute_view,
};

/// Which rows an export takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ExportScope {
    /// What the grid shows: the current page, and groups as open or closed as
    /// they are on screen.
    #[default]
    View,
    /// Every row that passes the filters, on every page, with every group open.
    Filtered,
}

/// One column of an export.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportColumn {
    /// The column this is.
    pub id: ColumnId,
    /// Its heading.
    pub label: String,
    /// The column groups it sits under, outermost first; empty without any.
    pub group_path: Vec<String>,
    /// How its values are formatted, for a writer that can carry the format
    /// along rather than the text.
    pub format: CellFormat,
    /// How its cells are aligned.
    pub align: CellAlign,
    /// What kind of value it holds, when the column says.
    pub kind: Option<ValueKind>,
}

impl ExportColumn {
    /// A column with a heading and nothing else to say: no groups, the plain
    /// format, aligned at the start. For a table built by hand rather than from
    /// a grid.
    #[must_use]
    pub fn new(id: impl Into<ColumnId>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            group_path: Vec::new(),
            format: CellFormat::Plain,
            align: CellAlign::Start,
            kind: None,
        }
    }
}

/// One cell: the value, and the text the grid shows for it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExportCell {
    /// The value, or `None` for an empty cell.
    pub value: Option<Value>,
    /// The value formatted by the locale and the column's format; empty for an
    /// empty cell.
    pub text: String,
}

/// One aggregate in an export: a column's sum, count or the like.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportAggregate {
    /// What it computed.
    pub kind: AggregateKind,
    /// What the locale calls it: "Sum".
    pub name: String,
    /// The result, or `None` when there was nothing to aggregate.
    pub value: Option<Value>,
    /// The result formatted in the column's format.
    pub text: String,
}

/// One row of an export.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ExportRow {
    /// A data row, one cell per column.
    Data(Vec<ExportCell>),
    /// The heading of a group.
    GroupHeader {
        /// How deep it sits, zero-based.
        level: usize,
        /// Its heading as the grid writes it: "City: Berlin".
        caption: String,
        /// How many rows it holds.
        count: usize,
        /// Whether its rows follow. A closed group carries its aggregates here,
        /// because it has no footer to carry them.
        expanded: bool,
        /// Per column, that column's aggregates; empty for an open group.
        aggregates: Vec<Vec<ExportAggregate>>,
    },
    /// The aggregates of a group, below its rows, or of the whole table.
    Aggregates {
        /// The level of the group they belong to, or `None` for the totals.
        level: Option<usize>,
        /// What to call the row where it has a name: the locale's "Totals" for
        /// the totals, nothing for a group's aggregates, which sit under the
        /// group they belong to.
        label: Option<String>,
        /// Per column, that column's aggregates.
        cells: Vec<Vec<ExportAggregate>>,
    },
}

/// A table ready to be written: the columns, and the rows under them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExportTable {
    /// The columns, in the order the grid lays them out.
    pub columns: Vec<ExportColumn>,
    /// The rows, in the order the grid shows them.
    pub rows: Vec<ExportRow>,
}

impl ExportTable {
    /// How many data rows there are, not counting headings and aggregates.
    #[must_use]
    pub fn data_rows(&self) -> usize {
        self.rows
            .iter()
            .filter(|row| matches!(row, ExportRow::Data(_)))
            .count()
    }
}

/// Builds the table an export writes.
///
/// `label` names a column, which the specs do not: a heading belongs to the
/// application's own column type. The columns are the visible ones, in layout
/// order, without the ones that are chrome rather than data: a checkbox, an
/// expander and a drag handle have nothing to write.
///
/// With [`ExportScope::Filtered`] the paging in `state` is ignored and every
/// group is open; with [`ExportScope::View`] the table is the page on screen.
/// Detail rows are never exported: they hold markup, not values.
///
/// A column with no [`value`](ColumnSpec::value) writes empty cells, as copying
/// does. A rendered cell is markup, and guessing at the text of markup would be
/// worse than leaving it out.
///
/// # Examples
///
/// ```
/// use datagrid_core::{
///     CellValue, ColumnSpec, ExportRow, ExportScope, GridLocale, GridState, export_table,
/// };
///
/// struct Person {
///     name: &'static str,
///     age: i64,
/// }
///
/// let rows = [Person { name: "Zoe", age: 31 }, Person { name: "Adi", age: 27 }];
/// let columns = [
///     ColumnSpec::new("name").value(|row: &Person| CellValue::Text(row.name)),
///     ColumnSpec::new("age").value(|row: &Person| CellValue::Int(row.age)),
/// ];
///
/// let table = export_table(
///     &rows,
///     &columns,
///     |id| id.as_str().to_uppercase(),
///     &GridState::new(),
///     &GridLocale::english(),
///     ExportScope::Filtered,
/// );
///
/// assert_eq!(table.columns[1].label, "AGE");
/// assert_eq!(table.data_rows(), 2);
/// let ExportRow::Data(cells) = &table.rows[0] else { unreachable!() };
/// assert_eq!(cells[0].text, "Zoe");
/// ```
#[must_use]
pub fn export_table<T>(
    rows: &[T],
    specs: &[ColumnSpec<T>],
    label: impl Fn(&ColumnId) -> String,
    state: &GridState,
    locale: &GridLocale,
    scope: ExportScope,
) -> ExportTable {
    let mut state = state.clone();
    if scope == ExportScope::Filtered {
        state.page = None;
        state.groups_collapsed = false;
        state.toggled_groups.clear();
    }
    let view = compute_view(rows, specs, &state);

    let shown: Vec<&ColumnSpec<T>> = crate::layout_order(specs, &state)
        .into_iter()
        .filter_map(|index| specs.get(index))
        .filter(|spec| !(spec.is_checkbox() || spec.is_expander() || spec.is_drag_handle()))
        .collect();

    let columns = shown
        .iter()
        .map(|spec| ExportColumn {
            id: spec.id.clone(),
            label: label(&spec.id),
            group_path: spec.group_path.clone(),
            format: spec.format.clone(),
            align: spec.effective_align(),
            kind: spec.kind,
        })
        .collect();

    // A column's aggregates, in the order the column lists them.
    let aggregates_of = |spec: &ColumnSpec<T>, list: &[AggregateValue]| -> Vec<ExportAggregate> {
        list.iter()
            .filter(|aggregate| aggregate.column == spec.id)
            .map(|aggregate| ExportAggregate {
                kind: aggregate.kind.clone(),
                name: locale.aggregate_name(&aggregate.kind).to_owned(),
                value: aggregate.value.clone(),
                text: locale.format_aggregate(aggregate, &spec.format),
            })
            .collect()
    };
    let per_column = |list: &[AggregateValue]| -> Vec<Vec<ExportAggregate>> {
        shown.iter().map(|spec| aggregates_of(spec, list)).collect()
    };

    let mut out = Vec::with_capacity(view.rows.len() + 1);
    for row in &view.rows {
        match *row {
            ViewRow::Data(index) => {
                let Some(row) = rows.get(index) else {
                    continue;
                };
                out.push(ExportRow::Data(
                    shown.iter().map(|spec| cell(spec, row, locale)).collect(),
                ));
            }
            ViewRow::GroupHeader(index) => {
                let Some(group) = view.groups.get(index) else {
                    continue;
                };
                let grouped = specs.iter().find(|spec| spec.id == group.column);
                let heading = label(&group.column);
                let value = match (&group.value, grouped) {
                    (Some(value), Some(spec)) => locale.format(&value.as_cell(), &spec.format),
                    (Some(value), None) => locale.format(&value.as_cell(), &CellFormat::Plain),
                    (None, _) => locale.filter_empty_value.to_string(),
                };
                out.push(ExportRow::GroupHeader {
                    level: group.level,
                    caption: locale.group_caption(&heading, &value),
                    count: group.count,
                    expanded: group.expanded,
                    aggregates: if group.expanded {
                        Vec::new()
                    } else {
                        per_column(&group.aggregates)
                    },
                });
            }
            ViewRow::GroupFooter(index) => {
                let Some(group) = view.groups.get(index) else {
                    continue;
                };
                out.push(ExportRow::Aggregates {
                    level: Some(group.level),
                    label: None,
                    cells: per_column(&group.aggregates),
                });
            }
            // Markup, not values; and any row kind added later is not known to
            // be one a file can hold.
            _ => {}
        }
    }
    // Totals over no rows are a row of empty cells, which says nothing.
    if view.filtered_len > 0 && !view.totals.is_empty() {
        out.push(ExportRow::Aggregates {
            level: None,
            label: Some(locale.totals.to_string()),
            cells: per_column(&view.totals),
        });
    }

    ExportTable { columns, rows: out }
}

/// One cell of a data row.
fn cell<T>(spec: &ColumnSpec<T>, row: &T, locale: &GridLocale) -> ExportCell {
    let Some(read) = &spec.value else {
        return ExportCell::default();
    };
    let value = read(row);
    ExportCell {
        text: locale.format(&value, &spec.format),
        value: Value::from_cell(&value),
    }
}

/// The columns of a grid in the order it lays them out, as indices into
/// `specs`: the visible ones, in the state's column order, with the pinned ones
/// gathered at their edges.
///
/// The one place that knows this order, so that the grid on screen and an
/// export of it cannot disagree about it.
#[must_use]
pub fn layout_order<'a, T: 'a>(
    specs: impl IntoIterator<Item = &'a ColumnSpec<T>>,
    state: &GridState,
) -> Vec<usize> {
    let specs: Vec<&ColumnSpec<T>> = specs.into_iter().collect();
    let declared: Vec<ColumnId> = specs.iter().map(|spec| spec.id.clone()).collect();

    let mut shown: Vec<usize> = state
        .ordered_columns(&declared)
        .iter()
        .filter_map(|id| specs.iter().position(|spec| &spec.id == id))
        .filter(|index| {
            specs
                .get(*index)
                .is_some_and(|spec| spec.is_visible(&state.hidden_columns))
        })
        .collect();

    // Pinned columns are laid out as a block at their edge, so that one can
    // never end up between two scrolling columns. The order within each block is
    // the column order, which a stable sort keeps.
    shown.sort_by_key(
        |index| match specs.get(*index).map(|spec| spec.effective_pin(state)) {
            Some(Pinned::Start) => 0,
            Some(Pinned::End) => 2,
            _ => 1,
        },
    );
    shown
}
