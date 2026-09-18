//! Typed column filters: conditions with operators, combined with AND or OR,
//! and the shortcuts the filter bar understands.

use crate::{CellValue, TextCollation};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

/// What kind of value a column holds, which decides the operators a filter
/// menu offers and how typed text is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ValueKind {
    /// Text.
    Text,
    /// Integers and floats.
    Number,
    /// `true` or `false`.
    Bool,
    /// A calendar date.
    Date,
    /// A date with a time of day.
    DateTime,
}

impl ValueKind {
    /// The kind of `value`, or `None` for [`CellValue::None`].
    #[must_use]
    pub const fn of(value: &CellValue<'_>) -> Option<Self> {
        match value {
            CellValue::None => None,
            CellValue::Bool(_) => Some(Self::Bool),
            CellValue::Int(_) | CellValue::Float(_) => Some(Self::Number),
            #[cfg(feature = "chrono")]
            CellValue::Date(_) => Some(Self::Date),
            #[cfg(feature = "chrono")]
            CellValue::DateTime(_) => Some(Self::DateTime),
            CellValue::Text(_) => Some(Self::Text),
        }
    }

    /// The operators that make sense for this kind, in the order a menu
    /// should list them.
    #[must_use]
    pub const fn operators(self) -> &'static [FilterOp] {
        use FilterOp::{
            Between, Contains, EndsWith, Equals, Greater, GreaterOrEqual, IsEmpty, IsNotEmpty,
            Less, LessOrEqual, NotEquals, StartsWith,
        };
        match self {
            Self::Text => &[
                Contains, StartsWith, EndsWith, Equals, NotEquals, IsEmpty, IsNotEmpty,
            ],
            Self::Number | Self::Date | Self::DateTime => &[
                Equals,
                NotEquals,
                Less,
                LessOrEqual,
                Greater,
                GreaterOrEqual,
                Between,
                IsEmpty,
                IsNotEmpty,
            ],
            Self::Bool => &[Equals, IsEmpty, IsNotEmpty],
        }
    }
}

/// A value a filter compares against. The owned counterpart of
/// [`CellValue`], so that it can live in [`GridState`](crate::GridState) and
/// travel to a server.
///
/// [`Text`](FilterValue::Text) is also what a user types: compared with a
/// number or a date, it is read as one ([`FilterValue::coerce`]), so `">100"`
/// in the filter bar works on a number column without the bar knowing the
/// column's kind.
///
/// Equality and hashing treat floats by their bits, so that a filter can be a
/// key and `NaN` equals itself.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FilterValue {
    /// Text, or a number or date not yet read as one.
    Text(String),
    /// An integer.
    Int(i64),
    /// A float.
    Float(f64),
    /// A boolean.
    Bool(bool),
    /// A calendar date.
    #[cfg(feature = "chrono")]
    Date(chrono::NaiveDate),
    /// A date and time.
    #[cfg(feature = "chrono")]
    DateTime(chrono::NaiveDateTime),
}

impl PartialEq for FilterValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a.to_bits() == b.to_bits(),
            (Self::Bool(a), Self::Bool(b)) => a == b,
            #[cfg(feature = "chrono")]
            (Self::Date(a), Self::Date(b)) => a == b,
            #[cfg(feature = "chrono")]
            (Self::DateTime(a), Self::DateTime(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for FilterValue {}

impl Hash for FilterValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Self::Text(value) => value.hash(state),
            Self::Int(value) => value.hash(state),
            Self::Float(value) => value.to_bits().hash(state),
            Self::Bool(value) => value.hash(state),
            #[cfg(feature = "chrono")]
            Self::Date(value) => value.hash(state),
            #[cfg(feature = "chrono")]
            Self::DateTime(value) => value.hash(state),
        }
    }
}

impl FilterValue {
    /// Takes an owned copy of a cell value. `None` for [`CellValue::None`],
    /// which a filter expresses with [`FilterOp::IsEmpty`] instead.
    #[must_use]
    pub fn from_cell(value: &CellValue<'_>) -> Option<Self> {
        Some(match *value {
            CellValue::None => return None,
            CellValue::Bool(value) => Self::Bool(value),
            CellValue::Int(value) => Self::Int(value),
            CellValue::Float(value) => Self::Float(value),
            #[cfg(feature = "chrono")]
            CellValue::Date(value) => Self::Date(value),
            #[cfg(feature = "chrono")]
            CellValue::DateTime(value) => Self::DateTime(value),
            CellValue::Text(value) => Self::Text(value.to_owned()),
        })
    }

    /// Borrows this value as a cell value.
    #[must_use]
    pub fn as_cell(&self) -> CellValue<'_> {
        match self {
            Self::Text(value) => CellValue::Text(value),
            Self::Int(value) => CellValue::Int(*value),
            Self::Float(value) => CellValue::Float(*value),
            Self::Bool(value) => CellValue::Bool(*value),
            #[cfg(feature = "chrono")]
            Self::Date(value) => CellValue::Date(*value),
            #[cfg(feature = "chrono")]
            Self::DateTime(value) => CellValue::DateTime(*value),
        }
    }

    /// This value as a value of `kind`, reading text if it has to.
    ///
    /// Numbers accept `.` or `,` as the decimal separator. Dates accept
    /// `2026-09-18`, `18.09.2026` and `09/18/2026`; a date and time also
    /// `2026-09-18 14:05` or `2026-09-18T14:05`. Booleans accept `true`,
    /// `false`, `yes`, `no`, `ja`, `nein`, `1` and `0`. `None` if the value
    /// cannot be read as `kind`.
    #[must_use]
    pub fn coerce(&self, kind: ValueKind) -> Option<Self> {
        match (kind, self) {
            (ValueKind::Text, Self::Text(_))
            | (ValueKind::Number, Self::Int(_) | Self::Float(_))
            | (ValueKind::Bool, Self::Bool(_)) => Some(self.clone()),
            #[cfg(feature = "chrono")]
            (ValueKind::Date, Self::Date(_)) | (ValueKind::DateTime, Self::DateTime(_)) => {
                Some(self.clone())
            }
            #[cfg(feature = "chrono")]
            (ValueKind::DateTime, Self::Date(date)) => {
                Some(Self::DateTime(date.and_time(chrono::NaiveTime::MIN)))
            }
            (_, Self::Text(text)) => Self::parse(kind, text),
            (ValueKind::Text, other) => Some(Self::Text(plain_text(&other.as_cell()))),
            _ => None,
        }
    }

    /// How this value reads in a form input, and back through
    /// [`FilterValue::parse`]: dates in the ISO forms that `type="date"` and
    /// `type="datetime-local"` inputs use.
    #[must_use]
    pub fn edit_text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Int(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::Bool(value) => value.to_string(),
            #[cfg(feature = "chrono")]
            Self::Date(value) => value.format("%Y-%m-%d").to_string(),
            #[cfg(feature = "chrono")]
            Self::DateTime(value) => value.format("%Y-%m-%dT%H:%M").to_string(),
        }
    }

    /// Reads `text` as a value of `kind`. See [`FilterValue::coerce`].
    #[must_use]
    pub fn parse(kind: ValueKind, text: &str) -> Option<Self> {
        let text = text.trim();
        match kind {
            ValueKind::Text => Some(Self::Text(text.to_owned())),
            ValueKind::Number => parse_number(text),
            ValueKind::Bool => match text.to_lowercase().as_str() {
                "true" | "yes" | "ja" | "1" => Some(Self::Bool(true)),
                "false" | "no" | "nein" | "0" => Some(Self::Bool(false)),
                _ => None,
            },
            #[cfg(feature = "chrono")]
            ValueKind::Date => parse_date(text).map(Self::Date),
            #[cfg(feature = "chrono")]
            ValueKind::DateTime => parse_date_time(text).map(Self::DateTime),
            #[cfg(not(feature = "chrono"))]
            ValueKind::Date | ValueKind::DateTime => None,
        }
    }
}

impl From<&str> for FilterValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for FilterValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<i64> for FilterValue {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<i32> for FilterValue {
    fn from(value: i32) -> Self {
        Self::Int(i64::from(value))
    }
}

impl From<u32> for FilterValue {
    fn from(value: u32) -> Self {
        Self::Int(i64::from(value))
    }
}

impl From<f64> for FilterValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<bool> for FilterValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::NaiveDate> for FilterValue {
    fn from(value: chrono::NaiveDate) -> Self {
        Self::Date(value)
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::NaiveDateTime> for FilterValue {
    fn from(value: chrono::NaiveDateTime) -> Self {
        Self::DateTime(value)
    }
}

fn parse_number(text: &str) -> Option<FilterValue> {
    if let Ok(value) = text.parse::<i64>() {
        return Some(FilterValue::Int(value));
    }
    // One comma and no dot is a German decimal comma.
    let normalized = if text.contains(',') && !text.contains('.') {
        text.replacen(',', ".", 1)
    } else {
        text.to_owned()
    };
    normalized
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(FilterValue::Float)
}

#[cfg(feature = "chrono")]
fn parse_date(text: &str) -> Option<chrono::NaiveDate> {
    ["%Y-%m-%d", "%d.%m.%Y", "%m/%d/%Y"]
        .iter()
        .find_map(|pattern| chrono::NaiveDate::parse_from_str(text, pattern).ok())
}

#[cfg(feature = "chrono")]
fn parse_date_time(text: &str) -> Option<chrono::NaiveDateTime> {
    [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
        "%d.%m.%Y %H:%M",
        "%m/%d/%Y %H:%M",
    ]
    .iter()
    .find_map(|pattern| chrono::NaiveDateTime::parse_from_str(text, pattern).ok())
    .or_else(|| parse_date(text).map(|date| date.and_time(chrono::NaiveTime::MIN)))
}

/// How a condition compares a cell with its operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FilterOp {
    /// Text contains the operand, ignoring case.
    Contains,
    /// Text starts with the operand, ignoring case.
    StartsWith,
    /// Text ends with the operand, ignoring case.
    EndsWith,
    /// Equal to the operand; text ignoring case.
    Equals,
    /// Not equal to the operand. An empty cell counts as not equal.
    NotEquals,
    /// Less than the operand.
    Less,
    /// Less than or equal to the operand.
    LessOrEqual,
    /// Greater than the operand.
    Greater,
    /// Greater than or equal to the operand.
    GreaterOrEqual,
    /// Between the operand and a second one, both included, in either order.
    Between,
    /// Empty: no value, or empty text.
    IsEmpty,
    /// Not empty.
    IsNotEmpty,
    /// Equal to any of a list of values: the value list of a filter menu.
    OneOf,
}

impl FilterOp {
    /// How many operands the operator takes: 0, 1, 2, or `None` for a list.
    #[must_use]
    pub const fn operands(self) -> Option<usize> {
        match self {
            Self::IsEmpty | Self::IsNotEmpty => Some(0),
            Self::Between => Some(2),
            Self::OneOf => None,
            _ => Some(1),
        }
    }

    /// A stable name, for `data-` attributes and wire formats.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::StartsWith => "starts-with",
            Self::EndsWith => "ends-with",
            Self::Equals => "equals",
            Self::NotEquals => "not-equals",
            Self::Less => "less",
            Self::LessOrEqual => "less-or-equal",
            Self::Greater => "greater",
            Self::GreaterOrEqual => "greater-or-equal",
            Self::Between => "between",
            Self::IsEmpty => "is-empty",
            Self::IsNotEmpty => "is-not-empty",
            Self::OneOf => "one-of",
        }
    }

    /// The operator named by [`as_str`](FilterOp::as_str).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Self::Contains,
            Self::StartsWith,
            Self::EndsWith,
            Self::Equals,
            Self::NotEquals,
            Self::Less,
            Self::LessOrEqual,
            Self::Greater,
            Self::GreaterOrEqual,
            Self::Between,
            Self::IsEmpty,
            Self::IsNotEmpty,
            Self::OneOf,
        ]
        .into_iter()
        .find(|op| op.as_str() == name)
    }
}

/// One condition: an operator and its operands.
///
/// Built with the constructors, such as [`Condition::greater`] or
/// [`Condition::one_of`]. The operands are [`FilterValue`]s; text operands are
/// read as numbers or dates when the cell is one.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Condition {
    /// The operator.
    pub op: FilterOp,
    /// The operands: none, one, two for [`FilterOp::Between`], or the list for
    /// [`FilterOp::OneOf`].
    pub values: Vec<FilterValue>,
}

impl Condition {
    /// A condition from an operator and its operands.
    #[must_use]
    pub const fn new(op: FilterOp, values: Vec<FilterValue>) -> Self {
        Self { op, values }
    }

    /// Text contains `value`.
    #[must_use]
    pub fn contains(value: impl Into<FilterValue>) -> Self {
        Self::new(FilterOp::Contains, vec![value.into()])
    }

    /// Text starts with `value`.
    #[must_use]
    pub fn starts_with(value: impl Into<FilterValue>) -> Self {
        Self::new(FilterOp::StartsWith, vec![value.into()])
    }

    /// Text ends with `value`.
    #[must_use]
    pub fn ends_with(value: impl Into<FilterValue>) -> Self {
        Self::new(FilterOp::EndsWith, vec![value.into()])
    }

    /// Equal to `value`.
    #[must_use]
    pub fn equals(value: impl Into<FilterValue>) -> Self {
        Self::new(FilterOp::Equals, vec![value.into()])
    }

    /// Not equal to `value`.
    #[must_use]
    pub fn not_equals(value: impl Into<FilterValue>) -> Self {
        Self::new(FilterOp::NotEquals, vec![value.into()])
    }

    /// Less than `value`.
    #[must_use]
    pub fn less(value: impl Into<FilterValue>) -> Self {
        Self::new(FilterOp::Less, vec![value.into()])
    }

    /// Less than or equal to `value`.
    #[must_use]
    pub fn less_or_equal(value: impl Into<FilterValue>) -> Self {
        Self::new(FilterOp::LessOrEqual, vec![value.into()])
    }

    /// Greater than `value`.
    #[must_use]
    pub fn greater(value: impl Into<FilterValue>) -> Self {
        Self::new(FilterOp::Greater, vec![value.into()])
    }

    /// Greater than or equal to `value`.
    #[must_use]
    pub fn greater_or_equal(value: impl Into<FilterValue>) -> Self {
        Self::new(FilterOp::GreaterOrEqual, vec![value.into()])
    }

    /// Between `low` and `high`, both included.
    #[must_use]
    pub fn between(low: impl Into<FilterValue>, high: impl Into<FilterValue>) -> Self {
        Self::new(FilterOp::Between, vec![low.into(), high.into()])
    }

    /// No value, or empty text.
    #[must_use]
    pub const fn is_empty() -> Self {
        Self::new(FilterOp::IsEmpty, Vec::new())
    }

    /// Has a value.
    #[must_use]
    pub const fn is_not_empty() -> Self {
        Self::new(FilterOp::IsNotEmpty, Vec::new())
    }

    /// Equal to one of `values`.
    #[must_use]
    pub fn one_of(values: impl IntoIterator<Item = impl Into<FilterValue>>) -> Self {
        Self::new(
            FilterOp::OneOf,
            values.into_iter().map(Into::into).collect(),
        )
    }

    /// Reads the filter bar's text: a leading `=`, `!=`, `<`, `<=`, `>` or
    /// `>=` makes a comparison, `a..b` a range, and anything else stays the
    /// case-insensitive substring test the bar has always done. `None` for
    /// empty text.
    ///
    /// ```
    /// use datagrid_core::{Condition, FilterOp};
    ///
    /// assert_eq!(Condition::from_text(">100").map(|c| c.op), Some(FilterOp::Greater));
    /// assert_eq!(Condition::from_text("10..20").map(|c| c.op), Some(FilterOp::Between));
    /// assert_eq!(Condition::from_text("berlin").map(|c| c.op), Some(FilterOp::Contains));
    /// ```
    #[must_use]
    pub fn from_text(text: &str) -> Option<Self> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }

        let prefixes: [(&str, FilterOp); 6] = [
            ("!=", FilterOp::NotEquals),
            ("<=", FilterOp::LessOrEqual),
            (">=", FilterOp::GreaterOrEqual),
            ("=", FilterOp::Equals),
            ("<", FilterOp::Less),
            (">", FilterOp::Greater),
        ];
        for (prefix, op) in prefixes {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let rest = rest.trim();
                // A bare operator is being typed; filtering on nothing would
                // hide rows for no reason.
                if rest.is_empty() {
                    return None;
                }
                return Some(Self::new(op, vec![FilterValue::from(rest)]));
            }
        }

        if let Some((low, high)) = trimmed.split_once("..") {
            let (low, high) = (low.trim(), high.trim());
            if !low.is_empty() && !high.is_empty() {
                return Some(Self::between(low, high));
            }
        }

        // Untrimmed: a trailing space is part of what the user searches for.
        Some(Self::contains(text))
    }

    /// Whether a cell passes this condition.
    ///
    /// `text` is the column's filter text, used by the text operators when the
    /// value is not itself text; without it, a number or date is matched by
    /// its plain form (`1234.5`, `2026-09-18`).
    #[must_use]
    pub fn matches(&self, value: &CellValue<'_>, text: Option<&str>) -> bool {
        match self.op {
            FilterOp::IsEmpty => is_empty(value, text),
            FilterOp::IsNotEmpty => !is_empty(value, text),
            FilterOp::Contains | FilterOp::StartsWith | FilterOp::EndsWith => {
                let Some(needle) = self.values.first().map(text_of_operand) else {
                    return true;
                };
                // The filter text wins over the value, as it always has for the
                // filter bar: it is what the column says it is searched by.
                let haystack: Cow<'_, str> = match (value, text) {
                    (_, Some(text)) => Cow::Borrowed(text),
                    (CellValue::Text(value), None) => Cow::Borrowed(value),
                    (CellValue::None, None) => return false,
                    (value, None) => Cow::Owned(plain_text(value)),
                };
                text_test(self.op, &haystack, &needle)
            }
            FilterOp::OneOf => self
                .values
                .iter()
                .any(|operand| equals(value, text, operand)),
            FilterOp::Equals => self
                .values
                .first()
                .is_none_or(|operand| equals(value, text, operand)),
            FilterOp::NotEquals => self
                .values
                .first()
                .is_none_or(|operand| !value.is_none() && !equals(value, text, operand)),
            FilterOp::Less => self.compare(value, 0, Ordering::is_lt),
            FilterOp::LessOrEqual => self.compare(value, 0, Ordering::is_le),
            FilterOp::Greater => self.compare(value, 0, Ordering::is_gt),
            FilterOp::GreaterOrEqual => self.compare(value, 0, Ordering::is_ge),
            FilterOp::Between => {
                let (Some(low), Some(high)) = (self.values.first(), self.values.get(1)) else {
                    return true;
                };
                let (Some(a), Some(b)) = (compare(value, low), compare(value, high)) else {
                    return false;
                };
                // Either order: `20..10` means the same range as `10..20`.
                (a.is_ge() && b.is_le()) || (a.is_le() && b.is_ge())
            }
        }
    }

    /// This condition with its operands read as `kind` once, instead of once
    /// per row. Operands that do not read as `kind` stay as they are, and then
    /// match nothing a comparison needs them for.
    #[must_use]
    pub fn prepared(&self, kind: ValueKind) -> Self {
        let text_op = matches!(
            self.op,
            FilterOp::Contains | FilterOp::StartsWith | FilterOp::EndsWith
        );
        if text_op {
            return self.clone();
        }
        let values = self
            .values
            .iter()
            .map(|value| value.coerce(kind).unwrap_or_else(|| value.clone()))
            .collect();
        Self::new(self.op, values)
    }

    fn compare(&self, value: &CellValue<'_>, index: usize, test: fn(Ordering) -> bool) -> bool {
        self.values
            .get(index)
            .is_none_or(|operand| compare(value, operand).is_some_and(test))
    }
}

/// The conditions on one column: all of them (AND) or any of them (OR).
///
/// A filter menu builds one of these; so does the filter bar, with a single
/// condition from [`Condition::from_text`]. No conditions means no filter.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct ColumnFilter {
    /// The conditions.
    pub conditions: Vec<Condition>,
    /// Whether a cell must pass any condition rather than all of them.
    pub any: bool,
}

impl ColumnFilter {
    /// A filter of one condition.
    #[must_use]
    pub fn new(condition: Condition) -> Self {
        Self {
            conditions: vec![condition],
            any: false,
        }
    }

    /// Adds a condition a cell must also pass.
    #[must_use]
    pub fn and(mut self, condition: Condition) -> Self {
        self.conditions.push(condition);
        self.any = false;
        self
    }

    /// Adds a condition a cell may pass instead.
    #[must_use]
    pub fn or(mut self, condition: Condition) -> Self {
        self.conditions.push(condition);
        self.any = true;
        self
    }

    /// Whether this filter lets every row through.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.conditions.is_empty()
    }

    /// What the filter bar's `text` means on a column of `kind`: the
    /// condition from [`Condition::from_text`], except that plain text is a
    /// substring test only on text columns and equality on every other kind —
    /// `30` on a number column means "equals 30", not "contains the digits 3
    /// and 0". `None` for empty text.
    ///
    /// This is exactly what a local grid does with
    /// [`GridState::column_filters`](crate::GridState::column_filters), so a
    /// server that reads [`GridQuery::column_filters`](crate::GridQuery) with it
    /// answers the same.
    #[must_use]
    pub fn from_bar_text(text: &str, kind: ValueKind) -> Option<Self> {
        let mut condition = Condition::from_text(text)?;
        if kind != ValueKind::Text && condition.op == FilterOp::Contains {
            condition.op = FilterOp::Equals;
            // Equality is on the value, not on what was typed around it.
            condition.values = condition
                .values
                .into_iter()
                .map(|value| match value {
                    FilterValue::Text(text) => FilterValue::Text(text.trim().to_owned()),
                    other => other,
                })
                .collect();
        }
        Some(Self::new(condition))
    }

    /// This filter with every operand read as `kind`. See
    /// [`Condition::prepared`].
    #[must_use]
    pub fn prepared(&self, kind: ValueKind) -> Self {
        Self {
            conditions: self
                .conditions
                .iter()
                .map(|condition| condition.prepared(kind))
                .collect(),
            any: self.any,
        }
    }

    /// Whether a cell passes this filter. See [`Condition::matches`].
    #[must_use]
    pub fn matches(&self, value: &CellValue<'_>, text: Option<&str>) -> bool {
        if self.any {
            self.conditions.is_empty()
                || self
                    .conditions
                    .iter()
                    .any(|condition| condition.matches(value, text))
        } else {
            self.conditions
                .iter()
                .all(|condition| condition.matches(value, text))
        }
    }
}

impl From<Condition> for ColumnFilter {
    fn from(condition: Condition) -> Self {
        Self::new(condition)
    }
}

fn is_empty(value: &CellValue<'_>, text: Option<&str>) -> bool {
    match value {
        CellValue::None => text.is_none_or(|text| text.trim().is_empty()),
        CellValue::Text(value) => value.trim().is_empty(),
        _ => false,
    }
}

/// How a value reads without a locale, for text operators on numbers and dates.
fn plain_text(value: &CellValue<'_>) -> String {
    match value {
        CellValue::None => String::new(),
        CellValue::Bool(value) => value.to_string(),
        CellValue::Int(value) => value.to_string(),
        CellValue::Float(value) => value.to_string(),
        #[cfg(feature = "chrono")]
        CellValue::Date(value) => value.to_string(),
        #[cfg(feature = "chrono")]
        CellValue::DateTime(value) => value.to_string(),
        CellValue::Text(value) => (*value).to_owned(),
    }
}

fn text_of_operand(operand: &FilterValue) -> String {
    match operand {
        FilterValue::Text(text) => text.clone(),
        other => plain_text(&other.as_cell()),
    }
}

fn text_test(op: FilterOp, haystack: &str, needle: &str) -> bool {
    match op {
        FilterOp::StartsWith => haystack.to_lowercase().starts_with(&needle.to_lowercase()),
        FilterOp::EndsWith => haystack.to_lowercase().ends_with(&needle.to_lowercase()),
        _ => contains_ignore_case(haystack, needle),
    }
}

/// Case-insensitive substring test.
///
/// Pure ASCII inputs — the overwhelming majority — take a non-allocating path.
/// Anything else falls back to full Unicode lowercasing, which has to allocate
/// because lowercasing can change a string's length.
pub(crate) fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }

    if haystack.is_ascii() && needle.is_ascii() {
        let needle = needle.as_bytes();
        return haystack
            .as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle));
    }

    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// Compares a cell with an operand of the same kind, reading a text operand
/// as the cell's kind. `None` when they cannot be compared: an empty cell, or
/// an operand that does not read as the cell's kind.
fn compare(value: &CellValue<'_>, operand: &FilterValue) -> Option<Ordering> {
    let kind = ValueKind::of(value)?;
    let operand = operand.coerce(kind)?;
    let ordering = value.cmp_with(&operand.as_cell(), TextCollation::CaseInsensitive);
    // Text compares ignoring case, but `cmp_with` breaks ties by case to stay
    // a total order; for a filter, `Berlin` equals `berlin`.
    if kind == ValueKind::Text {
        if let (CellValue::Text(a), FilterValue::Text(b)) = (value, &operand) {
            if a.to_lowercase() == b.to_lowercase() {
                return Some(Ordering::Equal);
            }
        }
    }
    Some(ordering)
}

fn equals(value: &CellValue<'_>, text: Option<&str>, operand: &FilterValue) -> bool {
    match compare(value, operand) {
        Some(ordering) => ordering.is_eq(),
        // A text operand against a value that is not text: fall back to the
        // column's filter text, so `=Berlin` works on any column with one.
        None => match (value, operand, text) {
            (CellValue::None, ..) => false,
            (_, FilterValue::Text(operand), Some(text)) => {
                text.trim().to_lowercase() == operand.trim().to_lowercase()
            }
            _ => false,
        },
    }
}
