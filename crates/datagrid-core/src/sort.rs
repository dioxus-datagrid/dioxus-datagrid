//! Sort keys, sort direction and the total order the grid sorts by.

use crate::ColumnId;
use core::cmp::Ordering;

/// How text is compared when sorting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TextCollation {
    /// Compare case-insensitively. Values that differ only in case fall back to
    /// a case-sensitive comparison, so the order stays total and deterministic.
    #[default]
    CaseInsensitive,
    /// Compare by Unicode scalar value, so `Z` sorts before `a`.
    CaseSensitive,
}

/// The value a column sorts by.
///
/// Sorting reads the same typed value as formatting, filtering and export do;
/// this alias keeps the name the sort API has always used.
pub type SortValue<'a> = crate::CellValue<'a>;

/// The direction a column is sorted in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SortDirection {
    /// Smallest first; [`SortValue::None`] last.
    #[default]
    Asc,
    /// Largest first; [`SortValue::None`] first.
    Desc,
}

impl SortDirection {
    /// Returns the opposite direction.
    #[must_use]
    pub const fn reversed(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }

    /// Applies this direction to an ordering produced in ascending order.
    #[must_use]
    pub const fn apply(self, ordering: Ordering) -> Ordering {
        match self {
            Self::Asc => ordering,
            Self::Desc => ordering.reverse(),
        }
    }
}

/// One entry of the sort. Within [`GridState::sort`](crate::GridState::sort)
/// the position in the vector is the priority — earlier entries win.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SortState {
    /// The column being sorted.
    pub column: ColumnId,
    /// The direction to sort it in.
    pub direction: SortDirection,
}

impl SortState {
    /// Creates a sort entry.
    #[must_use]
    pub fn new(column: impl Into<ColumnId>, direction: SortDirection) -> Self {
        Self {
            column: column.into(),
            direction,
        }
    }

    /// Creates an ascending sort entry.
    #[must_use]
    pub fn asc(column: impl Into<ColumnId>) -> Self {
        Self::new(column, SortDirection::Asc)
    }

    /// Creates a descending sort entry.
    #[must_use]
    pub fn desc(column: impl Into<ColumnId>) -> Self {
        Self::new(column, SortDirection::Desc)
    }
}
