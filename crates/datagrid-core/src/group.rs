//! Grouping rows by column values, and aggregating a column over rows.

use crate::{CellValue, ColumnId, ColumnSpec, GridQuery, Page, TextCollation, Value, ViewRow};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// Computes a custom aggregate from the rows it covers.
pub type AggregateFn<T> = Rc<dyn Fn(&[&T]) -> Option<Value>>;

/// What an aggregate computes, without the closure a custom one carries: what
/// a result is labelled with, and what a server is asked for.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AggregateKind {
    /// The sum of the numbers.
    Sum,
    /// The mean of the numbers.
    Average,
    /// The smallest value, in the column's sort order.
    Min,
    /// The largest value, in the column's sort order.
    Max,
    /// How many rows have a value in the column.
    Count,
    /// A function of the application's, under this label.
    Custom(String),
}

impl AggregateKind {
    /// A short name for styling and tests: `sum`, `average`, `min`, `max`,
    /// `count`, or `custom` for every custom aggregate.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Sum => "sum",
            Self::Average => "average",
            Self::Min => "min",
            Self::Max => "max",
            Self::Count => "count",
            Self::Custom(_) => "custom",
        }
    }
}

/// An aggregate of one column: what a footer shows below it.
///
/// Every function skips empty cells. [`Sum`](Aggregate::Sum) and
/// [`Average`](Aggregate::Average) also skip anything that is not a number;
/// [`Min`](Aggregate::Min) and [`Max`](Aggregate::Max) work on any value, in
/// the order the column sorts by.
pub enum Aggregate<T> {
    /// The sum: an integer while every number is one and it fits, a float
    /// otherwise.
    Sum,
    /// The mean, as a float.
    Average,
    /// The smallest value.
    Min,
    /// The largest value.
    Max,
    /// How many rows have a value.
    Count,
    /// Anything else, computed from the rows themselves, such as a weighted
    /// average.
    Custom {
        /// What the result is labelled with.
        label: String,
        /// Computes the result.
        compute: AggregateFn<T>,
    },
}

impl<T> Aggregate<T> {
    /// A custom aggregate, labelled `label`.
    ///
    /// ```
    /// # use datagrid_core::{Aggregate, Value};
    /// struct Line { quantity: u32, price: f64 }
    ///
    /// let revenue = Aggregate::custom("Revenue", |lines: &[&Line]| {
    ///     let total: f64 = lines.iter().map(|line| f64::from(line.quantity) * line.price).sum();
    ///     Some(Value::Float(total))
    /// });
    /// ```
    pub fn custom(
        label: impl Into<String>,
        compute: impl Fn(&[&T]) -> Option<Value> + 'static,
    ) -> Self {
        Self::Custom {
            label: label.into(),
            compute: Rc::new(compute),
        }
    }

    /// The built-in aggregate of `kind`; `None` for a custom one, whose
    /// function only the application knows.
    #[must_use]
    pub const fn from_kind(kind: &AggregateKind) -> Option<Self> {
        Some(match kind {
            AggregateKind::Sum => Self::Sum,
            AggregateKind::Average => Self::Average,
            AggregateKind::Min => Self::Min,
            AggregateKind::Max => Self::Max,
            AggregateKind::Count => Self::Count,
            AggregateKind::Custom(_) => return None,
        })
    }

    /// Gives `columns` the aggregates `query` asks for instead of their own,
    /// for a server that answers with [`compute_view`](crate::compute_view).
    /// Custom aggregates are left out.
    pub fn apply_query(columns: &mut [ColumnSpec<T>], query: &GridQuery) {
        for column in columns {
            column.aggregates = query
                .aggregates
                .iter()
                .filter(|(id, _)| *id == column.id)
                .filter_map(|(_, kind)| Self::from_kind(kind))
                .collect();
        }
    }

    /// What this aggregate computes.
    #[must_use]
    pub fn kind(&self) -> AggregateKind {
        match self {
            Self::Sum => AggregateKind::Sum,
            Self::Average => AggregateKind::Average,
            Self::Min => AggregateKind::Min,
            Self::Max => AggregateKind::Max,
            Self::Count => AggregateKind::Count,
            Self::Custom { label, .. } => AggregateKind::Custom(label.clone()),
        }
    }

    /// Computes this aggregate of `column` over `rows`.
    pub fn compute(&self, column: &ColumnSpec<T>, rows: &[&T]) -> Option<Value> {
        let values = || {
            rows.iter()
                .map(|row| column.read(row))
                .filter(|value| !value.is_none())
        };
        match self {
            Self::Sum => sum(values()).map(|(sum, _)| sum),
            Self::Average => {
                let (sum, count) = sum(values())?;
                #[allow(clippy::cast_precision_loss)]
                let mean = match sum {
                    Value::Int(sum) => sum as f64 / count as f64,
                    Value::Float(sum) => sum / count as f64,
                    _ => return None,
                };
                Some(Value::Float(mean))
            }
            Self::Min => extreme(values(), column.collation, false),
            Self::Max => extreme(values(), column.collation, true),
            Self::Count => Some(Value::Int(
                i64::try_from(values().count()).unwrap_or(i64::MAX),
            )),
            Self::Custom { compute, .. } => compute(rows),
        }
    }
}

/// The sum of the numbers among `values`, and how many there were. `None`
/// when there were none.
fn sum<'a>(values: impl Iterator<Item = CellValue<'a>>) -> Option<(Value, usize)> {
    let mut int: Option<i64> = Some(0);
    let mut float = 0.0_f64;
    let mut count = 0_usize;
    for value in values {
        match value {
            CellValue::Int(value) => {
                int = int.and_then(|sum| sum.checked_add(value));
                #[allow(clippy::cast_precision_loss)]
                {
                    float += value as f64;
                }
            }
            CellValue::Float(value) => {
                int = None;
                float += value;
            }
            _ => continue,
        }
        count += 1;
    }
    if count == 0 {
        return None;
    }
    Some((int.map_or(Value::Float(float), Value::Int), count))
}

/// The smallest or largest of `values`.
fn extreme<'a>(
    values: impl Iterator<Item = CellValue<'a>>,
    collation: TextCollation,
    largest: bool,
) -> Option<Value> {
    let found = values.reduce(|best, value| {
        let ordering = value.cmp_with(&best, collation);
        let better = if largest {
            ordering.is_gt()
        } else {
            ordering.is_lt()
        };
        if better { value } else { best }
    })?;
    Value::from_cell(&found)
}

impl<T> Clone for Aggregate<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Sum => Self::Sum,
            Self::Average => Self::Average,
            Self::Min => Self::Min,
            Self::Max => Self::Max,
            Self::Count => Self::Count,
            Self::Custom { label, compute } => Self::Custom {
                label: label.clone(),
                compute: Rc::clone(compute),
            },
        }
    }
}

impl<T> fmt::Debug for Aggregate<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind().fmt(f)
    }
}

/// Custom aggregates are equal when they share their closure, as columns
/// compare.
impl<T> PartialEq for Aggregate<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Custom { label, compute },
                Self::Custom {
                    label: other_label,
                    compute: other_compute,
                },
            ) => label == other_label && Rc::ptr_eq(compute, other_compute),
            _ => self.kind() == other.kind(),
        }
    }
}

/// One aggregate's result.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AggregateValue {
    /// The column it aggregates.
    pub column: ColumnId,
    /// What it computed.
    pub kind: AggregateKind,
    /// The result; `None` when there was nothing to aggregate.
    pub value: Option<Value>,
}

/// Computes every column's aggregates over `rows`, in column order.
pub(crate) fn aggregate_all<T>(columns: &[ColumnSpec<T>], rows: &[&T]) -> Vec<AggregateValue> {
    columns
        .iter()
        .flat_map(|column| {
            column
                .aggregates
                .iter()
                .map(move |aggregate| AggregateValue {
                    column: column.id.clone(),
                    kind: aggregate.kind(),
                    value: aggregate.compute(column, rows),
                })
        })
        .collect()
}

/// Where a group sits: the value of each grouped column, from the outermost
/// group down to this one. Identifies a group across recomputations, which is
/// what remembering whether it is expanded needs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GroupKey(pub Vec<Option<Value>>);

impl GroupKey {
    /// How deep the group is: `1` for a group of the first grouped column.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.0.len()
    }

    /// The key of a group inside this one, holding `value`.
    #[must_use]
    pub fn child(&self, value: Option<Value>) -> Self {
        let mut path = self.0.clone();
        path.push(value);
        Self(path)
    }

    /// The key of the group this one is inside; `None` at the top.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        let (_, parent) = self.0.split_last()?;
        (!parent.is_empty()).then(|| Self(parent.to_vec()))
    }
}

/// A group of rows sharing a value in a grouped column.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Group {
    /// Which group this is.
    pub key: GroupKey,
    /// The grouped column.
    pub column: ColumnId,
    /// The value its rows share; `None` for the rows without one.
    pub value: Option<Value>,
    /// How deep it sits, zero-based: `0` for the outermost groups.
    pub level: usize,
    /// How many data rows it holds, across every page.
    pub count: usize,
    /// Whether its rows are shown.
    pub expanded: bool,
    /// Its position among the groups of the same parent, one-based: what
    /// `aria-posinset` says.
    pub position: usize,
    /// How many groups share its parent: what `aria-setsize` says.
    pub siblings: usize,
    /// The columns' aggregates over its rows.
    pub aggregates: Vec<AggregateValue>,
}

impl Group {
    /// The aggregate of `kind` over `column` in this group, if there is one.
    #[must_use]
    pub fn aggregate(&self, column: &ColumnId, kind: &AggregateKind) -> Option<&AggregateValue> {
        find_aggregate(&self.aggregates, column, kind)
    }
}

/// The aggregate of `kind` over `column` among `aggregates`.
#[must_use]
pub fn find_aggregate<'a>(
    aggregates: &'a [AggregateValue],
    column: &ColumnId,
    kind: &AggregateKind,
) -> Option<&'a AggregateValue> {
    aggregates
        .iter()
        .find(|aggregate| &aggregate.column == column && &aggregate.kind == kind)
}

/// One group as a server counts it: its key, how many rows it holds, and its
/// aggregates. A server gets these from one `GROUP BY` per grouped column; see
/// [`GroupedPage::plan`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GroupSummary {
    /// The group's values, outermost first.
    pub key: GroupKey,
    /// How many rows match the query in this group.
    pub count: usize,
    /// The aggregates the query asked for, over this group's rows.
    pub aggregates: Vec<AggregateValue>,
}

/// One part of a grouped page, in display order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PagePart {
    /// A group's header: the index into [`GroupedPage::groups`].
    Header(usize),
    /// A group's footer: the index into [`GroupedPage::groups`].
    Footer(usize),
    /// Rows of one innermost group, which the server has to fetch: `limit`
    /// rows starting `offset` rows into the group, in the query's sort.
    Rows {
        /// The group the rows belong to.
        key: GroupKey,
        /// How many of its rows come before these.
        offset: usize,
        /// How many rows to fetch.
        limit: usize,
    },
}

/// Which group rows fall on a page of a grouped query, and which data rows a
/// server still has to fetch for it.
///
/// A server that cannot hold every row in memory groups in two steps. First it
/// counts: one [`GroupSummary`] per group, from a `GROUP BY` per level. Then
/// [`plan`](GroupedPage::plan) walks those groups as the grid would, headers,
/// rows and footers, and cuts out the requested page. For each
/// [`PagePart::Rows`] the server fetches rows of that group with `LIMIT` and
/// `OFFSET`, and [`into_page`](GroupedPage::into_page) puts it all together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GroupedPage {
    /// The page, in display order.
    pub parts: Vec<PagePart>,
    /// The groups with a header or footer on the page.
    pub groups: Vec<Group>,
    /// How many rows of every kind the query spans across every page.
    pub row_count: usize,
    /// How many data rows match, across every group.
    pub total: usize,
}

impl GroupedPage {
    /// Lays out the requested page of a grouped query.
    ///
    /// `levels` holds the groups of each grouped column in turn, each level in
    /// display order — the order of the group values in the query's sort,
    /// which an `ORDER BY` over the grouped columns gives. Groups whose parent
    /// is missing are ignored. Group footers are laid out when the query asks
    /// for aggregates, as they are locally. A page past the last one yields
    /// the last one.
    #[must_use]
    pub fn plan(levels: &[Vec<GroupSummary>], query: &GridQuery) -> Self {
        let children: Vec<HashMap<GroupKey, Vec<&GroupSummary>>> = levels
            .iter()
            .map(|level| {
                let mut by_parent: HashMap<GroupKey, Vec<&GroupSummary>> = HashMap::new();
                for summary in level {
                    let parent = summary.key.parent().unwrap_or_default();
                    by_parent.entry(parent).or_default().push(summary);
                }
                by_parent
            })
            .collect();

        let size = query.page_size.max(1);
        let walk = |page_index: usize| {
            let start = page_index.saturating_mul(size);
            let mut walk = Walk {
                query,
                children: &children,
                footers: !query.aggregates.is_empty(),
                start,
                end: start.saturating_add(size),
                position: 0,
                page: Self::default(),
                renumbered: HashMap::new(),
            };
            walk.level(&GroupKey::default(), 0);
            walk.page.row_count = walk.position;
            walk.page
        };

        let mut page = walk(query.page);
        let last = page.row_count.div_ceil(size).saturating_sub(1);
        if query.page > last {
            page = walk(last);
        }
        page.total = levels
            .first()
            .map_or(0, |level| level.iter().map(|group| group.count).sum());
        page
    }

    /// The row ranges to fetch, in order: each [`PagePart::Rows`] as its
    /// group, offset and limit.
    pub fn fetches(&self) -> impl Iterator<Item = (&GroupKey, usize, usize)> {
        self.parts.iter().filter_map(|part| match part {
            PagePart::Rows { key, offset, limit } => Some((key, *offset, *limit)),
            PagePart::Header(_) | PagePart::Footer(_) => None,
        })
    }

    /// The finished page: `fetched` holds the rows of each of
    /// [`fetches`](GroupedPage::fetches), in the same order, and `totals` the
    /// aggregates over every matching row.
    #[must_use]
    pub fn into_page<T>(self, fetched: Vec<Vec<T>>, totals: Vec<AggregateValue>) -> Page<T> {
        let mut rows = Vec::new();
        let mut layout = Vec::with_capacity(self.parts.len());
        let mut fetched = fetched.into_iter();
        for part in self.parts {
            match part {
                PagePart::Header(group) => layout.push(ViewRow::GroupHeader(group)),
                PagePart::Footer(group) => layout.push(ViewRow::GroupFooter(group)),
                PagePart::Rows { .. } => {
                    for row in fetched.next().unwrap_or_default() {
                        layout.push(ViewRow::Data(rows.len()));
                        rows.push(row);
                    }
                }
            }
        }
        Page {
            rows,
            total: self.total,
            layout,
            groups: self.groups,
            row_count: Some(self.row_count),
            totals,
        }
    }
}

/// The walk behind [`GroupedPage::plan`].
struct Walk<'a> {
    query: &'a GridQuery,
    children: &'a [HashMap<GroupKey, Vec<&'a GroupSummary>>],
    footers: bool,
    /// The page, as positions among every row.
    start: usize,
    end: usize,
    /// How many rows come before the one being placed.
    position: usize,
    page: GroupedPage,
    /// Where each group placed on the page went in `page.groups`.
    renumbered: HashMap<GroupKey, usize>,
}

impl Walk<'_> {
    fn level(&mut self, parent: &GroupKey, level: usize) {
        let children = self.children;
        let Some(groups) = children.get(level).and_then(|by| by.get(parent)) else {
            return;
        };
        let siblings = groups.len();
        for (position, summary) in groups.iter().enumerate() {
            let expanded = self.query.is_group_expanded(&summary.key);
            let group = Group {
                key: summary.key.clone(),
                column: self
                    .query
                    .group_by
                    .get(level)
                    .cloned()
                    .unwrap_or_else(|| ColumnId::new("")),
                value: summary.key.0.last().cloned().flatten(),
                level,
                count: summary.count,
                expanded,
                position: position + 1,
                siblings,
                aggregates: summary.aggregates.clone(),
            };
            self.place(&group, PagePart::Header);
            if !expanded {
                continue;
            }
            if level + 1 < children.len() {
                self.level(&summary.key, level + 1);
            } else {
                self.rows(&summary.key, summary.count);
            }
            if self.footers {
                self.place(&group, PagePart::Footer);
            }
        }
    }

    /// Places a header or footer of `group`, if it is on the page.
    fn place(&mut self, group: &Group, part: fn(usize) -> PagePart) {
        if (self.start..self.end).contains(&self.position) {
            let index = match self.renumbered.get(&group.key) {
                Some(&index) => index,
                None => {
                    self.page.groups.push(group.clone());
                    let index = self.page.groups.len() - 1;
                    self.renumbered.insert(group.key.clone(), index);
                    index
                }
            };
            self.page.parts.push(part(index));
        }
        self.position += 1;
    }

    /// Places the part of a group's `count` rows that is on the page.
    fn rows(&mut self, key: &GroupKey, count: usize) {
        let first = self.position.max(self.start);
        let last = self.position.saturating_add(count).min(self.end);
        if first < last {
            self.page.parts.push(PagePart::Rows {
                key: key.clone(),
                offset: first - self.position,
                limit: last - first,
            });
        }
        self.position += count;
    }
}
