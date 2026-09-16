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

/// A value extracted from a row to sort by.
///
/// # Ordering
///
/// [`SortValue`] implements a **total order**, which the sort relies on:
///
/// - [`None`](SortValue::None) sorts after every other value, so in ascending
///   order missing values end up last. Descending order reverses that, putting
///   them first.
/// - [`Int`](SortValue::Int) and [`Float`](SortValue::Float) compare
///   numerically *with each other*, so a column that yields both variants still
///   sorts sensibly. Integers beyond 2^53 lose precision in that comparison.
/// - [`Float`](SortValue::Float) uses [`f64::total_cmp`], which orders `NaN`
///   rather than declaring it unordered.
/// - [`Text`](SortValue::Text) compares case-insensitively by default; use
///   [`cmp_with`](SortValue::cmp_with) to pick a different [`TextCollation`].
/// - Across the remaining variants the order is `Bool` < numeric < `Text` <
///   `None`.
///
/// `PartialEq` is defined as "compares equal", so it follows the rules above
/// rather than deriving structural equality. In particular `Float(f64::NAN)`
/// equals itself, and `Int(1)` equals `Float(1.0)`.
#[derive(Clone, Debug)]
pub enum SortValue {
    /// No value — sorts last in ascending order.
    None,
    /// A boolean; `false` sorts before `true`.
    Bool(bool),
    /// An integer.
    Int(i64),
    /// A floating point number, ordered by [`f64::total_cmp`].
    Float(f64),
    /// Text, compared according to the column's [`TextCollation`].
    Text(String),
}

impl SortValue {
    /// Compares two values using an explicit [`TextCollation`].
    ///
    /// [`Ord::cmp`] delegates here with [`TextCollation::CaseInsensitive`].
    #[must_use]
    pub fn cmp_with(&self, other: &Self, collation: TextCollation) -> Ordering {
        match (self, other) {
            (Self::None, Self::None) => Ordering::Equal,
            (Self::Bool(a), Self::Bool(b)) => a.cmp(b),
            (Self::Int(a), Self::Int(b)) => a.cmp(b),
            (Self::Float(a), Self::Float(b)) => a.total_cmp(b),
            #[allow(clippy::cast_precision_loss)]
            (Self::Int(a), Self::Float(b)) => (*a as f64).total_cmp(b),
            #[allow(clippy::cast_precision_loss)]
            (Self::Float(a), Self::Int(b)) => a.total_cmp(&(*b as f64)),
            (Self::Text(a), Self::Text(b)) => compare_text(a, b, collation),
            _ => self.rank().cmp(&other.rank()),
        }
    }

    /// Where this variant sits when two different variants meet.
    ///
    /// `Int` and `Float` share a rank because they compare numerically; the
    /// rank only decides comparisons the match arms above did not handle.
    const fn rank(&self) -> u8 {
        match self {
            Self::Bool(_) => 0,
            Self::Int(_) | Self::Float(_) => 1,
            Self::Text(_) => 2,
            Self::None => 3,
        }
    }

    /// Returns `true` for [`SortValue::None`].
    #[must_use]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Compares two strings without allocating.
fn compare_text(a: &str, b: &str, collation: TextCollation) -> Ordering {
    match collation {
        TextCollation::CaseSensitive => a.cmp(b),
        TextCollation::CaseInsensitive => {
            // Identical strings are the common case in grouping columns, where a
            // sort compares the same handful of values over and over. Settling
            // them with one vectorized `memcmp` avoids walking the bytes and
            // then walking them again for the tiebreak below.
            if a == b {
                return Ordering::Equal;
            }

            let folded = compare_folded(a, b);
            // Fall back to the case-sensitive order so that values differing
            // only in case are not reported as equal — otherwise `Ord` and
            // `PartialEq` would disagree with each other.
            if folded == Ordering::Equal {
                a.cmp(b)
            } else {
                folded
            }
        }
    }
}

/// Case-insensitive comparison that stays byte-wise while both inputs are ASCII.
///
/// Sorting is comparison-bound — a hundred thousand rows cost well over a
/// million comparisons — and full Unicode case folding does a table lookup per
/// character. Byte-wise folding is exact for ASCII, and grid text is
/// overwhelmingly ASCII, so this walks the bytes and only hands off to the
/// Unicode path once it actually meets a non-ASCII byte.
fn compare_folded(a: &str, b: &str) -> Ordering {
    let (left, right) = (a.as_bytes(), b.as_bytes());
    let shared = left.len().min(right.len());

    for position in 0..shared {
        let (Some(&x), Some(&y)) = (left.get(position), right.get(position)) else {
            break;
        };

        if !x.is_ascii() || !y.is_ascii() {
            return compare_folded_unicode(a, b);
        }

        let ordering = x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase());
        if ordering != Ordering::Equal {
            return ordering;
        }
    }

    // The shared prefix matched, so only the longer string's tail is left to
    // decide — unless that tail is non-ASCII, where folding can change length
    // and the byte-wise shortcut stops being exact.
    let left_tail = left.get(shared..).unwrap_or_default();
    let right_tail = right.get(shared..).unwrap_or_default();
    if left_tail.is_ascii() && right_tail.is_ascii() {
        left.len().cmp(&right.len())
    } else {
        compare_folded_unicode(a, b)
    }
}

/// Full Unicode case-insensitive comparison, used once non-ASCII text appears.
fn compare_folded_unicode(a: &str, b: &str) -> Ordering {
    a.chars()
        .flat_map(char::to_lowercase)
        .cmp(b.chars().flat_map(char::to_lowercase))
}

impl Ord for SortValue {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cmp_with(other, TextCollation::CaseInsensitive)
    }
}

impl PartialOrd for SortValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for SortValue {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for SortValue {}

impl From<String> for SortValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for SortValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<bool> for SortValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<f64> for SortValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<f32> for SortValue {
    fn from(value: f32) -> Self {
        Self::Float(f64::from(value))
    }
}

/// Implements `From` for integer types that fit into an `i64` without loss.
macro_rules! impl_from_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for SortValue {
                fn from(value: $ty) -> Self {
                    Self::Int(i64::from(value))
                }
            }
        )*
    };
}

impl_from_int!(i8, i16, i32, i64, u8, u16, u32);

/// Implements `From` for integer types that may exceed `i64`.
///
/// Values that do not fit degrade to [`SortValue::Float`], which keeps them
/// ordered relative to everything else at the cost of precision.
macro_rules! impl_from_wide_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for SortValue {
                fn from(value: $ty) -> Self {
                    #[allow(clippy::cast_precision_loss)]
                    i64::try_from(value).map_or_else(|_| Self::Float(value as f64), Self::Int)
                }
            }
        )*
    };
}

impl_from_wide_int!(u64, usize, i128, u128);

impl<V> From<Option<V>> for SortValue
where
    V: Into<SortValue>,
{
    fn from(value: Option<V>) -> Self {
        value.map_or(Self::None, Into::into)
    }
}

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
