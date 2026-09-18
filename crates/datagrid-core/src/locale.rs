//! Every text the grid shows and every number and date format it uses, in one
//! place.

use crate::{CellFormat, CellValue};
use std::borrow::Cow;
#[cfg(feature = "chrono")]
use std::fmt::Write as _;

/// The texts and formats of one language.
///
/// Everything the grid itself writes — button labels, announcements, the
/// decimal separator — comes from here, so that switching the locale switches
/// all of it. Column labels and cell content are the app's and stay as they
/// are.
///
/// English is the default; [`GridLocale::german`] is the second locale that
/// ships. For any other language start from either and change the fields:
///
/// ```
/// use datagrid_core::GridLocale;
///
/// let french = GridLocale {
///     search: "Rechercher".into(),
///     next: "Suivant".into(),
///     page_of: "Page {page} sur {count}".into(),
///     ..GridLocale::german()
/// };
/// assert_eq!(french.page_of(1, 3), "Page 1 sur 3");
/// ```
///
/// Texts with a placeholder in braces, such as `{count}`, are templates; the
/// method of the same name fills them in.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct GridLocale {
    /// Separates the integer part from the decimals: `.` or `,`.
    pub decimal_separator: char,
    /// Groups thousands in formatted numbers: `,`, `.`, or a space.
    pub group_separator: char,
    /// Whether the currency symbol follows the amount (`1,00 €`) rather than
    /// preceding it (`$1.00`).
    pub currency_after: bool,
    /// Whether a space separates a percentage from its sign (`25 %`).
    pub percent_space: bool,
    /// The `chrono` pattern for dates, such as `"%d.%m.%Y"`.
    pub date_pattern: Cow<'static, str>,
    /// The `chrono` pattern for a date with a time.
    pub date_time_pattern: Cow<'static, str>,
    /// How `true` reads.
    pub yes: Cow<'static, str>,
    /// How `false` reads.
    pub no: Cow<'static, str>,
    /// Label and placeholder of the search box.
    pub search: Cow<'static, str>,
    /// Shown when no row matches.
    pub no_rows: Cow<'static, str>,
    /// Accessible name of the pagination.
    pub pagination: Cow<'static, str>,
    /// Accessible name of the button to the previous page.
    pub previous_page: Cow<'static, str>,
    /// Visible text of the button to the previous page.
    pub previous: Cow<'static, str>,
    /// Accessible name of the button to the next page.
    pub next_page: Cow<'static, str>,
    /// Visible text of the button to the next page.
    pub next: Cow<'static, str>,
    /// Where the pagination is, with `{page}` and `{count}`.
    pub page_of: Cow<'static, str>,
    /// Announced while a remote grid loads.
    pub loading: Cow<'static, str>,
    /// Button that repeats a failed request.
    pub retry: Cow<'static, str>,
    /// Accessible name of a column's filter input, with `{column}`.
    pub filter_column: Cow<'static, str>,
    /// Label of the menu that shows and hides columns.
    pub columns: Cow<'static, str>,
    /// How many rows match, for exactly one row.
    pub row_count_one: Cow<'static, str>,
    /// How many rows match, with `{count}`.
    pub row_count: Cow<'static, str>,
    /// How many rows are selected, with `{count}`.
    pub selected_count: Cow<'static, str>,
}

impl Default for GridLocale {
    fn default() -> Self {
        Self::english()
    }
}

impl GridLocale {
    /// English, the default.
    #[must_use]
    pub const fn english() -> Self {
        Self {
            decimal_separator: '.',
            group_separator: ',',
            currency_after: false,
            percent_space: false,
            date_pattern: Cow::Borrowed("%m/%d/%Y"),
            date_time_pattern: Cow::Borrowed("%m/%d/%Y %H:%M"),
            yes: Cow::Borrowed("Yes"),
            no: Cow::Borrowed("No"),
            search: Cow::Borrowed("Search"),
            no_rows: Cow::Borrowed("No matching rows"),
            pagination: Cow::Borrowed("Pagination"),
            previous_page: Cow::Borrowed("Previous page"),
            previous: Cow::Borrowed("Previous"),
            next_page: Cow::Borrowed("Next page"),
            next: Cow::Borrowed("Next"),
            page_of: Cow::Borrowed("Page {page} of {count}"),
            loading: Cow::Borrowed("Loading…"),
            retry: Cow::Borrowed("Retry"),
            filter_column: Cow::Borrowed("Filter {column}"),
            columns: Cow::Borrowed("Columns"),
            row_count_one: Cow::Borrowed("1 row"),
            row_count: Cow::Borrowed("{count} rows"),
            selected_count: Cow::Borrowed("{count} selected"),
        }
    }

    /// German.
    #[must_use]
    pub const fn german() -> Self {
        Self {
            decimal_separator: ',',
            group_separator: '.',
            currency_after: true,
            percent_space: true,
            date_pattern: Cow::Borrowed("%d.%m.%Y"),
            date_time_pattern: Cow::Borrowed("%d.%m.%Y %H:%M"),
            yes: Cow::Borrowed("Ja"),
            no: Cow::Borrowed("Nein"),
            search: Cow::Borrowed("Suchen"),
            no_rows: Cow::Borrowed("Keine passenden Zeilen"),
            pagination: Cow::Borrowed("Seitennavigation"),
            previous_page: Cow::Borrowed("Vorherige Seite"),
            previous: Cow::Borrowed("Zurück"),
            next_page: Cow::Borrowed("Nächste Seite"),
            next: Cow::Borrowed("Weiter"),
            page_of: Cow::Borrowed("Seite {page} von {count}"),
            loading: Cow::Borrowed("Wird geladen …"),
            retry: Cow::Borrowed("Erneut versuchen"),
            filter_column: Cow::Borrowed("{column} filtern"),
            columns: Cow::Borrowed("Spalten"),
            row_count_one: Cow::Borrowed("1 Zeile"),
            row_count: Cow::Borrowed("{count} Zeilen"),
            selected_count: Cow::Borrowed("{count} ausgewählt"),
        }
    }

    /// "Page 2 of 5", one-based.
    #[must_use]
    pub fn page_of(&self, page: usize, count: usize) -> String {
        fill(
            &self.page_of,
            &[
                ("page", &self.integer(page)),
                ("count", &self.integer(count)),
            ],
        )
    }

    /// "Filter Name", the accessible name of a column's filter input.
    #[must_use]
    pub fn filter_column(&self, column: &str) -> String {
        fill(&self.filter_column, &[("column", column)])
    }

    /// "12 rows", or "1 row".
    #[must_use]
    pub fn row_count(&self, count: usize) -> String {
        if count == 1 {
            self.row_count_one.clone().into_owned()
        } else {
            fill(&self.row_count, &[("count", &self.integer(count))])
        }
    }

    /// "3 selected".
    #[must_use]
    pub fn selected_count(&self, count: usize) -> String {
        fill(&self.selected_count, &[("count", &self.integer(count))])
    }

    /// A count with thousands grouping, as used in the texts above.
    #[must_use]
    pub fn integer(&self, value: usize) -> String {
        group(&value.to_string(), self.group_separator)
    }

    /// Formats a value as text.
    ///
    /// A format that does not fit the value falls back to
    /// [`CellFormat::Plain`]: text stays text under a number format, and a
    /// number is not a date. [`CellValue::None`] is always empty.
    #[must_use]
    pub fn format(&self, value: &CellValue<'_>, format: &CellFormat) -> String {
        match (format, value) {
            (_, CellValue::None) => String::new(),
            (CellFormat::Number { decimals }, CellValue::Int(_) | CellValue::Float(_)) => {
                self.fixed(value, *decimals)
            }
            (
                CellFormat::Currency { symbol, decimals },
                CellValue::Int(_) | CellValue::Float(_),
            ) => self.currency(value, symbol, *decimals),
            (CellFormat::Percent { decimals }, CellValue::Int(_) | CellValue::Float(_)) => {
                self.percent(value, *decimals)
            }
            #[cfg(feature = "chrono")]
            (_, CellValue::Date(_) | CellValue::DateTime(_)) => self.temporal(value, format),
            _ => self.plain(value),
        }
    }

    fn plain(&self, value: &CellValue<'_>) -> String {
        match value {
            CellValue::None => String::new(),
            CellValue::Bool(true) => self.yes.clone().into_owned(),
            CellValue::Bool(false) => self.no.clone().into_owned(),
            CellValue::Int(value) => value.to_string(),
            CellValue::Float(value) => value
                .to_string()
                .replace('.', self.decimal_separator.encode_utf8(&mut [0; 4])),
            #[cfg(feature = "chrono")]
            CellValue::Date(_) | CellValue::DateTime(_) => self.temporal(value, &CellFormat::Plain),
            CellValue::Text(text) => (*text).to_owned(),
        }
    }

    /// A number rounded to `decimals`, with grouping.
    fn fixed(&self, value: &CellValue<'_>, decimals: u8) -> String {
        let decimals = usize::from(decimals);
        let (negative, integer, fraction) = match *value {
            CellValue::Int(value) => (
                value < 0,
                value.unsigned_abs().to_string(),
                "0".repeat(decimals),
            ),
            CellValue::Float(value) if value.is_finite() => {
                let rounded = format!("{:.*}", decimals, value.abs());
                let (integer, fraction) = rounded.split_once('.').unwrap_or((&rounded, ""));
                // -0.001 rounded to two decimals is zero, and zero has no sign.
                let is_zero = rounded.bytes().all(|byte| matches!(byte, b'0' | b'.'));
                (
                    value.is_sign_negative() && !is_zero,
                    integer.to_owned(),
                    fraction.to_owned(),
                )
            }
            _ => return self.plain(value),
        };

        let mut text = String::with_capacity(integer.len() + decimals + 4);
        if negative {
            text.push('-');
        }
        text.push_str(&group(&integer, self.group_separator));
        if !fraction.is_empty() {
            text.push(self.decimal_separator);
            text.push_str(&fraction);
        }
        text
    }

    fn currency(&self, value: &CellValue<'_>, symbol: &str, decimals: u8) -> String {
        let amount = self.fixed(value, decimals);
        if self.currency_after {
            format!("{amount}\u{a0}{symbol}")
        } else {
            let (sign, digits) = match amount.strip_prefix('-') {
                Some(digits) => ("-", digits),
                None => ("", amount.as_str()),
            };
            // A code such as CHF needs a space before the digits; a sign such
            // as $ does not.
            let gap = if symbol.chars().all(char::is_alphabetic) {
                "\u{a0}"
            } else {
                ""
            };
            format!("{sign}{symbol}{gap}{digits}")
        }
    }

    fn percent(&self, value: &CellValue<'_>, decimals: u8) -> String {
        #[allow(clippy::cast_precision_loss)]
        let fraction = match *value {
            CellValue::Int(value) => value as f64,
            CellValue::Float(value) => value,
            _ => return self.plain(value),
        };
        let number = self.fixed(&CellValue::Float(fraction * 100.0), decimals);
        if self.percent_space {
            format!("{number}\u{a0}%")
        } else {
            format!("{number}%")
        }
    }

    #[cfg(feature = "chrono")]
    fn temporal(&self, value: &CellValue<'_>, format: &CellFormat) -> String {
        let pattern: &str = match (format, value) {
            (CellFormat::DatePattern(pattern), _) => pattern,
            (CellFormat::DateTime, CellValue::DateTime(_)) => &self.date_time_pattern,
            (CellFormat::Date, _) | (_, CellValue::Date(_)) => &self.date_pattern,
            _ => &self.date_time_pattern,
        };

        // `to_string` on chrono's formatter panics on an invalid pattern;
        // writing reports it as an error instead, and ISO 8601 stands in.
        let mut text = String::new();
        let written = match value {
            CellValue::Date(date) => write!(text, "{}", date.format(pattern)),
            CellValue::DateTime(date_time) => write!(text, "{}", date_time.format(pattern)),
            _ => return self.plain(value),
        };
        if written.is_err() {
            text.clear();
            let _ = match value {
                CellValue::Date(date) => write!(text, "{date}"),
                CellValue::DateTime(date_time) => write!(text, "{date_time}"),
                _ => Ok(()),
            };
        }
        text
    }
}

/// Inserts `separator` between groups of three digits, counted from the end.
fn group(digits: &str, separator: char) -> String {
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (position, digit) in digits.chars().enumerate() {
        if position > 0 && (digits.len() - position) % 3 == 0 {
            grouped.push(separator);
        }
        grouped.push(digit);
    }
    grouped
}

/// Replaces each `{key}` in `template` with its value.
fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut text = template.to_owned();
    for (key, value) in values {
        text = text.replace(&format!("{{{key}}}"), value);
    }
    text
}
