//! Column definitions that pair core sorting and filtering with rendering.

use crate::CellEditor;
use datagrid_core::{
    CellAlign, CellFormat, CellOverflow, CellValue, ColumnId, ColumnSpec, ColumnWidth, EditError,
    FromValue, GridLocale, SortValue, TextCollation, Value, ValueKind,
};
use dioxus::prelude::*;
use std::fmt;
use std::rc::Rc;

/// Renders one cell of a column.
pub type CellRenderer<T> = Rc<dyn Fn(&T) -> Element>;

/// Renders a column's header content.
pub type HeaderRenderer = Rc<dyn Fn() -> Element>;

/// Renders a column's own editor; see [`Column::editor`].
pub type EditorRenderer = Rc<dyn Fn(CellEditor) -> Element>;

/// A column: what the core needs to sort and filter it, plus how to draw it.
///
/// Built fluently. Only an id and a label are required. A column with a
/// [`value`](Column::value_of) is sortable and, unless it has its own
/// [`cell`](Column::cell) renderer, shows that value formatted by its
/// [`format`](Column::format) and the grid's locale. One with no
/// [`filter_by`](Column::filter_by) takes part in neither filtering nor search.
///
/// ```
/// use dioxus::prelude::*;
/// use dioxus_datagrid::{CellFormat, Column};
///
/// #[derive(Clone, PartialEq)]
/// struct User {
///     name: String,
///     age: u32,
/// }
///
/// let columns = vec![
///     Column::new("name", "Name")
///         .cell(|user: &User| rsx! { "{user.name}" })
///         .sort_by_text(|user: &User| user.name.as_str())
///         .filter_by(|user: &User| user.name.clone()),
///     Column::new("age", "Age")
///         .value_of(|user: &User| user.age)
///         .format(CellFormat::number(0)),
/// ];
/// # let _ = columns;
/// ```
pub struct Column<T> {
    spec: ColumnSpec<T>,
    label: String,
    cell: Option<CellRenderer<T>>,
    header: Option<HeaderRenderer>,
    editor: Option<EditorRenderer>,
}

impl<T> Column<T> {
    /// Creates a column with a stable id and a human-readable label.
    ///
    /// The id ties [`GridState`](datagrid_core::GridState) to this column and
    /// should stay stable across releases; the label is what the header shows.
    #[must_use]
    pub fn new(id: impl Into<ColumnId>, label: impl Into<String>) -> Self {
        Self {
            spec: ColumnSpec::new(id),
            label: label.into(),
            cell: None,
            header: None,
            editor: None,
        }
    }

    /// Sets how a cell of this column is rendered.
    ///
    /// Without this the column shows its formatted [value](Column::value_of),
    /// or nothing if it has none.
    #[must_use]
    pub fn cell(mut self, render: impl Fn(&T) -> Element + 'static) -> Self {
        self.cell = Some(Rc::new(render));
        self
    }

    /// Replaces the header content, which defaults to the label.
    ///
    /// The sort affordance stays on the surrounding header cell, so this only
    /// controls what sits inside it.
    #[must_use]
    pub fn header(mut self, render: impl Fn() -> Element + 'static) -> Self {
        self.header = Some(Rc::new(render));
        self
    }

    /// Sets how this column reads its typed value from a row, which it sorts
    /// by and, without a [`cell`](Column::cell) renderer, shows. The closure
    /// may borrow text from the row.
    #[must_use]
    pub fn value<F>(mut self, value: F) -> Self
    where
        F: for<'a> Fn(&'a T) -> CellValue<'a> + 'static,
    {
        self.spec = self.spec.value(value);
        self
    }

    /// Sets the column's value to text borrowed from the row.
    #[must_use]
    pub fn value_text<F>(mut self, text: F) -> Self
    where
        F: for<'a> Fn(&'a T) -> &'a str + 'static,
    {
        self.spec = self.spec.value_text(text);
        self
    }

    /// Sets the column's value to one that does not borrow from the row: a
    /// number, a boolean, a date with the `chrono` feature, or an [`Option`] of
    /// one.
    #[must_use]
    pub fn value_of<V, F>(mut self, value: F) -> Self
    where
        F: Fn(&T) -> V + 'static,
        V: for<'a> Into<CellValue<'a>>,
    {
        self.spec = self.spec.value_of(value);
        self
    }

    /// Sets whether the column can be sorted by its value.
    #[must_use]
    pub fn sortable(mut self, sortable: bool) -> Self {
        self.spec = self.spec.sortable(sortable);
        self
    }

    /// Sets how the value is turned into text, such as
    /// [`CellFormat::currency`]. Numeric formats also align the column at the
    /// end.
    #[must_use]
    pub fn format(mut self, format: CellFormat) -> Self {
        self.spec = self.spec.format(format);
        self
    }

    /// Sets the horizontal alignment, overriding the one from the format.
    #[must_use]
    pub fn align(mut self, align: CellAlign) -> Self {
        self.spec = self.spec.align(align);
        self
    }

    /// Sets what happens to content wider than the column.
    #[must_use]
    pub fn overflow(mut self, overflow: CellOverflow) -> Self {
        self.spec = self.spec.overflow(overflow);
        self
    }

    /// Declares what kind of value the column holds, instead of reading it
    /// from the rows: for a remote grid before its first page, or a column
    /// that is often empty. It decides the operators a filter menu offers.
    #[must_use]
    pub fn kind(mut self, kind: ValueKind) -> Self {
        self.spec = self.spec.kind(kind);
        self
    }

    /// Makes the column sortable by text borrowed from the row.
    #[must_use]
    pub fn sort_by_text<F>(mut self, key: F) -> Self
    where
        F: for<'a> Fn(&'a T) -> &'a str + 'static,
    {
        self.spec = self.spec.sort_by_text(key);
        self
    }

    /// Makes the column sortable by a value that does not borrow from the row.
    #[must_use]
    pub fn sort_by_value<V, F>(mut self, key: F) -> Self
    where
        F: Fn(&T) -> V + 'static,
        V: for<'a> Into<SortValue<'a>>,
    {
        self.spec = self.spec.sort_by_value(key);
        self
    }

    /// Makes the column sortable by an explicit [`SortValue`].
    #[must_use]
    pub fn sort_by<F>(mut self, key: F) -> Self
    where
        F: for<'a> Fn(&'a T) -> SortValue<'a> + 'static,
    {
        self.spec = self.spec.sort_by(key);
        self
    }

    /// Makes the column filterable and searchable by the given text.
    #[must_use]
    pub fn filter_by(mut self, text: impl Fn(&T) -> String + 'static) -> Self {
        self.spec = self.spec.filter_by(text);
        self
    }

    /// Sets the layout width.
    #[must_use]
    pub fn width(mut self, width: ColumnWidth) -> Self {
        self.spec = self.spec.width(width);
        self
    }

    /// Sets the minimum width honoured while resizing, in CSS pixels.
    #[must_use]
    pub fn min_width(mut self, min_width: f32) -> Self {
        self.spec = self.spec.min_width(min_width);
        self
    }

    /// Sets whether the user may change this column's width. Columns are
    /// resizable by default, wherever the header renders resize handles.
    #[must_use]
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.spec = self.spec.resizable(resizable);
        self
    }

    /// Sets the text collation used when sorting this column.
    #[must_use]
    pub fn collation(mut self, collation: TextCollation) -> Self {
        self.spec = self.spec.collation(collation);
        self
    }

    /// Hides the column by default.
    #[must_use]
    pub fn hidden(mut self) -> Self {
        self.spec = self.spec.hidden();
        self
    }

    /// Makes the column editable through a setter that takes the field's own
    /// type. Text a user types is read as the column's kind of value and
    /// converted with [`FromValue`]; what cannot be converted is refused with
    /// a message before the setter runs.
    ///
    /// ```
    /// use dioxus_datagrid::Column;
    ///
    /// #[derive(Clone, PartialEq)]
    /// struct User { name: String, age: u32, email: Option<String> }
    ///
    /// let columns = vec![
    ///     Column::new("name", "Name")
    ///         .value_text(|user: &User| user.name.as_str())
    ///         .editable(|user: &mut User, name: String| user.name = name),
    ///     Column::new("age", "Age")
    ///         .value_of(|user: &User| user.age)
    ///         .editable(|user: &mut User, age: u32| user.age = age),
    ///     // Only an `Option` accepts an empty cell.
    ///     Column::new("email", "Email")
    ///         .value(|user: &User| user.email.as_deref().into())
    ///         .editable(|user: &mut User, email: Option<String>| user.email = email),
    /// ];
    /// # let _ = columns;
    /// ```
    #[must_use]
    pub fn editable<V, F>(mut self, set: F) -> Self
    where
        V: FromValue,
        F: Fn(&mut T, V) + 'static,
    {
        self.spec = self.spec.editable(set);
        self
    }

    /// Makes the column editable through a setter that takes the edited
    /// [`Value`] as it is, or `None` for an empty cell, and may refuse it.
    #[must_use]
    pub fn edit<F>(mut self, set: F) -> Self
    where
        F: Fn(&mut T, Option<Value>) -> Result<(), EditError> + 'static,
    {
        self.spec = self.spec.edit(set);
        self
    }

    /// Checks the row after this column was edited; the message shows at the
    /// cell's editor.
    #[must_use]
    pub fn validate<F>(mut self, validate: F) -> Self
    where
        F: Fn(&T) -> Result<(), String> + 'static,
    {
        self.spec = self.spec.validate(validate);
        self
    }

    /// Limits the column to these values, which its editor offers as a list.
    #[must_use]
    pub fn choices(mut self, choices: impl IntoIterator<Item = impl Into<Value>>) -> Self {
        self.spec = self.spec.choices(choices);
        self
    }

    /// Replaces the column's editor with one of your own. It gets a
    /// [`CellEditor`] with the text and the handlers to wire up; the grid
    /// still reads, validates and saves the text.
    ///
    /// ```
    /// use dioxus::prelude::*;
    /// use dioxus_datagrid::{CellEditor, Column};
    ///
    /// #[derive(Clone, PartialEq)]
    /// struct Task { notes: String }
    ///
    /// let notes = Column::new("notes", "Notes")
    ///     .value_text(|task: &Task| task.notes.as_str())
    ///     .editable(|task: &mut Task, notes: String| task.notes = notes)
    ///     .editor(|editor: CellEditor| rsx! {
    ///         textarea {
    ///             id: "{editor.id}",
    ///             aria_label: (!editor.in_form).then(|| editor.label.clone()),
    ///             value: "{editor.text}",
    ///             oninput: move |event| editor.set_text.call(event.value()),
    ///             onkeydown: editor.onkeydown,
    ///             onmounted: editor.onmounted,
    ///         }
    ///     });
    /// # let _ = notes;
    /// ```
    #[must_use]
    pub fn editor(mut self, render: impl Fn(CellEditor) -> Element + 'static) -> Self {
        self.editor = Some(Rc::new(render));
        self
    }

    /// The column's own editor, if it has one.
    #[must_use]
    pub fn custom_editor(&self) -> Option<&EditorRenderer> {
        self.editor.as_ref()
    }

    /// Whether the column can be edited.
    #[must_use]
    pub fn is_editable(&self) -> bool {
        self.spec.is_editable()
    }

    /// This column's id.
    #[must_use]
    pub fn id(&self) -> &ColumnId {
        &self.spec.id
    }

    /// This column's label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The core specification this column carries.
    #[must_use]
    pub fn spec(&self) -> &ColumnSpec<T> {
        &self.spec
    }

    /// Whether this column can be sorted.
    #[must_use]
    pub fn is_sortable(&self) -> bool {
        self.spec.is_sortable()
    }

    /// Renders one cell of this column: its own renderer if it has one,
    /// otherwise its value formatted for `locale`, otherwise nothing.
    pub fn render_cell(&self, row: &T, locale: &GridLocale) -> Element {
        match (&self.cell, self.spec.display_text(row, locale)) {
            (Some(render), _) => render(row),
            (None, Some(text)) => rsx! { "{text}" },
            (None, None) => rsx! {},
        }
    }

    /// The cell's text for a tooltip, if the column shows one: only with
    /// [`CellOverflow::TruncateWithTooltip`] and a value to format.
    #[must_use]
    pub fn tooltip(&self, row: &T, locale: &GridLocale) -> Option<String> {
        match self.spec.overflow {
            CellOverflow::TruncateWithTooltip => self.spec.display_text(row, locale),
            CellOverflow::Truncate | CellOverflow::Wrap => None,
        }
    }

    /// Renders this column's header content, defaulting to its label.
    pub fn render_header(&self) -> Element {
        match &self.header {
            Some(render) => render(),
            None => {
                let label = self.label.clone();
                rsx! { "{label}" }
            }
        }
    }
}

impl<T> Clone for Column<T> {
    fn clone(&self) -> Self {
        Self {
            spec: self.spec.clone(),
            label: self.label.clone(),
            cell: self.cell.clone(),
            header: self.header.clone(),
            editor: self.editor.clone(),
        }
    }
}

impl<T> fmt::Debug for Column<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Column")
            .field("id", self.id())
            .field("label", &self.label)
            .field("spec", &self.spec)
            .finish()
    }
}

/// Compares column identity, not the behaviour of the closures.
///
/// Dioxus uses this to decide whether a column list actually changed, so it has
/// to be cheap and it has to be false when a closure was rebuilt — which
/// `Rc::ptr_eq` gives us.
impl<T> PartialEq for Column<T> {
    fn eq(&self, other: &Self) -> bool {
        fn same<F: ?Sized>(a: Option<&Rc<F>>, b: Option<&Rc<F>>) -> bool {
            match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                _ => false,
            }
        }

        self.spec == other.spec
            && self.label == other.label
            && same(self.cell.as_ref(), other.cell.as_ref())
            && same(self.header.as_ref(), other.header.as_ref())
            && same(self.editor.as_ref(), other.editor.as_ref())
    }
}
