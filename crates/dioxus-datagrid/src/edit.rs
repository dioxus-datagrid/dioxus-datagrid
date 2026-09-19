//! Editing on the grid handle: the edit in progress, saving through the
//! application's callbacks, and changes collected for a batch.
//!
//! The grid never writes the rows it shows. An edit works on a copy of a row;
//! committing it hands the edited copy to a callback, and the application
//! stores it. While a save is in flight the grid shows the edited copy, and
//! takes it back if the save fails.

use crate::{Column, GridHandle};
use datagrid_core::{CellFocus, Changes, ColumnId, GridRow, ValueKind};
use dioxus::prelude::*;
use std::cell::RefCell;
use std::fmt;

/// How rows are edited.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EditMode {
    /// One cell at a time, saved as soon as the cell is committed.
    #[default]
    Cell,
    /// Every editable cell of a row at once, inline, saved together.
    Row,
    /// Every editable cell of a row in a form dialog.
    Dialog,
    /// One cell at a time, collected until the user saves or discards the
    /// whole batch.
    Batch,
}

/// Where focus goes after an edit is committed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditMove {
    /// Stays on the edited cell.
    Stay,
    /// Moves to the cell below, as `Enter` does.
    Down,
    /// Edits the next editable cell, as `Tab` does.
    Next,
    /// Edits the previous editable cell, as `Shift+Tab` does.
    Previous,
}

/// What is being edited, as the cells and the form need to know it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditTarget {
    /// The row's position on the current page; `None` for a new row.
    pub row_index: Option<usize>,
    /// The one cell being edited, or `None` for every editable cell of the row.
    pub column: Option<ColumnId>,
    /// Whether the edit happens in a form rather than in the cells.
    pub form: bool,
}

impl EditTarget {
    /// Whether the cell at `row_index` in `column` shows an editor.
    #[must_use]
    pub fn edits_cell(&self, row_index: usize, column: &ColumnId) -> bool {
        !self.form
            && self.row_index == Some(row_index)
            && self.column.as_ref().is_none_or(|id| id == column)
    }
}

/// Where saving stands, for a status message.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum EditStatus {
    /// Nothing saved yet, or the last message was cleared.
    #[default]
    Idle,
    /// A save is in progress.
    Saving,
    /// The last save succeeded.
    Saved,
    /// The last save failed, with the message to show.
    Failed(String),
}

/// How a grid edits, and what it calls to save. See
/// [`GridHandle::set_editing`].
///
/// Each callback receives a token with the rows. Saving succeeds unless the
/// callback calls the token's `fail`; the token may be moved into an `async`
/// block, and saving counts as finished when the block drops it:
///
/// ```ignore
/// on_save: move |save: Save<User>| async move {
///     if let Err(error) = api.update(save.row()).await {
///         save.fail(error.to_string());
///     }
/// }
/// ```
///
/// For a local grid, the callbacks update the data the grid shows. A remote
/// grid reloads its page after every successful save.
pub struct Editing<T: GridRow + 'static> {
    /// How rows are edited.
    pub mode: EditMode,
    /// Saves an edited row. Not called in [`EditMode::Batch`].
    pub on_save: Option<EventHandler<Save<T>>>,
    /// Saves a new row. Not called in [`EditMode::Batch`].
    pub on_create: Option<EventHandler<Create<T>>>,
    /// Deletes rows. Not called in [`EditMode::Batch`].
    pub on_delete: Option<EventHandler<Delete<T>>>,
    /// Saves a batch of changes in [`EditMode::Batch`].
    pub on_batch_save: Option<EventHandler<SaveBatch<T>>>,
    /// Makes the row a new-row form starts from. Without it rows cannot be
    /// added. Its key must not collide with an existing row's.
    pub new_row: Option<Callback<(), T>>,
    /// Checks a whole row before it is committed, after every column's own
    /// validation passed.
    pub validate_row: Option<Callback<T, Result<(), String>>>,
    /// Whether deleting asks first. `true` by default.
    pub confirm_delete: bool,
}

impl<T: GridRow> Default for Editing<T> {
    fn default() -> Self {
        Self {
            mode: EditMode::default(),
            on_save: None,
            on_create: None,
            on_delete: None,
            on_batch_save: None,
            new_row: None,
            validate_row: None,
            confirm_delete: true,
        }
    }
}

impl<T: GridRow> Clone for Editing<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: GridRow> Copy for Editing<T> {}

impl<T: GridRow> PartialEq for Editing<T> {
    fn eq(&self, other: &Self) -> bool {
        self.mode == other.mode
            && self.on_save == other.on_save
            && self.on_create == other.on_create
            && self.on_delete == other.on_delete
            && self.on_batch_save == other.on_batch_save
            && self.new_row == other.new_row
            && self.validate_row == other.validate_row
            && self.confirm_delete == other.confirm_delete
    }
}

impl<T: GridRow> fmt::Debug for Editing<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Editing")
            .field("mode", &self.mode)
            .field("can_create", &self.new_row.is_some())
            .field("confirm_delete", &self.confirm_delete)
            .finish_non_exhaustive()
    }
}

/// Reports how a save ended, once. Dropping it without a report counts as
/// success.
struct Completion(RefCell<Option<Finish>>);

/// What a [`Completion`] runs with the result.
type Finish = Box<dyn FnOnce(Result<(), String>)>;

impl Completion {
    fn new(finish: impl FnOnce(Result<(), String>) + 'static) -> Self {
        Self(RefCell::new(Some(Box::new(finish))))
    }

    fn finish(&self, result: Result<(), String>) {
        // Taken before calling, so the report can touch anything it likes.
        let finish = self.0.borrow_mut().take();
        if let Some(finish) = finish {
            finish(result);
        }
    }
}

impl Drop for Completion {
    fn drop(&mut self) {
        self.finish(Ok(()));
    }
}

macro_rules! token_methods {
    () => {
        /// Reports that saving failed. The grid shows `message` and, for an
        /// edited row, the row as it was.
        pub fn fail(self, message: impl Into<String>) {
            self.done.finish(Err(message.into()));
        }

        /// Reports that saving succeeded. The same as dropping the token.
        pub fn succeed(self) {
            self.done.finish(Ok(()));
        }
    };
}

/// An edited row to save, handed to [`Editing::on_save`].
pub struct Save<T> {
    original: T,
    row: T,
    done: Completion,
}

impl<T> Save<T> {
    /// The row as it was before the edit.
    pub fn original(&self) -> &T {
        &self.original
    }

    /// The row as edited.
    pub fn row(&self) -> &T {
        &self.row
    }

    token_methods!();
}

/// A new row to save, handed to [`Editing::on_create`].
pub struct Create<T> {
    row: T,
    done: Completion,
}

impl<T> Create<T> {
    /// The new row.
    pub fn row(&self) -> &T {
        &self.row
    }

    token_methods!();
}

/// Rows to delete, handed to [`Editing::on_delete`].
pub struct Delete<T> {
    rows: Vec<T>,
    done: Completion,
}

impl<T> Delete<T> {
    /// The rows to delete.
    pub fn rows(&self) -> &[T] {
        &self.rows
    }

    token_methods!();
}

/// A batch of changes to save, handed to [`Editing::on_batch_save`]. Saved
/// all together or not at all: on failure the changes stay, to be saved again
/// or discarded.
pub struct SaveBatch<T> {
    changes: Changes<T>,
    done: Completion,
}

impl<T> SaveBatch<T> {
    /// The changed, added and deleted rows.
    pub fn changes(&self) -> &Changes<T> {
        &self.changes
    }

    token_methods!();
}

macro_rules! token_debug {
    ($($name:ident),*) => {$(
        impl<T> fmt::Debug for $name<T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name)).finish_non_exhaustive()
            }
        }
    )*};
}

token_debug!(Save, Create, Delete, SaveBatch);

/// The edit in progress.
#[derive(Clone)]
pub(crate) struct Session<T: GridRow> {
    /// Which row, by key rather than position, so that the edit follows its
    /// row if the page changes underneath it.
    pub(crate) key: T::Key,
    original: T,
    pub(crate) creating: bool,
    /// The one cell being edited, for cell and batch edits.
    column: Option<ColumnId>,
    pub(crate) form: bool,
    /// Every editor's text, as first shown and as typed now.
    texts: Vec<(ColumnId, String, String)>,
    errors: Vec<(ColumnId, String)>,
    row_error: Option<String>,
    /// The editor that should have focus, and a counter so asking again
    /// moves focus there again.
    focus: Option<ColumnId>,
    focus_nonce: u64,
}

impl<T: GridRow> Session<T> {
    /// Where the edit shows, with its row at `row_index` on the current page.
    pub(crate) fn target(&self, row_index: Option<usize>) -> EditTarget {
        EditTarget {
            row_index,
            column: self.column.clone(),
            form: self.form,
        }
    }
}

/// A row shown as edited while its save is in flight.
#[derive(Clone)]
pub(crate) struct Pending<T> {
    row: T,
    /// Saved, and waiting for a remote grid's reload to show the server's
    /// version.
    confirmed: bool,
}

/// Rows the grid shows differently from its data: edited rows being saved,
/// and a batch's changes.
#[derive(Clone)]
pub(crate) struct EditRows<T> {
    pending: Vec<Pending<T>>,
    changes: Changes<T>,
}

impl<T> Default for EditRows<T> {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
            changes: Changes::default(),
        }
    }
}

impl<T: GridRow + PartialEq> EditRows<T> {
    /// The row with `key` as the grid should show it, if not as in the data.
    fn current(&self, key: &T::Key) -> Option<&T> {
        self.changes.current(key).or_else(|| {
            self.pending
                .iter()
                .rev()
                .find(|pending| pending.row.key() == *key)
                .map(|pending| &pending.row)
        })
    }

    /// Forgets saved rows once a remote grid's new page has arrived.
    pub(crate) fn drop_confirmed(&mut self) {
        self.pending.retain(|pending| !pending.confirmed);
    }
}

/// Writes a signal unless the grid it belongs to is gone, as it may be when a
/// save finishes after the page was left.
fn set_quietly<V: 'static>(signal: &mut Signal<V>, update: impl FnOnce(&mut V)) {
    if let Ok(mut value) = signal.try_write() {
        update(&mut value);
    }
}

impl<T: GridRow + PartialEq> GridHandle<T> {
    /// Sets how the grid edits and what it calls to save. Call it on every
    /// render, as the registry's editor component does; it only stores the
    /// configuration and never re-renders anything.
    ///
    /// Without it the grid is read-only.
    pub fn set_editing(&mut self, editing: Editing<T>) {
        // Compared first: the callbacks keep their identity across renders,
        // so this writes only when something really changed.
        let previous = *self.edit_config.peek();
        if previous == Some(editing) {
            return;
        }
        // An edit or a batch started under another mode cannot be finished
        // under this one: a batch would have no way left to be saved.
        if previous.is_some_and(|previous| previous.mode != editing.mode) {
            self.abandon_edits();
        }
        self.edit_config.set(Some(editing));
    }

    /// Makes the grid read-only again, dropping any edit in progress and any
    /// unsaved batch. The registry's editor component calls it when it goes
    /// away.
    pub fn clear_editing(&mut self) {
        if self
            .edit_config
            .try_peek()
            .is_ok_and(|config| config.is_some())
        {
            self.abandon_edits();
            set_quietly(&mut self.edit_config, |config| *config = None);
        }
    }

    /// Drops the edit in progress, the batch and a pending delete.
    fn abandon_edits(&mut self) {
        set_quietly(&mut self.edit_session, |session| *session = None);
        set_quietly(&mut self.edit_rows, |rows| {
            rows.changes = Changes::default()
        });
        set_quietly(&mut self.edit_confirm, |confirm| *confirm = None);
    }

    /// How the grid edits, if it does. Reading it subscribes the caller.
    #[must_use]
    pub fn edit_mode(&self) -> Option<EditMode> {
        self.edit_config.read().as_ref().map(|editing| editing.mode)
    }

    fn editing(&self) -> Option<Editing<T>> {
        *self.edit_config.peek()
    }

    /// Whether rows can be added: the grid edits and has a
    /// [`new_row`](Editing::new_row).
    #[must_use]
    pub fn can_create(&self) -> bool {
        self.editing()
            .is_some_and(|editing| editing.new_row.is_some())
    }

    /// Whether rows can be deleted.
    #[must_use]
    pub fn can_delete(&self) -> bool {
        self.editing().is_some_and(|editing| match editing.mode {
            EditMode::Batch => editing.on_batch_save.is_some(),
            _ => editing.on_delete.is_some(),
        })
    }

    /// Whether any column can be edited.
    #[must_use]
    pub fn has_editable_columns(&self) -> bool {
        self.edit_config.read().is_some()
            && self
                .columns
                .read()
                .iter()
                .any(|column| column.spec().is_editable())
    }

    /// What is being edited, if anything. Reading it subscribes the caller,
    /// but only to changes of the target, not to every keystroke.
    #[must_use]
    pub fn edit_target(&self) -> Option<EditTarget> {
        self.edit_target.read().clone()
    }

    /// Whether an edit is in progress.
    #[must_use]
    pub fn is_editing(&self) -> bool {
        self.edit_target.read().is_some()
    }

    /// The row at `row_index` on the current page as the grid shows it: with
    /// unsaved batch changes and edits being saved applied.
    pub fn with_row<R>(&self, row_index: usize, read: impl FnOnce(&T) -> R) -> Option<R> {
        let index = *self.view().read().indices.get(row_index)?;
        let data = self.data.read();
        let row = data.get(index)?;
        let edits = self.edit_rows.read();
        Some(read(edits.current(&row.key()).unwrap_or(row)))
    }

    fn row_at(&self, row_index: usize) -> Option<T> {
        self.with_row(row_index, T::clone)
    }

    /// Starts editing the cell at `row_index` on the current page, in the
    /// visible column `column_index`: that cell alone in [`EditMode::Cell`]
    /// and [`EditMode::Batch`], the whole row otherwise, with focus on that
    /// column's editor.
    ///
    /// Commits an edit already in progress first, and does nothing if that
    /// fails. Returns whether an edit started.
    pub fn start_edit(&mut self, row_index: usize, column_index: usize) -> bool {
        let Some(editing) = self.editing() else {
            return false;
        };
        if self.edit_session.peek().is_some() && !self.commit_edit(EditMove::Stay) {
            return false;
        }
        let columns = self.visible_columns();
        let Some(column) = columns.get(column_index) else {
            return false;
        };
        let Some(row) = self.row_at(row_index) else {
            return false;
        };
        let key = row.key();
        if self.edit_rows.peek().changes.is_deleted(&key) {
            return false;
        }

        let editable: Vec<&Column<T>> = columns
            .iter()
            .filter(|column| column.spec().is_editable())
            .collect();
        let (edited, single, form): (Vec<&Column<T>>, bool, bool) = match editing.mode {
            EditMode::Cell | EditMode::Batch => {
                if !column.spec().is_editable() {
                    return false;
                }
                (vec![column], true, false)
            }
            EditMode::Row => (editable, false, false),
            EditMode::Dialog => (editable, false, true),
        };
        let Some(first) = edited.first() else {
            return false;
        };
        let focus = if column.spec().is_editable() {
            column.id().clone()
        } else {
            first.id().clone()
        };

        let texts = self.texts_for(&edited, &row);
        self.edit_session.set(Some(Session {
            key,
            original: row,
            creating: false,
            column: single.then(|| column.id().clone()),
            form,
            texts,
            errors: Vec::new(),
            row_error: None,
            focus: Some(focus),
            focus_nonce: 0,
        }));
        self.set_focus_quietly(CellFocus::new(row_index + 1, column_index));
        true
    }

    /// Opens the form for a new row, starting from
    /// [`new_row`](Editing::new_row). Returns whether it opened.
    pub fn start_create(&mut self) -> bool {
        let Some(new_row) = self.editing().and_then(|editing| editing.new_row) else {
            return false;
        };
        if self.edit_session.peek().is_some() && !self.commit_edit(EditMove::Stay) {
            return false;
        }
        let row = new_row.call(());
        let columns = self.visible_columns();
        let editable: Vec<&Column<T>> = columns
            .iter()
            .filter(|column| column.spec().is_editable())
            .collect();
        let Some(first) = editable.first() else {
            return false;
        };
        let focus = first.id().clone();
        let texts = self.texts_for(&editable, &row);
        self.edit_session.set(Some(Session {
            key: row.key(),
            original: row,
            creating: true,
            column: None,
            form: true,
            texts,
            errors: Vec::new(),
            row_error: None,
            focus: Some(focus),
            focus_nonce: 0,
        }));
        true
    }

    fn texts_for(&self, columns: &[&Column<T>], row: &T) -> Vec<(ColumnId, String, String)> {
        let locale = self.locale().peek().clone();
        columns
            .iter()
            .map(|column| {
                let text = column.spec().edit_text(row, &locale);
                (column.id().clone(), text.clone(), text)
            })
            .collect()
    }

    /// The text in the editor for `column`, or `None` if it has none.
    #[must_use]
    pub fn editor_text(&self, column: &ColumnId) -> Option<String> {
        self.edit_session.read().as_ref().and_then(|session| {
            session
                .texts
                .iter()
                .find(|(id, _, _)| id == column)
                .map(|(_, _, text)| text.clone())
        })
    }

    /// Changes the text in the editor for `column`, and clears its error.
    pub fn set_editor_text(&mut self, column: &ColumnId, text: impl Into<String>) {
        let text = text.into();
        if let Some(session) = self.edit_session.write().as_mut() {
            if let Some(entry) = session.texts.iter_mut().find(|(id, _, _)| id == column) {
                entry.2 = text;
            }
            session.errors.retain(|(id, _)| id != column);
        }
    }

    /// Why the editor for `column` was refused, if it was.
    #[must_use]
    pub fn editor_error(&self, column: &ColumnId) -> Option<String> {
        self.edit_session.read().as_ref().and_then(|session| {
            session
                .errors
                .iter()
                .find(|(id, _)| id == column)
                .map(|(_, message)| message.clone())
        })
    }

    /// Why the row as a whole was refused, if it was: the
    /// [row validator's](Editing::validate_row) message, or a note that
    /// fields have errors.
    #[must_use]
    pub fn edit_row_error(&self) -> Option<String> {
        self.edit_session
            .read()
            .as_ref()
            .and_then(|session| session.row_error.clone())
    }

    /// The editor that should take focus, and a counter that changes each
    /// time focus is asked for again.
    #[must_use]
    pub fn editor_focus(&self) -> Option<(ColumnId, u64)> {
        self.edit_session.read().as_ref().and_then(|session| {
            session
                .focus
                .clone()
                .map(|column| (column, session.focus_nonce))
        })
    }

    /// Moves focus to the editor for `column`.
    pub fn focus_editor(&mut self, column: &ColumnId) {
        if let Some(session) = self.edit_session.write().as_mut() {
            session.focus = Some(column.clone());
            session.focus_nonce = session.focus_nonce.wrapping_add(1);
        }
    }

    /// The columns with an editor, in order.
    #[must_use]
    pub fn edited_columns(&self) -> Vec<ColumnId> {
        self.edit_session
            .read()
            .as_ref()
            .map(|session| session.texts.iter().map(|(id, _, _)| id.clone()).collect())
            .unwrap_or_default()
    }

    /// Whether the edit is of a new row.
    #[must_use]
    pub fn is_creating(&self) -> bool {
        self.edit_session
            .read()
            .as_ref()
            .is_some_and(|session| session.creating)
    }

    /// Abandons the edit in progress. Focus returns to the edited cell.
    pub fn cancel_edit(&mut self) {
        let row_index = self.edited_row_index();
        let Some(session) = self.edit_session.write().take() else {
            return;
        };
        self.return_focus(&session, row_index, EditMove::Stay);
    }

    /// Where the edited row is on the current page, if it is there.
    fn edited_row_index(&self) -> Option<usize> {
        self.edit_target
            .peek()
            .as_ref()
            .and_then(|target| target.row_index)
    }

    /// Reads every editor, validates the row and, if all is well, saves it or,
    /// in [`EditMode::Batch`], records it. `then` says where focus goes.
    ///
    /// If something is refused, the edit stays open with the errors shown and
    /// focus on the first refused editor, and `false` is returned.
    pub fn commit_edit(&mut self, then: EditMove) -> bool {
        let Some(session) = self.edit_session.peek().clone() else {
            return true;
        };
        let Some(editing) = self.editing() else {
            self.edit_session.set(None);
            return true;
        };
        let locale = self.locale().peek().clone();
        let columns = self.columns.peek().clone();

        let mut draft = session.original.clone();
        let mut errors = Vec::new();
        for (id, first, text) in &session.texts {
            // An existing row keeps what was not touched, even if it would not
            // pass today's validation. A new row is checked in full.
            if !session.creating && first == text {
                continue;
            }
            let Some(column) = columns.iter().find(|column| column.id() == id) else {
                continue;
            };
            let kind = self.value_kind(id).unwrap_or(ValueKind::Text);
            if let Err(error) = column.spec().apply_edit(&mut draft, text, kind) {
                errors.push((id.clone(), error.message(&locale)));
            }
        }
        let mut row_error =
            (!errors.is_empty() && session.texts.len() > 1).then(|| locale.edit_check.to_string());
        if errors.is_empty() {
            if let Some(validate) = editing.validate_row {
                row_error = validate.call(draft.clone()).err();
            }
        }

        if !errors.is_empty() || row_error.is_some() {
            if let Some(open) = self.edit_session.write().as_mut() {
                open.focus = errors
                    .first()
                    .map(|(id, _)| id.clone())
                    .or(open.focus.clone());
                open.focus_nonce = open.focus_nonce.wrapping_add(1);
                open.errors = errors;
                open.row_error = row_error;
            }
            return false;
        }

        let row_index = self.edited_row_index();
        self.edit_session.set(None);
        self.save(&editing, &session, draft);
        self.return_focus(&session, row_index, then);
        true
    }

    /// Hands a committed edit to the application, or to the batch.
    fn save(&mut self, editing: &Editing<T>, session: &Session<T>, draft: T) {
        if editing.mode == EditMode::Batch {
            let mut rows = self.edit_rows.write();
            if session.creating {
                rows.changes.add(draft);
            } else {
                rows.changes.update(session.original.clone(), draft);
            }
            return;
        }

        let mut grid = *self;
        if session.creating {
            let Some(on_create) = editing.on_create else {
                return;
            };
            self.edit_status.set(EditStatus::Saving);
            on_create.call(Create {
                row: draft,
                done: Completion::new(move |result| grid.finish_save(result, None)),
            });
            return;
        }

        if draft == session.original {
            return;
        }
        let Some(on_save) = editing.on_save else {
            return;
        };
        let key = session.key.clone();
        self.edit_rows.write().pending.push(Pending {
            row: draft.clone(),
            confirmed: false,
        });
        self.edit_status.set(EditStatus::Saving);
        on_save.call(Save {
            original: session.original.clone(),
            row: draft,
            done: Completion::new(move |result| grid.finish_save(result, Some(key))),
        });
    }

    /// Takes a save's result: keeps or drops the edited row shown in the
    /// meantime, and reports the outcome.
    fn finish_save(&mut self, result: Result<(), String>, key: Option<T::Key>) {
        let remote = self.remote;
        if let Some(key) = &key {
            set_quietly(&mut self.edit_rows, |rows| {
                let index = rows
                    .pending
                    .iter()
                    .position(|pending| !pending.confirmed && pending.row.key() == *key);
                if let Some(index) = index {
                    if remote && result.is_ok() {
                        if let Some(pending) = rows.pending.get_mut(index) {
                            pending.confirmed = true;
                        }
                    } else {
                        rows.pending.remove(index);
                    }
                }
            });
        }
        self.report(result);
    }

    /// Shows how a save ended, and reloads a remote grid after a success.
    fn report(&mut self, result: Result<(), String>) {
        let status = match result {
            Ok(()) => {
                if self.remote {
                    self.reload();
                }
                EditStatus::Saved
            }
            Err(message) => {
                let text = self
                    .locale
                    .try_peek()
                    .map(|locale| locale.edit_save_failed(&message))
                    .unwrap_or(message);
                EditStatus::Failed(text)
            }
        };
        set_quietly(&mut self.edit_status, |current| *current = status);
    }

    /// Puts focus back on the grid after an edit ends.
    fn return_focus(&mut self, session: &Session<T>, row_index: Option<usize>, then: EditMove) {
        let Some(row_index) = row_index else {
            // A new row: back to wherever the grid's focus was.
            self.request_focus_pull();
            return;
        };
        let columns = self.visible_columns();
        let column_index = session
            .column
            .as_ref()
            .or(session.focus.as_ref())
            .and_then(|id| columns.iter().position(|column| column.id() == id))
            .unwrap_or(0);
        let rows = self.view().read().indices.len();

        match then {
            EditMove::Stay => {}
            EditMove::Down => {
                if row_index + 1 < rows {
                    self.set_focus(CellFocus::new(row_index + 2, column_index));
                    self.request_focus_pull();
                    return;
                }
            }
            EditMove::Next | EditMove::Previous => {
                if let Some((row, column)) =
                    self.next_editable(row_index, column_index, then == EditMove::Next)
                {
                    if self.start_edit(row, column) {
                        return;
                    }
                }
            }
        }
        self.set_focus(CellFocus::new(row_index + 1, column_index));
        self.request_focus_pull();
    }

    /// The next editable cell after (or before) the given one, across rows on
    /// the current page.
    fn next_editable(
        &self,
        row_index: usize,
        column_index: usize,
        forward: bool,
    ) -> Option<(usize, usize)> {
        let columns = self.visible_columns();
        let editable: Vec<usize> = columns
            .iter()
            .enumerate()
            .filter(|(_, column)| column.spec().is_editable())
            .map(|(index, _)| index)
            .collect();
        let rows = self.view().read().indices.len();
        let width = columns.len();
        let here = row_index * width + column_index;
        let cells = (0..rows).flat_map(|row| editable.iter().map(move |&column| (row, column)));
        if forward {
            cells
                .into_iter()
                .find(|&(row, column)| row * width + column > here)
        } else {
            cells
                .into_iter()
                .rev()
                .find(|&(row, column)| row * width + column < here)
        }
    }

    /// Moves focus between the editors of a row edited inline, wrapping at
    /// either end, as `Tab` does there.
    pub fn focus_next_editor(&mut self, from: &ColumnId, forward: bool) {
        let columns = self.edited_columns();
        let Some(position) = columns.iter().position(|id| id == from) else {
            return;
        };
        let count = columns.len();
        let next = if forward {
            (position + 1) % count
        } else {
            (position + count - 1) % count
        };
        if let Some(column) = columns.get(next) {
            let column = column.clone();
            self.focus_editor(&column);
        }
    }

    /// Asks to delete the rows with `keys`: after confirmation if
    /// [`confirm_delete`](Editing::confirm_delete) is set, otherwise at once.
    pub fn request_delete(&mut self, keys: &[T::Key]) {
        let Some(editing) = self.editing() else {
            return;
        };
        if !self.can_delete() || keys.is_empty() {
            return;
        }
        let rows: Vec<T> = (0..self.view().read().indices.len())
            .filter_map(|index| self.row_at(index))
            .filter(|row| keys.contains(&row.key()))
            .collect();
        if rows.is_empty() {
            return;
        }
        if editing.confirm_delete {
            self.edit_confirm.set(Some(rows));
        } else {
            self.delete_rows(rows);
        }
    }

    /// The rows the focused row stands for when deleting: the selection if the
    /// focused row is part of it, otherwise the focused row alone.
    #[must_use]
    pub fn delete_candidates(&self) -> Vec<T::Key> {
        let Some(row_index) = self.focus().row.checked_sub(1) else {
            return self.selected_keys();
        };
        match self.key_at(row_index) {
            Some(key) if self.is_selected(&key) => self.selected_keys(),
            Some(key) => vec![key],
            None => self.selected_keys(),
        }
    }

    /// How many rows wait for the user to confirm deleting them.
    #[must_use]
    pub fn pending_delete(&self) -> Option<usize> {
        self.edit_confirm.read().as_ref().map(Vec::len)
    }

    /// Deletes the rows waiting for confirmation.
    pub fn confirm_delete(&mut self) {
        let Some(rows) = self.edit_confirm.write().take() else {
            return;
        };
        self.delete_rows(rows);
        self.request_focus_pull();
    }

    /// Keeps the rows waiting for confirmation.
    pub fn cancel_delete(&mut self) {
        if self.edit_confirm.write().take().is_some() {
            self.request_focus_pull();
        }
    }

    fn delete_rows(&mut self, rows: Vec<T>) {
        let Some(editing) = self.editing() else {
            return;
        };
        if editing.mode == EditMode::Batch {
            let mut edits = self.edit_rows.write();
            for row in rows {
                edits.changes.delete(row);
            }
            return;
        }
        let Some(on_delete) = editing.on_delete else {
            return;
        };
        let keys: Vec<T::Key> = rows.iter().map(GridRow::key).collect();
        let mut grid = *self;
        self.edit_status.set(EditStatus::Saving);
        on_delete.call(Delete {
            rows,
            done: Completion::new(move |result| {
                // A remote grid keeps its selection across pages, so deleted
                // rows have to leave it here. A local grid prunes it when the
                // rows leave the data.
                if grid.remote && result.is_ok() {
                    set_quietly(&mut grid.selection, |selection| {
                        for key in &keys {
                            selection.deselect(key);
                        }
                    });
                }
                grid.report(result);
            }),
        });
    }

    /// How many rows the batch has changed, added or deleted.
    #[must_use]
    pub fn pending_changes(&self) -> usize {
        self.edit_rows.read().changes.len()
    }

    /// The batch's changes.
    #[must_use]
    pub fn changes(&self) -> Changes<T> {
        self.edit_rows.read().changes.clone()
    }

    /// Whether the cell at `row_index` in the visible column `column_index`
    /// has an unsaved batch change.
    #[must_use]
    pub fn is_cell_changed(&self, row_index: usize, column_index: usize) -> bool {
        let Some(key) = self.key_at(row_index) else {
            return false;
        };
        let edits = self.edit_rows.read();
        let Some(original) = edits.changes.original(&key) else {
            return false;
        };
        let Some(current) = edits.changes.current(&key) else {
            return false;
        };
        let columns = self.visible_columns();
        columns.get(column_index).is_some_and(|column| {
            !datagrid_core::changed_columns(std::slice::from_ref(column.spec()), original, current)
                .is_empty()
        })
    }

    /// Whether the row at `row_index` is marked deleted in the batch.
    #[must_use]
    pub fn is_row_deleted(&self, row_index: usize) -> bool {
        self.key_at(row_index)
            .is_some_and(|key| self.edit_rows.read().changes.is_deleted(&key))
    }

    /// Whether the row at `row_index` is shown as edited while its save runs.
    #[must_use]
    pub fn is_row_saving(&self, row_index: usize) -> bool {
        self.key_at(row_index).is_some_and(|key| {
            self.edit_rows
                .read()
                .pending
                .iter()
                .any(|pending| !pending.confirmed && pending.row.key() == key)
        })
    }

    /// Saves the batch through [`on_batch_save`](Editing::on_batch_save).
    pub fn save_changes(&mut self) {
        if self.edit_session.peek().is_some() && !self.commit_edit(EditMove::Stay) {
            return;
        }
        let Some(on_batch_save) = self.editing().and_then(|editing| editing.on_batch_save) else {
            return;
        };
        let changes = self.edit_rows.peek().changes.clone();
        if changes.is_empty() {
            return;
        }
        let mut grid = *self;
        let saved = changes.clone();
        self.edit_status.set(EditStatus::Saving);
        on_batch_save.call(SaveBatch {
            changes,
            done: Completion::new(move |result| {
                if result.is_ok() {
                    let remote = grid.remote;
                    set_quietly(&mut grid.edit_rows, |rows| {
                        // Until a remote grid's reload arrives, show the
                        // saved versions rather than the old ones.
                        if remote {
                            rows.pending
                                .extend(saved.updated.iter().map(|(_, row)| Pending {
                                    row: row.clone(),
                                    confirmed: true,
                                }));
                        }
                        rows.changes = Changes::default();
                    });
                }
                grid.report(result);
            }),
        });
    }

    /// Abandons the batch.
    pub fn discard_changes(&mut self) {
        self.edit_session.set(None);
        self.edit_rows.write().changes = Changes::default();
        self.edit_status.set(EditStatus::Idle);
    }

    /// Where saving stands.
    #[must_use]
    pub fn edit_status(&self) -> EditStatus {
        self.edit_status.read().clone()
    }
}
