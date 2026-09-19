//! Grouping rows by column values, and aggregating a column over rows.

use crate::{CellValue, ColumnId, ColumnSpec, TextCollation, Value};
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
