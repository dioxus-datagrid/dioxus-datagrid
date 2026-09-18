//! The typed value a column reads from a row, and the total order over it.

use crate::TextCollation;
use core::cmp::Ordering;

/// A typed value read from one cell of a row.
///
/// A column reads it through [`ColumnSpec::value`](crate::ColumnSpec::value),
/// and everything that needs to know what a cell *is* rather than how it is
/// drawn works from it: sorting today, formatting, typed filters, aggregates and
/// export as they arrive. [`SortValue`](crate::SortValue) is the same type under
/// the name the sort API has always used.
///
/// # Ordering
///
/// [`CellValue`] implements a **total order**, which the sort relies on:
///
/// - [`None`](CellValue::None) sorts after every other value, so in ascending
///   order missing values end up last. Descending order reverses that, putting
///   them first.
/// - [`Int`](CellValue::Int) and [`Float`](CellValue::Float) compare
///   numerically *with each other*, so a column that yields both variants still
///   sorts sensibly. Integers beyond 2^53 lose precision in that comparison.
/// - [`Float`](CellValue::Float) uses [`f64::total_cmp`], which orders `NaN`
///   rather than declaring it unordered.
/// - With the `chrono` feature, `Date` and `DateTime` compare chronologically
///   with each other; a date counts as midnight at its start.
/// - [`Text`](CellValue::Text) compares case-insensitively by default; use
///   [`cmp_with`](CellValue::cmp_with) to pick a different [`TextCollation`].
/// - Across the remaining variants the order is `Bool` < numeric < temporal <
///   `Text` < `None`.
///
/// `PartialEq` is defined as "compares equal", so it follows the rules above
/// rather than deriving structural equality. In particular `Float(f64::NAN)`
/// equals itself, and `Int(1)` equals `Float(1.0)`.
///
/// # Borrowing
///
/// [`Text`](CellValue::Text) borrows from the row rather than owning a
/// `String`. Sorting a large grid extracts one key per row per sort column, and
/// owning them meant an allocation each — hundreds of thousands of them for a
/// six-figure row count, for values thrown away as soon as the sort finished.
///
/// The cost is that a text value has to *exist* in the row; it cannot be
/// computed on the fly. In practice that is rarely a real constraint:
///
/// - Sorting by a formatted value (a date, a currency amount) should sort by the
///   underlying number anyway, which orders correctly where the formatted text
///   would not. Formatting is the column's job, see
///   [`CellFormat`](crate::CellFormat).
/// - A label for an enum can be returned as `&'static str`, which coerces here.
/// - Sorting by something like `"last, first"` is a two-column sort, which is
///   what [`GridState::sort`](crate::GridState::sort) is for.
///
/// Where a computed text is genuinely needed, store it on the row.
#[derive(Clone, Copy, Debug)]
pub enum CellValue<'a> {
    /// No value — sorts last in ascending order.
    None,
    /// A boolean; `false` sorts before `true`.
    Bool(bool),
    /// An integer.
    Int(i64),
    /// A floating point number, ordered by [`f64::total_cmp`].
    Float(f64),
    /// A calendar date.
    #[cfg(feature = "chrono")]
    Date(chrono::NaiveDate),
    /// A date and time of day, without a time zone.
    #[cfg(feature = "chrono")]
    DateTime(chrono::NaiveDateTime),
    /// Text borrowed from the row, compared according to the column's
    /// [`TextCollation`].
    Text(&'a str),
}

impl<'a> CellValue<'a> {
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
            #[cfg(feature = "chrono")]
            (Self::Date(a), Self::Date(b)) => a.cmp(b),
            #[cfg(feature = "chrono")]
            (Self::DateTime(a), Self::DateTime(b)) => a.cmp(b),
            #[cfg(feature = "chrono")]
            (Self::Date(a), Self::DateTime(b)) => a.and_time(chrono::NaiveTime::MIN).cmp(b),
            #[cfg(feature = "chrono")]
            (Self::DateTime(a), Self::Date(b)) => a.cmp(&b.and_time(chrono::NaiveTime::MIN)),
            (Self::Text(a), Self::Text(b)) => compare_text(a, b, collation),
            _ => self.rank().cmp(&other.rank()),
        }
    }

    /// Where this variant sits when two different variants meet.
    ///
    /// Variants that compare with each other share a rank; the rank only
    /// decides comparisons the match arms above did not handle.
    const fn rank(&self) -> u8 {
        match self {
            Self::Bool(_) => 0,
            Self::Int(_) | Self::Float(_) => 1,
            #[cfg(feature = "chrono")]
            Self::Date(_) | Self::DateTime(_) => 2,
            Self::Text(_) => 3,
            Self::None => 4,
        }
    }

    /// Returns `true` for [`CellValue::None`].
    #[must_use]
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Whether this is a number, which formats and aligns like one.
    #[must_use]
    pub const fn is_numeric(&self) -> bool {
        matches!(self, Self::Int(_) | Self::Float(_))
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

impl Ord for CellValue<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cmp_with(other, TextCollation::CaseInsensitive)
    }
}

impl PartialOrd for CellValue<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for CellValue<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for CellValue<'_> {}

impl<'a> From<&'a str> for CellValue<'a> {
    fn from(value: &'a str) -> Self {
        Self::Text(value)
    }
}

impl<'a> From<&'a String> for CellValue<'a> {
    fn from(value: &'a String) -> Self {
        Self::Text(value.as_str())
    }
}

impl From<bool> for CellValue<'_> {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<f64> for CellValue<'_> {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<f32> for CellValue<'_> {
    fn from(value: f32) -> Self {
        Self::Float(f64::from(value))
    }
}

/// Implements `From` for integer types that fit into an `i64` without loss.
macro_rules! impl_from_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for CellValue<'_> {
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
/// Values that do not fit degrade to [`CellValue::Float`], which keeps them
/// ordered relative to everything else at the cost of precision.
macro_rules! impl_from_wide_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for CellValue<'_> {
                fn from(value: $ty) -> Self {
                    #[allow(clippy::cast_precision_loss)]
                    i64::try_from(value).map_or_else(|_| Self::Float(value as f64), Self::Int)
                }
            }
        )*
    };
}

impl_from_wide_int!(u64, usize, i128, u128);

impl<'a, V> From<Option<V>> for CellValue<'a>
where
    V: Into<CellValue<'a>>,
{
    fn from(value: Option<V>) -> Self {
        value.map_or(Self::None, Into::into)
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::NaiveDate> for CellValue<'_> {
    fn from(value: chrono::NaiveDate) -> Self {
        Self::Date(value)
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::NaiveDateTime> for CellValue<'_> {
    fn from(value: chrono::NaiveDateTime) -> Self {
        Self::DateTime(value)
    }
}
