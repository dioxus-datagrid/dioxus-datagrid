//! The editing primitives: editors in cells and in a form, the confirmation
//! before deleting, buttons to start and save edits, and a status line.
//!
//! Like every primitive they render roles, ARIA attributes and key handling,
//! and leave the look to `data-*` attributes and the caller's classes.

use crate::filter_menu::next_id;
use crate::{Column, EditMode, EditMove, EditStatus, GridHandle};
use datagrid_core::{GridRow as GridRowKey, ValueKind};
use dioxus::prelude::*;
use std::rc::Rc;

/// What a column's own editor works with; see
/// [`Column::editor`](crate::Column::editor).
///
/// The grid keeps the text. An editor shows [`text`](CellEditor::text), hands
/// every change to [`set_text`](CellEditor::set_text), and puts
/// [`onkeydown`](CellEditor::onkeydown) and
/// [`onmounted`](CellEditor::onmounted) on its input so keys and focus work
/// as in the built-in editors.
#[derive(Clone, PartialEq)]
pub struct CellEditor {
    /// The text being edited.
    pub text: String,
    /// Why the last attempt to commit refused this text, if it did.
    pub error: Option<String>,
    /// The column's label: the input's `aria-label` in a cell.
    pub label: String,
    /// Whether the editor sits in a form, where a `<label for>` names it
    /// instead.
    pub in_form: bool,
    /// The id to give the input.
    pub id: String,
    /// The id of the error message, for `aria-describedby` while there is one.
    pub error_id: String,
    /// Changes the text.
    pub set_text: Callback<String>,
    /// Commits on `Enter`, cancels on `Escape` and moves on with `Tab`.
    pub onkeydown: Callback<KeyboardEvent>,
    /// Takes focus when the grid asks for this editor.
    pub onmounted: Callback<MountedEvent>,
}

impl std::fmt::Debug for CellEditor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CellEditor")
            .field("text", &self.text)
            .field("error", &self.error)
            .field("label", &self.label)
            .field("in_form", &self.in_form)
            .finish_non_exhaustive()
    }
}

/// Reads an editor's text as a checkbox state.
fn checked(text: &str) -> bool {
    matches!(
        text.trim().to_lowercase().as_str(),
        "true" | "yes" | "ja" | "1"
    )
}

/// The editor for one column of the row being edited: in its cell, or as a
/// field of [`GridEditDialog`].
///
/// [`GridCell`](crate::primitives::GridCell) renders it by itself while its
/// cell is edited; use it directly only to build a form of your own.
///
/// Picks the input from the column: a list for a column with
/// [choices](datagrid_core::ColumnSpec::choices), a checkbox for booleans, a
/// date or date-and-time input for dates, a text input otherwise, with
/// `inputmode="decimal"` for numbers. A column's own
/// [editor](crate::Column::editor) replaces it.
///
/// Keys: `Enter` commits — and in a cell moves down, except when a whole row
/// is edited. `Escape` cancels. In a cell, `Tab` and `Shift+Tab` commit and
/// edit the next or previous editable cell; when a whole row is edited they
/// move between its editors. In a form, `Tab` is left to the browser.
///
/// A refused value shows its message after the input, with `role="alert"`,
/// and the input carries `aria-invalid` and `aria-describedby`. Renders
/// `data-editor` on the input and `data-edit-error` on the message.
#[component]
pub fn GridCellEditor<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Position among the visible columns, zero-based.
    column_index: usize,
    /// Whether the editor is a form field rather than a cell's content.
    #[props(default)]
    in_form: bool,
    /// The input's id. Generated if not given.
    #[props(default)]
    input_id: Option<String>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let generated = use_hook(|| format!("dg-editor-{}", next_id()));
    let id = input_id.unwrap_or(generated);
    let error_id = format!("{id}-error");
    let mut element = use_signal(|| None::<Rc<MountedData>>);
    let mut handled = use_signal(|| None::<u64>);

    let columns = grid.visible_columns();
    let column: Option<Column<T>> = columns.get(column_index).cloned();
    let column_id = column.as_ref().map(|column| column.id().clone());

    // Take focus whenever the grid asks for this editor, once per request.
    let wanted = column_id.clone();
    use_effect(move || {
        let Some((asked, nonce)) = grid.editor_focus() else {
            return;
        };
        if Some(&asked) != wanted.as_ref() || *handled.peek() == Some(nonce) {
            return;
        }
        if let Some(target) = element.peek().clone() {
            handled.set(Some(nonce));
            spawn(async move {
                let _ = target.set_focus(true).await;
            });
        }
    });

    let mounted_column = column_id.clone();
    let onmounted = use_callback(move |event: MountedEvent| {
        let target = event.data();
        element.set(Some(target.clone()));
        if let Some((asked, nonce)) = grid.editor_focus() {
            if Some(&asked) == mounted_column.as_ref() && *handled.peek() != Some(nonce) {
                handled.set(Some(nonce));
                spawn(async move {
                    let _ = target.set_focus(true).await;
                });
            }
        }
    });

    let text_column = column_id.clone();
    let set_text = use_callback(move |text: String| {
        if let Some(column) = &text_column {
            grid.set_editor_text(column, text);
        }
    });

    let key_column = column_id.clone();
    let onkeydown = use_callback(move |event: KeyboardEvent| {
        let data = event.data();
        let row_mode = grid.edit_mode() == Some(EditMode::Row);
        match data.key() {
            Key::Enter => {
                event.prevent_default();
                let then = if in_form || row_mode {
                    EditMove::Stay
                } else {
                    EditMove::Down
                };
                grid.commit_edit(then);
            }
            Key::Escape => {
                event.prevent_default();
                grid.cancel_edit();
            }
            Key::Tab if !in_form => {
                event.prevent_default();
                let forward = !data.modifiers().shift();
                if row_mode {
                    if let Some(column) = &key_column {
                        grid.focus_next_editor(column, forward);
                    }
                } else {
                    grid.commit_edit(if forward {
                        EditMove::Next
                    } else {
                        EditMove::Previous
                    });
                }
            }
            _ => {}
        }
        // The grid's own keys — arrows, Space, Enter — must not act while
        // typing, so nothing an editor sees reaches the grid.
        event.stop_propagation();
    });

    let (Some(column), Some(column_id)) = (column, column_id) else {
        return rsx! {};
    };
    let Some(text) = grid.editor_text(&column_id) else {
        return rsx! {};
    };
    let error = grid.editor_error(&column_id);
    let label = column.label().to_string();

    if let Some(render) = column.custom_editor() {
        return render(CellEditor {
            text,
            error,
            label,
            in_form,
            id,
            error_id,
            set_text,
            onkeydown,
            onmounted,
        });
    }

    let kind = grid.value_kind(&column_id).unwrap_or(ValueKind::Text);
    let aria_label = (!in_form).then(|| label.clone());
    let invalid = error.is_some().then_some("true");
    let described_by = error.as_ref().map(|_| error_id.clone());
    let spec = column.spec();
    let choices: Vec<(String, String)> = {
        let locale = grid.locale();
        let locale = locale.read();
        spec.choices
            .iter()
            .map(|value| {
                (
                    value.edit_text(),
                    locale.format(&value.as_cell(), &spec.format),
                )
            })
            .collect()
    };

    let input = if !choices.is_empty() {
        let blank = !choices.iter().any(|(value, _)| *value == text);
        rsx! {
            select {
                id: "{id}",
                aria_label,
                aria_invalid: invalid,
                aria_describedby: described_by,
                "data-editor": "",
                onmounted,
                onkeydown,
                onchange: move |event| set_text.call(event.value()),
                ..attributes,
                if blank {
                    option { value: "", selected: true, "" }
                }
                for (value , display) in choices {
                    option {
                        key: "{value}",
                        value: "{value}",
                        selected: value == text,
                        "{display}"
                    }
                }
            }
        }
    } else if kind == ValueKind::Bool {
        rsx! {
            input {
                id: "{id}",
                r#type: "checkbox",
                checked: checked(&text),
                aria_label,
                aria_invalid: invalid,
                aria_describedby: described_by,
                "data-editor": "",
                onmounted,
                onkeydown,
                onchange: move |event| set_text.call(event.checked().to_string()),
                ..attributes,
            }
        }
    } else {
        let (input_type, input_mode) = match kind {
            ValueKind::Date => ("date", None),
            ValueKind::DateTime => ("datetime-local", None),
            ValueKind::Number => ("text", Some("decimal")),
            ValueKind::Text | ValueKind::Bool => ("text", None),
        };
        rsx! {
            input {
                id: "{id}",
                r#type: input_type,
                inputmode: input_mode,
                autocomplete: "off",
                value: "{text}",
                aria_label,
                aria_invalid: invalid,
                aria_describedby: described_by,
                "data-editor": "",
                onmounted,
                onkeydown,
                oninput: move |event| set_text.call(event.value()),
                ..attributes,
            }
        }
    };

    rsx! {
        {input}
        if let Some(error) = error {
            span { id: "{error_id}", role: "alert", "data-edit-error": "", "{error}" }
        }
    }
}

/// A layer over the page behind a modal dialog: `position: fixed` over the
/// viewport, hidden from assistive technology. Clicks on it do nothing, as
/// for a modal they should not.
#[component]
fn Backdrop() -> Element {
    rsx! {
        div {
            "data-edit-backdrop": "",
            aria_hidden: "true",
            style: "position: fixed; inset: 0;",
        }
    }
}

/// Moves focus to `target`, if it is mounted.
fn focus(target: Signal<Option<Rc<MountedData>>>) {
    if let Some(target) = target.peek().clone() {
        spawn(async move {
            let _ = target.set_focus(true).await;
        });
    }
}

/// The form for editing a whole row in a dialog, and for a new row.
///
/// Renders while an edit happens in a form: in [`EditMode::Dialog`], and in
/// every mode for a row started with [`GridHandle::start_create`]. A modal
/// `role="dialog"` with a field per editable visible column, each a
/// `<label>` and a [`GridCellEditor`], then *Save* and *Cancel*.
///
/// Focus starts in the first field and stays inside: tabbing past the last
/// control comes back to the first and the other way round. `Escape` or
/// *Cancel* closes it without saving. Errors appear at their fields; a
/// message for the whole row after them. Afterwards focus returns to the grid.
///
/// Renders a `position: fixed` backdrop (`data-edit-backdrop`) and the
/// dialog (`data-edit-dialog`) with `data-edit-field`, `data-edit-row-error`
/// and `data-edit-actions` inside; position them yourself.
#[component]
pub fn GridEditDialog<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let title_id = use_hook(|| format!("dg-edit-dialog-{}", next_id()));
    let cancel_button = use_signal(|| None::<Rc<MountedData>>);
    let mut cancel_mounted = cancel_button;

    if !grid.edit_target().is_some_and(|target| target.form) {
        return rsx! {};
    }

    let locale = grid.locale();
    let locale = locale.read().clone();
    let title = if grid.is_creating() {
        locale.edit_new_row.to_string()
    } else {
        locale.edit_row.to_string()
    };
    let edited = grid.edited_columns();
    let fields: Vec<(usize, String)> = grid
        .visible_columns()
        .iter()
        .enumerate()
        .filter(|(_, column)| edited.contains(column.id()))
        .map(|(index, column)| (index, column.label().to_string()))
        .collect();
    let first = edited.first().cloned();
    let row_error = grid.edit_row_error();

    rsx! {
        Backdrop {}
        div {
            role: "dialog",
            aria_modal: "true",
            aria_labelledby: "{title_id}",
            "data-edit-dialog": "",
            onkeydown: move |event: KeyboardEvent| {
                if event.key() == Key::Escape {
                    event.prevent_default();
                    grid.cancel_edit();
                }
            },
            ..attributes,
            // Focus guards: tabbing out of either end lands here and is sent
            // round to the other end.
            div {
                tabindex: "0",
                aria_hidden: "true",
                "data-focus-guard": "",
                onfocus: move |_| focus(cancel_button),
            }
            h2 { id: "{title_id}", "{title}" }
            for (index , label) in fields {
                div { key: "{index}", "data-edit-field": "",
                    label { r#for: "{title_id}-{index}", "{label}" }
                    GridCellEditor {
                        grid,
                        column_index: index,
                        in_form: true,
                        input_id: format!("{title_id}-{index}"),
                    }
                }
            }
            if let Some(error) = row_error {
                p { role: "alert", "data-edit-row-error": "", "{error}" }
            }
            div { "data-edit-actions": "",
                button {
                    r#type: "button",
                    "data-edit-save": "",
                    onclick: move |_| {
                        grid.commit_edit(EditMove::Stay);
                    },
                    "{locale.edit_save}"
                }
                button {
                    r#type: "button",
                    "data-edit-cancel": "",
                    onmounted: move |event| cancel_mounted.set(Some(event.data())),
                    onclick: move |_| grid.cancel_edit(),
                    "{locale.edit_cancel}"
                }
            }
            div {
                tabindex: "0",
                aria_hidden: "true",
                "data-focus-guard": "",
                onfocus: move |_| {
                    if let Some(first) = &first {
                        grid.focus_editor(first);
                    }
                },
            }
        }
    }
}

/// Asks before rows are deleted.
///
/// Renders while [`GridHandle::pending_delete`] has rows: a modal
/// `role="alertdialog"` saying how many, with *Delete* and *Keep*. Focus
/// starts on *Keep*, the choice that loses nothing, and stays inside the
/// dialog; `Escape` keeps the rows. Afterwards focus returns to the grid.
///
/// Renders `data-edit-backdrop` and `data-delete-confirm`, with the buttons
/// marked `data-delete` and `data-delete-keep`.
#[component]
pub fn GridDeleteConfirm<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let message_id = use_hook(|| format!("dg-delete-{}", next_id()));
    let mut delete_button = use_signal(|| None::<Rc<MountedData>>);
    let mut keep_button = use_signal(|| None::<Rc<MountedData>>);

    let Some(count) = grid.pending_delete() else {
        return rsx! {};
    };
    let locale = grid.locale();
    let locale = locale.read().clone();

    rsx! {
        Backdrop {}
        div {
            role: "alertdialog",
            aria_modal: "true",
            aria_labelledby: "{message_id}",
            "data-delete-confirm": "",
            onkeydown: move |event: KeyboardEvent| {
                if event.key() == Key::Escape {
                    event.prevent_default();
                    grid.cancel_delete();
                }
            },
            ..attributes,
            div {
                tabindex: "0",
                aria_hidden: "true",
                "data-focus-guard": "",
                onfocus: move |_| focus(keep_button),
            }
            p { id: "{message_id}", "{locale.delete_confirm(count)}" }
            div { "data-edit-actions": "",
                button {
                    r#type: "button",
                    "data-delete": "",
                    onmounted: move |event| delete_button.set(Some(event.data())),
                    onclick: move |_| grid.confirm_delete(),
                    "{locale.edit_delete}"
                }
                button {
                    r#type: "button",
                    "data-delete-keep": "",
                    onmounted: move |event| {
                        let target = event.data();
                        keep_button.set(Some(target.clone()));
                        spawn(async move {
                            let _ = target.set_focus(true).await;
                        });
                    },
                    onclick: move |_| grid.cancel_delete(),
                    "{locale.delete_keep}"
                }
            }
            div {
                tabindex: "0",
                aria_hidden: "true",
                "data-focus-guard": "",
                onfocus: move |_| focus(delete_button),
            }
        }
    }
}

/// Buttons to add, edit and delete rows, and in [`EditMode::Batch`] to save
/// or discard the batch.
///
/// Shows only what the grid's [editing](GridHandle::set_editing) allows:
/// *Add* with a [`new_row`](crate::Editing::new_row), *Delete* with a delete
/// callback, *Edit* with editable columns. *Edit* and *Delete* act on the
/// focused row — *Delete* on the whole selection if the focused row is part
/// of it. While a row is edited inline, *Save* and *Cancel* finish it.
///
/// Each button carries a `data-edit-*` attribute naming its action.
#[component]
pub fn GridEditToolbar<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let Some(mode) = grid.edit_mode() else {
        return rsx! {};
    };
    let locale = grid.locale();
    let locale = locale.read().clone();
    let rows = grid.view().read().indices.len();
    let editable = grid.has_editable_columns() && rows > 0;
    let candidates = grid.delete_candidates();
    let pending = grid.pending_changes();
    let row_editing = grid
        .edit_target()
        .is_some_and(|target| !target.form && target.column.is_none());

    rsx! {
        div { "data-edit-toolbar": "", ..attributes,
            if grid.can_create() {
                button {
                    r#type: "button",
                    "data-edit-add": "",
                    onclick: move |_| {
                        grid.start_create();
                    },
                    "{locale.edit_add}"
                }
            }
            if row_editing {
                button {
                    r#type: "button",
                    "data-edit-save": "",
                    onclick: move |_| {
                        grid.commit_edit(EditMove::Stay);
                    },
                    "{locale.edit_save}"
                }
                button {
                    r#type: "button",
                    "data-edit-cancel": "",
                    onclick: move |_| grid.cancel_edit(),
                    "{locale.edit_cancel}"
                }
            } else if editable {
                button {
                    r#type: "button",
                    "data-edit-start": "",
                    onclick: move |_| {
                        let focus = grid.focus();
                        let row = focus.row.saturating_sub(1);
                        if !grid.start_edit(row, focus.col) {
                            // The focused column cannot be edited: take the
                            // row's first one that can.
                            let first = grid
                                .visible_columns()
                                .iter()
                                .position(|column| column.spec().is_editable());
                            if let Some(column) = first {
                                grid.start_edit(row, column);
                            }
                        }
                    },
                    "{locale.edit_edit}"
                }
            }
            if grid.can_delete() {
                button {
                    r#type: "button",
                    "data-edit-delete": "",
                    disabled: candidates.is_empty(),
                    onclick: move |_| {
                        let keys = grid.delete_candidates();
                        grid.request_delete(&keys);
                    },
                    "{locale.edit_delete}"
                }
            }
            if mode == EditMode::Batch {
                button {
                    r#type: "button",
                    "data-batch-save": "",
                    disabled: pending == 0,
                    onclick: move |_| grid.save_changes(),
                    "{locale.batch_save}"
                }
                button {
                    r#type: "button",
                    "data-batch-discard": "",
                    disabled: pending == 0,
                    onclick: move |_| grid.discard_changes(),
                    "{locale.batch_discard}"
                }
            }
        }
    }
}

/// Where editing stands, as a live region: *Saving…*, *Saved*, why a save
/// failed, why an inline row was refused, and in [`EditMode::Batch`] how many
/// changes are not saved yet.
///
/// Renders its container always, so a screen reader is already listening
/// when the message appears. `data-state` is `idle`, `saving`, `saved`,
/// `failed` or `invalid`.
#[component]
pub fn GridEditStatus<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let locale = grid.locale();
    let locale = locale.read().clone();
    let status = grid.edit_status();
    let inline_error = grid
        .edit_target()
        .filter(|target| !target.form)
        .and_then(|_| grid.edit_row_error());
    let pending = (grid.edit_mode() == Some(EditMode::Batch))
        .then(|| grid.pending_changes())
        .filter(|&count| count > 0);

    let (state, message) = match (&inline_error, &status) {
        (Some(error), _) => ("invalid", error.clone()),
        (None, EditStatus::Saving) => ("saving", locale.edit_saving.to_string()),
        (None, EditStatus::Saved) => ("saved", locale.edit_saved.to_string()),
        (None, EditStatus::Failed(message)) => ("failed", message.clone()),
        (None, EditStatus::Idle) => ("idle", String::new()),
    };

    rsx! {
        div {
            role: "status",
            aria_live: "polite",
            "data-state": state,
            ..attributes,
            if !message.is_empty() {
                span { "data-edit-message": "", "{message}" }
            }
            if let Some(count) = pending {
                span { "data-edit-pending": "", "{locale.batch_pending(count)}" }
            }
        }
    }
}
