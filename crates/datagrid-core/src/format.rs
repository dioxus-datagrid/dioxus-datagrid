//! How a column turns its [`CellValue`](crate::CellValue) into text, and how it
//! lays that text out.

use std::borrow::Cow;

/// How a column's value is turned into text.
///
/// The format says *what kind* of text; the separators, symbol placement and
/// date patterns come from the [`GridLocale`](crate::GridLocale), so the same
/// column reads `1,234.50` in English and `1.234,50` in German.
///
/// A format that does not fit the value — a date format on a number, say —
/// falls back to [`Plain`](CellFormat::Plain) for that value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CellFormat {
    /// The value as it is: integers without grouping, floats in their shortest
    /// exact form, booleans as the locale's yes and no, dates in the locale's
    /// date pattern. Only the decimal separator is localized.
    #[default]
    Plain,
    /// A number with a fixed count of decimals and thousands grouping.
    Number {
        /// Digits after the decimal separator.
        decimals: u8,
    },
    /// A number as an amount of money.
    Currency {
        /// The currency symbol or code, such as `€` or `CHF`.
        symbol: Cow<'static, str>,
        /// Digits after the decimal separator.
        decimals: u8,
    },
    /// A fraction as a percentage: `0.25` reads `25%`.
    Percent {
        /// Digits after the decimal separator.
        decimals: u8,
    },
    /// A date or date and time in the locale's date pattern, without the time.
    Date,
    /// A date and time in the locale's date-time pattern.
    DateTime,
    /// A date or date and time in an explicit
    /// [`chrono` pattern](https://docs.rs/chrono/latest/chrono/format/strftime/index.html),
    /// such as `"%Y-%m-%d"`. Without the `chrono` feature there are no dates to
    /// apply it to.
    DatePattern(Cow<'static, str>),
}

impl CellFormat {
    /// A number with `decimals` decimals and thousands grouping.
    #[must_use]
    pub const fn number(decimals: u8) -> Self {
        Self::Number { decimals }
    }

    /// An amount of money in the given currency.
    #[must_use]
    pub fn currency(symbol: impl Into<Cow<'static, str>>, decimals: u8) -> Self {
        Self::Currency {
            symbol: symbol.into(),
            decimals,
        }
    }

    /// A fraction shown as a percentage with `decimals` decimals.
    #[must_use]
    pub const fn percent(decimals: u8) -> Self {
        Self::Percent { decimals }
    }

    /// Dates in an explicit `chrono` pattern.
    #[must_use]
    pub fn date_pattern(pattern: impl Into<Cow<'static, str>>) -> Self {
        Self::DatePattern(pattern.into())
    }

    /// Whether this format is for numbers.
    #[must_use]
    pub const fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::Number { .. } | Self::Currency { .. } | Self::Percent { .. }
        )
    }

    /// The alignment a column with this format gets unless it sets its own:
    /// numbers line up at the end, so that their digits align, and everything
    /// else at the start.
    #[must_use]
    pub const fn default_align(&self) -> CellAlign {
        if self.is_numeric() {
            CellAlign::End
        } else {
            CellAlign::Start
        }
    }
}

/// Horizontal alignment of a cell's content.
///
/// Logical rather than left and right, so that it follows the writing
/// direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CellAlign {
    /// At the start of the line: left in left-to-right text.
    #[default]
    Start,
    /// Centered.
    Center,
    /// At the end of the line: right in left-to-right text.
    End,
}

impl CellAlign {
    /// The CSS keyword for this alignment, as used by `text-align`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
        }
    }
}

/// What happens to cell content wider than its column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CellOverflow {
    /// Cut off with an ellipsis.
    #[default]
    Truncate,
    /// Cut off with an ellipsis, with the full text as a tooltip. The tooltip
    /// needs the column's value, since a custom cell renderer's output is not
    /// text the grid can read.
    TruncateWithTooltip,
    /// Wrapped onto further lines, making the row taller. Not for virtualized
    /// grids, whose rows all have the same height.
    Wrap,
}

impl CellOverflow {
    /// A stable name for this mode, for use in a `data-` attribute.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Truncate => "truncate",
            Self::TruncateWithTooltip => "truncate-tooltip",
            Self::Wrap => "wrap",
        }
    }
}
