//! Column definitions that pair core sorting and filtering with rendering.

use datagrid_core::{ColumnId, ColumnSpec, ColumnWidth, SortValue, TextCollation};
use dioxus::prelude::*;
use std::fmt;
use std::rc::Rc;

/// Renders one cell of a column.
pub type CellRenderer<T> = Rc<dyn Fn(&T) -> Element>;

/// Renders a column's header content.
pub type HeaderRenderer = Rc<dyn Fn() -> Element>;

/// A column: what the core needs to sort and filter it, plus how to draw it.
///
/// Built fluently. Only an id and a label are required; a column with no
/// [`sort_by`](Column::sort_by_text) cannot be sorted, and one with no
/// [`filter_by`](Column::filter_by) takes part in neither filtering nor search.
///
/// ```
/// use dioxus::prelude::*;
/// use dioxus_datagrid::Column;
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
///         .cell(|user: &User| rsx! { "{user.age}" })
///         .sort_by_value(|user: &User| user.age),
/// ];
/// # let _ = columns;
/// ```
pub struct Column<T> {
    spec: ColumnSpec<T>,
    label: String,
    cell: Option<CellRenderer<T>>,
    header: Option<HeaderRenderer>,
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
        }
    }

    /// Sets how a cell of this column is rendered.
    ///
    /// Without this the column renders empty cells, which is occasionally what
    /// you want for a column that exists only to be sorted or filtered on.
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

    /// Renders one cell of this column, or nothing if no renderer was set.
    pub fn render_cell(&self, row: &T) -> Element {
        match &self.cell {
            Some(render) => render(row),
            None => rsx! {},
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
    }
}
