//! Column identity and column specifications.

use crate::{GridState, SortValue, TextCollation};
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

/// Extracts the value a column sorts by, borrowed from the row.
///
/// Higher-ranked over the row's lifetime so the returned
/// [`SortValue`] can point into the row instead of owning a copy of it.
///
/// [`Rc`] rather than `Arc` because a Dioxus `VirtualDom` is single-threaded.
pub type SortKeyFn<T> = Rc<dyn for<'a> Fn(&'a T) -> SortValue<'a>>;

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
/// A column without a [`sort_key`](ColumnSpec::sort_key) cannot be sorted, and
/// one without a [`filter_text`](ColumnSpec::filter_text) takes part in neither
/// column filtering nor search.
pub struct ColumnSpec<T> {
    /// Stable identifier, referenced from [`GridState`](crate::GridState).
    pub id: ColumnId,
    /// Extracts the value this column sorts by.
    pub sort_key: Option<SortKeyFn<T>>,
    /// Extracts the text this column filters and searches on.
    pub filter_text: Option<FilterTextFn<T>>,
    /// Layout width.
    pub width: ColumnWidth,
    /// Lower bound for interactive resizing, in CSS pixels. `None` means
    /// [`DEFAULT_MIN_COLUMN_WIDTH`].
    pub min_width: Option<f32>,
    /// Whether the column is shown by default.
    ///
    /// This is the column's own setting. Runtime visibility also depends on
    /// [`GridState::hidden_columns`](crate::GridState::hidden_columns); see
    /// [`ColumnSpec::is_visible`].
    pub visible: bool,
    /// How this column compares text when sorting.
    pub collation: TextCollation,
}

impl<T> ColumnSpec<T> {
    /// Creates a visible column with no sort key and no filter text.
    #[must_use]
    pub fn new(id: impl Into<ColumnId>) -> Self {
        Self {
            id: id.into(),
            sort_key: None,
            filter_text: None,
            width: ColumnWidth::Auto,
            min_width: None,
            visible: true,
            collation: TextCollation::CaseInsensitive,
        }
    }

    /// Makes the column sortable by a key borrowed from the row.
    ///
    /// This is the general form. For the two common cases prefer
    /// [`sort_by_text`](ColumnSpec::sort_by_text), which takes a plain `&str`,
    /// or [`sort_by_value`](ColumnSpec::sort_by_value), which takes anything
    /// that does not borrow.
    ///
    /// ```
    /// # use datagrid_core::{ColumnSpec, SortValue};
    /// struct Task { title: String, done: bool }
    ///
    /// let column = ColumnSpec::new("status").sort_by(|task: &Task| {
    ///     // A `&'static str` borrows for long enough to be returned here.
    ///     SortValue::Text(if task.done { "done" } else { "open" })
    /// });
    /// ```
    #[must_use]
    pub fn sort_by<F>(mut self, key: F) -> Self
    where
        F: for<'a> Fn(&'a T) -> SortValue<'a> + 'static,
    {
        self.sort_key = Some(Rc::new(key));
        self
    }

    /// Makes the column sortable by text borrowed from the row.
    ///
    /// ```
    /// # use datagrid_core::ColumnSpec;
    /// struct User { name: String }
    ///
    /// let column = ColumnSpec::new("name").sort_by_text(|user: &User| user.name.as_str());
    /// ```
    #[must_use]
    pub fn sort_by_text<F>(mut self, key: F) -> Self
    where
        F: for<'a> Fn(&'a T) -> &'a str + 'static,
    {
        self.sort_key = Some(Rc::new(move |row| SortValue::Text(key(row))));
        self
    }

    /// Makes the column sortable by a value that does not borrow from the row —
    /// a number, a boolean, or an [`Option`] of one.
    ///
    /// ```
    /// # use datagrid_core::ColumnSpec;
    /// struct User { age: u32, score: Option<f64> }
    ///
    /// let age = ColumnSpec::new("age").sort_by_value(|user: &User| user.age);
    /// let score = ColumnSpec::new("score").sort_by_value(|user: &User| user.score);
    /// ```
    #[must_use]
    pub fn sort_by_value<V, F>(mut self, key: F) -> Self
    where
        F: Fn(&T) -> V + 'static,
        V: for<'a> Into<SortValue<'a>>,
    {
        self.sort_key = Some(Rc::new(move |row: &T| key(row).into()));
        self
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

    /// Whether this column can take part in filtering and search.
    #[must_use]
    pub const fn is_filterable(&self) -> bool {
        self.filter_text.is_some()
    }

    /// Whether this column can be sorted.
    #[must_use]
    pub const fn is_sortable(&self) -> bool {
        self.sort_key.is_some()
    }
}

impl<T> Clone for ColumnSpec<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            sort_key: self.sort_key.clone(),
            filter_text: self.filter_text.clone(),
            width: self.width,
            min_width: self.min_width,
            visible: self.visible,
            collation: self.collation,
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
            .field("visible", &self.visible)
            .field("collation", &self.collation)
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
            && self.visible == other.visible
            && self.collation == other.collation
            && same_closure(self.sort_key.as_ref(), other.sort_key.as_ref())
            && same_closure(self.filter_text.as_ref(), other.filter_text.as_ref())
    }
}
