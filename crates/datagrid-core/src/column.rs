//! Column identity and column specifications.

use crate::{
    CellAlign, CellFormat, CellOverflow, CellValue, GridLocale, GridState, TextCollation, ValueKind,
};
use std::borrow::Cow;
use std::fmt;
use std::rc::Rc;

/// A stable, human-readable column identifier.
///
/// Ids tie [`GridState`](crate::GridState) to columns. Because state can be
/// persisted and restored against a different column set, an id that no longer
/// resolves is ignored rather than treated as an error.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ColumnId(Cow<'static, str>);

impl ColumnId {
    /// Creates a column id.
    #[must_use]
    pub fn new(id: impl Into<Cow<'static, str>>) -> Self {
        Self(id.into())
    }

    /// Borrows the id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&'static str> for ColumnId {
    fn from(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }
}

impl From<String> for ColumnId {
    fn from(value: String) -> Self {
        Self(Cow::Owned(value))
    }
}

impl AsRef<str> for ColumnId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ColumnId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<str> for ColumnId {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for ColumnId {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

/// Reads a column's typed value from a row, borrowing from it where it can.
///
/// Higher-ranked over the row's lifetime so the returned
/// [`CellValue`] can point into the row instead of owning a copy of it.
///
/// [`Rc`] rather than `Arc` because a Dioxus `VirtualDom` is single-threaded.
pub type ValueFn<T> = Rc<dyn for<'a> Fn(&'a T) -> CellValue<'a>>;

/// The name [`ValueFn`] had while it was only used for sorting.
pub type SortKeyFn<T> = ValueFn<T>;

/// The narrowest a column can be resized to when it sets no
/// [`min_width`](ColumnSpec::min_width), in CSS pixels. Wide enough for a short
/// label and the resize handle.
pub const DEFAULT_MIN_COLUMN_WIDTH: f32 = 48.0;

/// Extracts the text a column filters and searches on.
pub type FilterTextFn<T> = Rc<dyn Fn(&T) -> String>;

/// How wide a column should be laid out.
///
/// The core crate only carries this value; turning it into CSS is the
/// renderer's job.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ColumnWidth {
    /// Size to content.
    #[default]
    Auto,
    /// A fixed width in CSS pixels.
    Px(f32),
    /// A share of the remaining free space.
    Fraction(f32),
}

/// Everything the core logic needs to know about one column.
///
/// The closures are [`Rc`] rather than `Arc` because a Dioxus `VirtualDom` is
/// single-threaded and WASM imposes no `Send` bound.
///
/// A column without a [`value`](ColumnSpec::value) cannot be sorted, and one
/// without a [`filter_text`](ColumnSpec::filter_text) takes part in neither
/// column filtering nor search.
pub struct ColumnSpec<T> {
    /// Stable identifier, referenced from [`GridState`](crate::GridState).
    pub id: ColumnId,
    /// Reads this column's typed value from a row. Sorting, formatting and
    /// everything else that needs to know what a cell *is* works from it.
    pub value: Option<ValueFn<T>>,
    /// Whether the column can be sorted by its [`value`](ColumnSpec::value).
    /// `true` by default; it has no effect without a value.
    pub sortable: bool,
    /// Extracts the text this column filters and searches on.
    pub filter_text: Option<FilterTextFn<T>>,
    /// Layout width.
    pub width: ColumnWidth,
    /// Lower bound for interactive resizing, in CSS pixels. `None` means
    /// [`DEFAULT_MIN_COLUMN_WIDTH`].
    pub min_width: Option<f32>,
    /// Whether the user may change this column's width. `true` by default.
    pub resizable: bool,
    /// Whether the column is shown by default.
    ///
    /// This is the column's own setting. Runtime visibility also depends on
    /// [`GridState::hidden_columns`](crate::GridState::hidden_columns); see
    /// [`ColumnSpec::is_visible`].
    pub visible: bool,
    /// How this column compares text when sorting.
    pub collation: TextCollation,
    /// How the value is turned into text.
    pub format: CellFormat,
    /// Horizontal alignment of the cell content. `None` derives it from the
    /// [`format`](ColumnSpec::format); see [`ColumnSpec::effective_align`].
    pub align: Option<CellAlign>,
    /// What happens to content wider than the column.
    pub overflow: CellOverflow,
    /// What kind of value the column holds. `None` lets
    /// [`ColumnSpec::value_kind`] find out from the rows.
    pub kind: Option<ValueKind>,
}

impl<T> ColumnSpec<T> {
    /// Creates a visible column with no sort key and no filter text.
    #[must_use]
    pub fn new(id: impl Into<ColumnId>) -> Self {
        Self {
            id: id.into(),
            value: None,
            sortable: true,
            filter_text: None,
            width: ColumnWidth::Auto,
            min_width: None,
            resizable: true,
            visible: true,
            collation: TextCollation::CaseInsensitive,
            format: CellFormat::Plain,
            align: None,
            overflow: CellOverflow::Truncate,
            kind: None,
        }
    }

    /// Sets how this column reads its typed value from a row.
    ///
    /// The value is what the column sorts by and what it formats for display,
    /// so a column with a value is sortable unless
    /// [`sortable(false)`](ColumnSpec::sortable) says otherwise. This is the
    /// general form: the closure may borrow text from the row. For the common
    /// cases prefer [`value_text`](ColumnSpec::value_text) or
    /// [`value_of`](ColumnSpec::value_of).
    ///
    /// ```
    /// # use datagrid_core::{CellValue, ColumnSpec};
    /// struct Task { title: String, done: bool }
    ///
    /// let column = ColumnSpec::new("status").value(|task: &Task| {
    ///     // A `&'static str` borrows for long enough to be returned here.
    ///     CellValue::Text(if task.done { "done" } else { "open" })
    /// });
    /// ```
    #[must_use]
    pub fn value<F>(mut self, value: F) -> Self
    where
        F: for<'a> Fn(&'a T) -> CellValue<'a> + 'static,
    {
        self.value = Some(Rc::new(value));
        self
    }

    /// Sets the column's value to text borrowed from the row.
    ///
    /// ```
    /// # use datagrid_core::ColumnSpec;
    /// struct User { name: String }
    ///
    /// let column = ColumnSpec::new("name").value_text(|user: &User| user.name.as_str());
    /// ```
    #[must_use]
    pub fn value_text<F>(self, text: F) -> Self
    where
        F: for<'a> Fn(&'a T) -> &'a str + 'static,
    {
        self.value(move |row| CellValue::Text(text(row)))
    }

    /// Sets the column's value to one that does not borrow from the row — a
    /// number, a boolean, a date with the `chrono` feature, or an [`Option`] of
    /// one.
    ///
    /// ```
    /// # use datagrid_core::ColumnSpec;
    /// struct User { age: u32, score: Option<f64> }
    ///
    /// let age = ColumnSpec::new("age").value_of(|user: &User| user.age);
    /// let score = ColumnSpec::new("score").value_of(|user: &User| user.score);
    /// ```
    #[must_use]
    pub fn value_of<V, F>(self, value: F) -> Self
    where
        F: Fn(&T) -> V + 'static,
        V: for<'a> Into<CellValue<'a>>,
    {
        self.value(move |row: &T| value(row).into())
    }

    /// Makes the column sortable by a value borrowed from the row.
    ///
    /// The same as [`value`](ColumnSpec::value), under the name from before
    /// columns had a typed value.
    #[must_use]
    pub fn sort_by<F>(self, key: F) -> Self
    where
        F: for<'a> Fn(&'a T) -> CellValue<'a> + 'static,
    {
        self.value(key).sortable(true)
    }

    /// Makes the column sortable by text borrowed from the row.
    ///
    /// The same as [`value_text`](ColumnSpec::value_text).
    #[must_use]
    pub fn sort_by_text<F>(self, key: F) -> Self
    where
        F: for<'a> Fn(&'a T) -> &'a str + 'static,
    {
        self.value_text(key).sortable(true)
    }

    /// Makes the column sortable by a value that does not borrow from the row.
    ///
    /// The same as [`value_of`](ColumnSpec::value_of).
    #[must_use]
    pub fn sort_by_value<V, F>(self, key: F) -> Self
    where
        F: Fn(&T) -> V + 'static,
        V: for<'a> Into<CellValue<'a>>,
    {
        self.value_of(key).sortable(true)
    }

    /// Sets whether the column can be sorted by its value.
    #[must_use]
    pub const fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Sets how the value is turned into text.
    #[must_use]
    pub fn format(mut self, format: CellFormat) -> Self {
        self.format = format;
        self
    }

    /// Sets the horizontal alignment, overriding the one derived from the
    /// format.
    #[must_use]
    pub const fn align(mut self, align: CellAlign) -> Self {
        self.align = Some(align);
        self
    }

    /// Sets what happens to content wider than the column.
    #[must_use]
    pub const fn overflow(mut self, overflow: CellOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    /// Reads this column's value from a row, or [`CellValue::None`] if the
    /// column has no value.
    #[must_use]
    pub fn read<'a>(&self, row: &'a T) -> CellValue<'a> {
        self.value
            .as_ref()
            .map_or(CellValue::None, |value| value(row))
    }

    /// Formats this column's value in a row as text, using the column's
    /// [`format`](ColumnSpec::format) and the given locale. `None` for a column
    /// without a value.
    #[must_use]
    pub fn display_text(&self, row: &T, locale: &GridLocale) -> Option<String> {
        let value = self.value.as_ref()?;
        Some(locale.format(&value(row), &self.format))
    }

    /// The alignment to lay the cell out with: the column's own
    /// [`align`](ColumnSpec::align) if set, otherwise
    /// [`CellFormat::default_align`].
    #[must_use]
    pub fn effective_align(&self) -> CellAlign {
        self.align.unwrap_or_else(|| self.format.default_align())
    }

    /// Makes the column filterable and searchable by the given text.
    #[must_use]
    pub fn filter_by(mut self, text: impl Fn(&T) -> String + 'static) -> Self {
        self.filter_text = Some(Rc::new(text));
        self
    }

    /// Sets the layout width.
    #[must_use]
    pub const fn width(mut self, width: ColumnWidth) -> Self {
        self.width = width;
        self
    }

    /// Sets the minimum width honoured while resizing.
    #[must_use]
    pub const fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = Some(min_width);
        self
    }

    /// Sets whether the user may change this column's width.
    #[must_use]
    pub const fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Sets the text collation used when sorting this column.
    #[must_use]
    pub const fn collation(mut self, collation: TextCollation) -> Self {
        self.collation = collation;
        self
    }

    /// Hides the column by default.
    #[must_use]
    pub const fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }

    /// Whether the column is visible given the current runtime state.
    ///
    /// A column is visible when its own [`visible`](ColumnSpec::visible) flag is
    /// set *and* it is not listed in
    /// [`GridState::hidden_columns`](crate::GridState::hidden_columns).
    #[must_use]
    pub fn is_visible(&self, hidden_columns: &[ColumnId]) -> bool {
        self.visible && !hidden_columns.contains(&self.id)
    }

    /// The narrowest this column may be resized to, in CSS pixels.
    ///
    /// [`min_width`](ColumnSpec::min_width) if it is set to a usable value,
    /// otherwise [`DEFAULT_MIN_COLUMN_WIDTH`].
    #[must_use]
    pub fn resize_min_width(&self) -> f32 {
        match self.min_width {
            Some(min) if min.is_finite() && min >= 0.0 => min,
            _ => DEFAULT_MIN_COLUMN_WIDTH,
        }
    }

    /// Clamps a proposed width to what this column allows.
    ///
    /// Never narrower than [`resize_min_width`](ColumnSpec::resize_min_width).
    /// A non-finite width, such as one computed from a missing measurement,
    /// yields the minimum rather than an unusable layout.
    #[must_use]
    pub fn clamp_width(&self, width: f32) -> f32 {
        let min = self.resize_min_width();
        if width.is_finite() {
            width.max(min)
        } else {
            min
        }
    }

    /// The width to lay this column out with, given the runtime state.
    ///
    /// A width chosen by resizing, recorded in
    /// [`GridState::column_widths`](crate::GridState::column_widths), wins over
    /// the column's own [`width`](ColumnSpec::width) and is clamped to the
    /// minimum. State restored from storage can therefore never make a column
    /// narrower than its definition allows.
    #[must_use]
    pub fn effective_width(&self, state: &GridState) -> ColumnWidth {
        state
            .column_width(&self.id)
            .map_or(self.width, |width| ColumnWidth::Px(self.clamp_width(width)))
    }

    /// Whether this column can be filtered: by its
    /// [`filter_text`](ColumnSpec::filter_text), its [`value`](ColumnSpec::value)
    /// or both.
    #[must_use]
    pub const fn is_filterable(&self) -> bool {
        self.filter_text.is_some() || self.value.is_some()
    }

    /// Whether the global search looks at this column: only by its
    /// [`filter_text`](ColumnSpec::filter_text).
    #[must_use]
    pub const fn is_searchable(&self) -> bool {
        self.filter_text.is_some()
    }

    /// Declares what kind of value the column holds, instead of letting
    /// [`value_kind`](ColumnSpec::value_kind) find out from the rows. Worth it
    /// for a column that is often empty, or a remote grid before its first page.
    #[must_use]
    pub const fn kind(mut self, kind: ValueKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// What kind of value the column holds: the declared
    /// [`kind`](ColumnSpec::kind), otherwise the kind of the first value in
    /// `rows` that is not empty, otherwise text if it has filter text.
    #[must_use]
    pub fn value_kind(&self, rows: &[T]) -> Option<ValueKind> {
        self.kind
            .or_else(|| {
                let value = self.value.as_ref()?;
                rows.iter().find_map(|row| ValueKind::of(&value(row)))
            })
            .or_else(|| self.filter_text.as_ref().map(|_| ValueKind::Text))
    }

    /// Whether this column can be sorted.
    #[must_use]
    pub const fn is_sortable(&self) -> bool {
        self.sortable && self.value.is_some()
    }
}

impl<T> Clone for ColumnSpec<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            value: self.value.clone(),
            sortable: self.sortable,
            filter_text: self.filter_text.clone(),
            width: self.width,
            min_width: self.min_width,
            resizable: self.resizable,
            visible: self.visible,
            collation: self.collation,
            format: self.format.clone(),
            align: self.align,
            overflow: self.overflow,
            kind: self.kind,
        }
    }
}

impl<T> fmt::Debug for ColumnSpec<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ColumnSpec")
            .field("id", &self.id)
            .field("sortable", &self.is_sortable())
            .field("filterable", &self.is_filterable())
            .field("width", &self.width)
            .field("min_width", &self.min_width)
            .field("resizable", &self.resizable)
            .field("visible", &self.visible)
            .field("collation", &self.collation)
            .field("format", &self.format)
            .field("align", &self.align)
            .field("overflow", &self.overflow)
            .field("kind", &self.kind)
            .finish()
    }
}

/// Compares the identity of two specs, not the behaviour of their closures.
///
/// Two specs are equal when their ids and plain fields match and their closures
/// are literally the same allocation. That is what a UI framework needs to
/// decide whether a column list changed.
impl<T> PartialEq for ColumnSpec<T> {
    fn eq(&self, other: &Self) -> bool {
        fn same_closure<F: ?Sized>(a: Option<&Rc<F>>, b: Option<&Rc<F>>) -> bool {
            match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                _ => false,
            }
        }

        self.id == other.id
            && self.width == other.width
            && self.min_width == other.min_width
            && self.resizable == other.resizable
            && self.visible == other.visible
            && self.collation == other.collation
            && self.sortable == other.sortable
            && self.format == other.format
            && self.align == other.align
            && self.overflow == other.overflow
            && self.kind == other.kind
            && same_closure(self.value.as_ref(), other.value.as_ref())
            && same_closure(self.filter_text.as_ref(), other.filter_text.as_ref())
    }
}
