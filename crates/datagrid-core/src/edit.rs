//! Editing: reading what a user typed as a column's kind of value, writing it
//! into a row, validating the result, and keeping track of changes that have
//! not been saved yet.
//!
//! The grid never owns the rows. An edit works on a copy of a row, and the
//! application decides what saving means; see [`Changes`] for edits collected
//! into one batch.

use crate::{ColumnId, ColumnSpec, GridLocale, GridRow, Value, ValueKind};
use std::rc::Rc;

/// Writes an edited value into a row. `None` is an empty cell.
///
/// [`Rc`] rather than `Arc` because a Dioxus `VirtualDom` is single-threaded.
pub type SetFn<T> = Rc<dyn Fn(&mut T, Option<Value>) -> Result<(), EditError>>;

/// Checks a row after an edit, returning a message for the user if it is not
/// acceptable.
pub type ValidateFn<T> = Rc<dyn Fn(&T) -> Result<(), String>>;

/// Why an edit was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
    /// The cell may not be empty.
    Required,
    /// The text could not be read as this kind of value.
    Invalid(ValueKind),
    /// The value is not one of the column's [choices](ColumnSpec::choices).
    NotAChoice,
    /// Refused by a validator, with its message.
    Message(String),
}

impl EditError {
    /// The message to show, in the locale's language.
    #[must_use]
    pub fn message(&self, locale: &GridLocale) -> String {
        match self {
            Self::Required => locale.edit_required.to_string(),
            Self::Invalid(ValueKind::Number) => locale.edit_invalid_number.to_string(),
            Self::Invalid(ValueKind::Date | ValueKind::DateTime) => {
                locale.edit_invalid_date.to_string()
            }
            Self::Invalid(ValueKind::Bool | ValueKind::Text) => locale.edit_invalid.to_string(),
            Self::NotAChoice => locale.edit_not_a_choice.to_string(),
            Self::Message(message) => message.clone(),
        }
    }
}

impl From<String> for EditError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for EditError {
    fn from(message: &str) -> Self {
        Self::Message(message.to_owned())
    }
}

/// Turns an edited value into the type of a row's field, for
/// [`ColumnSpec::editable`].
///
/// Implemented for `String`, `bool`, the integer and float types, `chrono`'s
/// `NaiveDate` and `NaiveDateTime` with the `chrono` feature, and an
/// [`Option`] of any of them, which is the only one that accepts an empty cell.
pub trait FromValue: Sized {
    /// Converts `value`; `None` is an empty cell.
    ///
    /// # Errors
    ///
    /// [`EditError::Required`] for an empty cell where the type has no empty
    /// value, [`EditError::Invalid`] for a value of the wrong kind or out of
    /// range.
    fn from_value(value: Option<Value>) -> Result<Self, EditError>;
}

impl FromValue for String {
    /// Empty text for an empty cell.
    fn from_value(value: Option<Value>) -> Result<Self, EditError> {
        Ok(match value {
            None => Self::new(),
            Some(Value::Text(text)) => text,
            Some(other) => other.edit_text(),
        })
    }
}

impl FromValue for bool {
    fn from_value(value: Option<Value>) -> Result<Self, EditError> {
        match value.map(|value| value.coerce(ValueKind::Bool)) {
            None => Err(EditError::Required),
            Some(Some(Value::Bool(value))) => Ok(value),
            Some(_) => Err(EditError::Invalid(ValueKind::Bool)),
        }
    }
}

/// Reads a number, for the numeric [`FromValue`] implementations.
fn number(value: Option<Value>) -> Result<Value, EditError> {
    let value = value.ok_or(EditError::Required)?;
    value
        .coerce(ValueKind::Number)
        .ok_or(EditError::Invalid(ValueKind::Number))
}

impl FromValue for f64 {
    fn from_value(value: Option<Value>) -> Result<Self, EditError> {
        match number(value)? {
            #[allow(clippy::cast_precision_loss)]
            Value::Int(value) => Ok(value as Self),
            Value::Float(value) if value.is_finite() => Ok(value),
            _ => Err(EditError::Invalid(ValueKind::Number)),
        }
    }
}

impl FromValue for f32 {
    fn from_value(value: Option<Value>) -> Result<Self, EditError> {
        let wide = f64::from_value(value)?;
        #[allow(clippy::cast_possible_truncation)]
        let narrow = wide as Self;
        if narrow.is_finite() {
            Ok(narrow)
        } else {
            Err(EditError::Invalid(ValueKind::Number))
        }
    }
}

impl FromValue for i64 {
    /// Accepts a float only if it is a whole number, so `2.5` is refused
    /// rather than cut to `2`.
    fn from_value(value: Option<Value>) -> Result<Self, EditError> {
        match number(value)? {
            Value::Int(value) => Ok(value),
            #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
            Value::Float(value) if value.fract() == 0.0 && value.abs() < i64::MAX as f64 => {
                Ok(value as Self)
            }
            _ => Err(EditError::Invalid(ValueKind::Number)),
        }
    }
}

macro_rules! integer_from_value {
    ($($ty:ty),*) => {$(
        impl FromValue for $ty {
            /// Refuses values out of the type's range, such as a negative
            /// number for an unsigned type.
            fn from_value(value: Option<Value>) -> Result<Self, EditError> {
                Self::try_from(i64::from_value(value)?)
                    .map_err(|_| EditError::Invalid(ValueKind::Number))
            }
        }
    )*};
}

integer_from_value!(i8, i16, i32, isize, u8, u16, u32, u64, usize);

#[cfg(feature = "chrono")]
impl FromValue for chrono::NaiveDate {
    fn from_value(value: Option<Value>) -> Result<Self, EditError> {
        match value.map(|value| value.coerce(ValueKind::Date)) {
            None => Err(EditError::Required),
            Some(Some(Value::Date(date))) => Ok(date),
            Some(_) => Err(EditError::Invalid(ValueKind::Date)),
        }
    }
}

#[cfg(feature = "chrono")]
impl FromValue for chrono::NaiveDateTime {
    fn from_value(value: Option<Value>) -> Result<Self, EditError> {
        match value.map(|value| value.coerce(ValueKind::DateTime)) {
            None => Err(EditError::Required),
            Some(Some(Value::DateTime(date_time))) => Ok(date_time),
            Some(_) => Err(EditError::Invalid(ValueKind::DateTime)),
        }
    }
}

impl<V: FromValue> FromValue for Option<V> {
    /// `None` for an empty cell.
    fn from_value(value: Option<Value>) -> Result<Self, EditError> {
        match value {
            None => Ok(None),
            Some(value) => V::from_value(Some(value)).map(Some),
        }
    }
}

impl<T> ColumnSpec<T> {
    /// Makes the column editable through a setter that takes the edited value
    /// as the field's own type, converted with [`FromValue`].
    ///
    /// ```
    /// # use datagrid_core::ColumnSpec;
    /// struct User { name: String, age: u32, score: Option<f64> }
    ///
    /// let name = ColumnSpec::new("name").editable(|user: &mut User, name: String| user.name = name);
    /// // A negative age is refused before the setter runs.
    /// let age = ColumnSpec::new("age").editable(|user: &mut User, age: u32| user.age = age);
    /// // Only an `Option` accepts an empty cell.
    /// let score = ColumnSpec::new("score").editable(|user: &mut User, score: Option<f64>| user.score = score);
    /// ```
    #[must_use]
    pub fn editable<V, F>(self, set: F) -> Self
    where
        V: FromValue,
        F: Fn(&mut T, V) + 'static,
    {
        self.edit(move |row: &mut T, value| {
            set(row, V::from_value(value)?);
            Ok(())
        })
    }

    /// Makes the column editable through a setter that takes the edited
    /// [`Value`] as it is, and may refuse it. The general form of
    /// [`editable`](ColumnSpec::editable).
    #[must_use]
    pub fn edit<F>(mut self, set: F) -> Self
    where
        F: Fn(&mut T, Option<Value>) -> Result<(), EditError> + 'static,
    {
        self.set = Some(Rc::new(set));
        self
    }

    /// Checks the row after this column was edited. The message is shown at
    /// the cell.
    ///
    /// ```
    /// # use datagrid_core::ColumnSpec;
    /// struct User { name: String }
    ///
    /// let name = ColumnSpec::new("name")
    ///     .editable(|user: &mut User, name: String| user.name = name)
    ///     .validate(|user: &User| {
    ///         if user.name.trim().is_empty() { Err("A name is required".into()) } else { Ok(()) }
    ///     });
    /// ```
    #[must_use]
    pub fn validate<F>(mut self, validate: F) -> Self
    where
        F: Fn(&T) -> Result<(), String> + 'static,
    {
        self.validate = Some(Rc::new(validate));
        self
    }

    /// Limits the column to these values, offered as a list to choose from.
    #[must_use]
    pub fn choices(mut self, choices: impl IntoIterator<Item = impl Into<Value>>) -> Self {
        self.choices = choices.into_iter().map(Into::into).collect();
        self
    }

    /// Whether the column can be edited.
    #[must_use]
    pub const fn is_editable(&self) -> bool {
        self.set.is_some()
    }

    /// The text an editor for this column starts with: the row's value, in
    /// the form [`apply_edit`](ColumnSpec::apply_edit) reads back. Numbers use
    /// the locale's decimal separator.
    #[must_use]
    pub fn edit_text(&self, row: &T, locale: &GridLocale) -> String {
        let Some(value) = Value::from_cell(&self.read(row)) else {
            return String::new();
        };
        match value {
            Value::Float(_) if locale.decimal_separator != '.' => value
                .edit_text()
                .replace('.', locale.decimal_separator.encode_utf8(&mut [0; 4])),
            _ => value.edit_text(),
        }
    }

    /// Reads `text` as this column's kind of value and writes it into `row`,
    /// then runs the column's [validator](ColumnSpec::validate).
    ///
    /// Blank text is an empty cell. Text columns keep what was typed,
    /// including spaces at either end. `row` may be partly changed when an
    /// error is returned, so edit a copy.
    ///
    /// # Errors
    ///
    /// Whatever the text cannot be read as, the setter refuses or the
    /// validator reports; [`EditError::Message`] with no text for a column that
    /// is not editable.
    pub fn apply_edit(&self, row: &mut T, text: &str, kind: ValueKind) -> Result<(), EditError> {
        let Some(set) = &self.set else {
            return Err(EditError::Message(String::new()));
        };
        let value = if text.trim().is_empty() {
            None
        } else if kind == ValueKind::Text {
            Some(Value::Text(text.to_owned()))
        } else {
            Some(Value::parse(kind, text).ok_or(EditError::Invalid(kind))?)
        };
        if let Some(value) = &value {
            if !self.choices.is_empty() && !self.choices.contains(value) {
                return Err(EditError::NotAChoice);
            }
        }
        set(row, value)?;
        if let Some(validate) = &self.validate {
            validate(row).map_err(EditError::Message)?;
        }
        Ok(())
    }
}

/// The columns whose value differs between two versions of a row.
///
/// Columns without a [value](ColumnSpec::value) are never reported: there is
/// nothing to compare.
#[must_use]
pub fn changed_columns<T>(columns: &[ColumnSpec<T>], before: &T, after: &T) -> Vec<ColumnId> {
    columns
        .iter()
        .filter(|column| {
            column.value.is_some() && !same_value(&column.read(before), &column.read(after))
        })
        .map(|column| column.id.clone())
        .collect()
}

/// Equality for change tracking: floats by their bits, so `NaN` is unchanged.
fn same_value(a: &crate::CellValue<'_>, b: &crate::CellValue<'_>) -> bool {
    match (a, b) {
        (crate::CellValue::Float(a), crate::CellValue::Float(b)) => a.to_bits() == b.to_bits(),
        _ => a == b,
    }
}

/// Edits collected but not saved yet: the rows changed, added and deleted, as
/// a batch edit keeps them until the user saves or discards.
///
/// Each row appears at most once. Editing a row twice keeps its first
/// original; editing it back to how it was drops it; deleting a row that was
/// added drops it altogether.
#[derive(Clone, Debug, PartialEq)]
pub struct Changes<T> {
    /// Changed rows, as `(original, current)`.
    pub updated: Vec<(T, T)>,
    /// New rows.
    pub added: Vec<T>,
    /// Deleted rows, as they were before any change.
    pub deleted: Vec<T>,
}

impl<T> Default for Changes<T> {
    fn default() -> Self {
        Self {
            updated: Vec::new(),
            added: Vec::new(),
            deleted: Vec::new(),
        }
    }
}

impl<T: GridRow + PartialEq> Changes<T> {
    /// No changes.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether there is nothing to save.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.updated.is_empty() && self.added.is_empty() && self.deleted.is_empty()
    }

    /// How many rows are changed, added or deleted.
    #[must_use]
    pub fn len(&self) -> usize {
        self.updated.len() + self.added.len() + self.deleted.len()
    }

    /// Records that `original` now reads `current`.
    pub fn update(&mut self, original: T, current: T) {
        let key = original.key();
        if let Some(added) = self.added.iter_mut().find(|row| row.key() == key) {
            *added = current;
            return;
        }
        if let Some(entry) = self
            .updated
            .iter_mut()
            .find(|(first, _)| first.key() == key)
        {
            entry.1 = current;
        } else if original != current {
            self.updated.push((original, current));
        }
        // Edited back to how it was: nothing left to save.
        self.updated.retain(|(first, now)| first != now);
    }

    /// Records a new row.
    pub fn add(&mut self, row: T) {
        self.added.push(row);
    }

    /// Records that `row` is deleted.
    pub fn delete(&mut self, row: T) {
        let key = row.key();
        if let Some(index) = self.added.iter().position(|added| added.key() == key) {
            self.added.remove(index);
            return;
        }
        let original = match self
            .updated
            .iter()
            .position(|(first, _)| first.key() == key)
        {
            Some(index) => self.updated.remove(index).0,
            None => row,
        };
        if !self.is_deleted(&key) {
            self.deleted.push(original);
        }
    }

    /// Takes back a deletion.
    pub fn restore(&mut self, key: &T::Key) {
        self.deleted.retain(|row| row.key() != *key);
    }

    /// The row as edited, if it was changed or added.
    #[must_use]
    pub fn current(&self, key: &T::Key) -> Option<&T> {
        self.updated
            .iter()
            .find(|(original, _)| original.key() == *key)
            .map(|(_, current)| current)
            .or_else(|| self.added.iter().find(|row| row.key() == *key))
    }

    /// The row before it was changed, if it was.
    #[must_use]
    pub fn original(&self, key: &T::Key) -> Option<&T> {
        self.updated
            .iter()
            .find(|(original, _)| original.key() == *key)
            .map(|(original, _)| original)
    }

    /// Whether the row is marked deleted.
    #[must_use]
    pub fn is_deleted(&self, key: &T::Key) -> bool {
        self.deleted.iter().any(|row| row.key() == *key)
    }

    /// The columns changed in the row with `key`, if it was changed.
    #[must_use]
    pub fn changed_columns(&self, columns: &[ColumnSpec<T>], key: &T::Key) -> Vec<ColumnId> {
        self.updated
            .iter()
            .find(|(original, _)| original.key() == *key)
            .map(|(original, current)| changed_columns(columns, original, current))
            .unwrap_or_default()
    }
}
