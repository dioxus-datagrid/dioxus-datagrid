//! Editing through the handle, as rendered: cell, row, dialog and batch edits,
//! validation, deleting, and a failed save that has to be taken back.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{DataSource, GridQuery, GridRow, Page};
use dioxus::core::NoOpMutations;
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{
    GridBody, GridDeleteConfirm, GridEditDialog, GridEditStatus, GridHeader, GridRoot,
};
use dioxus_datagrid::{
    Column, Create, Delete, EditMode, EditMove, EditStatus, Editing, GridHandle, GridOptions, Save,
    SaveBatch, use_grid, use_grid_remote,
};
use std::cell::RefCell;
use std::time::Duration;
use tokio::time::{Instant, sleep, sleep_until};

#[derive(Clone, Debug, PartialEq)]
struct User {
    id: u32,
    name: String,
    age: u32,
}

impl GridRow for User {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

fn users() -> Vec<User> {
    [("Ada", 36), ("Ben", 41), ("Cleo", 29)]
        .into_iter()
        .zip(1..)
        .map(|((name, age), id)| User {
            id,
            name: name.to_owned(),
            age,
        })
        .collect()
}

fn columns() -> Vec<Column<User>> {
    vec![
        Column::new("id", "Id").value_of(|user: &User| user.id),
        Column::new("name", "Name")
            .value_text(|user: &User| user.name.as_str())
            .editable(|user: &mut User, name: String| user.name = name)
            .validate(|user: &User| {
                if user.name.trim().is_empty() {
                    Err("A name is required".into())
                } else {
                    Ok(())
                }
            }),
        Column::new("age", "Age")
            .value_of(|user: &User| user.age)
            .editable(|user: &mut User, age: u32| user.age = age),
    ]
}

thread_local! {
    static GRID: RefCell<Option<GridHandle<User>>> = const { RefCell::new(None) };
    static CALLS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn log(call: String) {
    CALLS.with(|calls| calls.borrow_mut().push(call));
}

fn calls() -> Vec<String> {
    CALLS.with(|calls| calls.borrow().clone())
}

/// Runs `act` on the grid inside the dom's runtime, then renders.
fn act<R>(dom: &mut VirtualDom, act: impl FnOnce(&mut GridHandle<User>) -> R) -> R {
    let result = dom.in_scope(ScopeId::ROOT, || {
        GRID.with(|grid| act(grid.borrow_mut().as_mut().unwrap()))
    });
    dom.render_immediate(&mut NoOpMutations);
    result
}

async fn settle(dom: &mut VirtualDom) -> String {
    let deadline = Instant::now() + Duration::from_millis(100);
    loop {
        tokio::select! {
            () = dom.wait_for_work() => dom.render_immediate(&mut NoOpMutations),
            () = sleep_until(deadline) => break,
        }
    }
    dioxus_ssr::render(dom)
}

#[derive(Clone, Copy, PartialEq, Props)]
struct LocalProps {
    mode: EditMode,
    /// Whether saving an edited row fails.
    fail: bool,
}

/// A local grid whose callbacks write the data, or fail.
#[component]
fn Local(props: LocalProps) -> Element {
    let LocalProps { mode, fail } = props;
    let mut rows = use_signal(users);
    let cols = use_hook(columns);
    let mut grid = use_grid(rows, cols, GridOptions::default());
    let editing = use_hook(move || Editing {
        mode,
        on_save: Some(EventHandler::new(move |save: Save<User>| {
            log(format!("save {} {}", save.row().name, save.row().age));
            if fail {
                save.fail("Disk full");
                return;
            }
            let row = save.row().clone();
            rows.with_mut(|rows| {
                if let Some(slot) = rows.iter_mut().find(|slot| slot.id == row.id) {
                    *slot = row;
                }
            });
        })),
        on_create: Some(EventHandler::new(move |create: Create<User>| {
            log(format!("create {}", create.row().name));
            rows.push(create.row().clone());
        })),
        on_delete: Some(EventHandler::new(move |delete: Delete<User>| {
            let keys: Vec<u32> = delete.rows().iter().map(|row| row.id).collect();
            log(format!("delete {keys:?}"));
            rows.retain(|row| !keys.contains(&row.id));
        })),
        on_batch_save: Some(EventHandler::new(move |batch: SaveBatch<User>| {
            let changes = batch.changes();
            log(format!(
                "batch {} {} {}",
                changes.updated.len(),
                changes.added.len(),
                changes.deleted.len()
            ));
        })),
        new_row: Some(Callback::new(|()| User {
            id: 99,
            name: String::new(),
            age: 18,
        })),
        ..Editing::default()
    });
    grid.set_editing(editing);
    use_hook(move || GRID.with(|slot| *slot.borrow_mut() = Some(grid)));

    rsx! {
        GridEditStatus { grid }
        GridRoot { grid,
            GridHeader { grid }
            GridBody { grid }
        }
        GridEditDialog { grid }
        GridDeleteConfirm { grid }
    }
}

fn local(mode: EditMode, fail: bool) -> VirtualDom {
    CALLS.with(|calls| calls.borrow_mut().clear());
    let mut dom = VirtualDom::new_with_props(Local, LocalProps { mode, fail });
    dom.rebuild_in_place();
    dom
}

fn html(dom: &VirtualDom) -> String {
    dioxus_ssr::render(dom)
}

#[test]
fn a_cell_edit_saves_through_the_callback_and_shows_the_new_value() {
    let mut dom = local(EditMode::Cell, false);
    // Row 0 is Ada; column 2 is the age.
    assert!(act(&mut dom, |grid| grid.start_edit(0, 2)));
    let editing = html(&dom);
    assert!(editing.contains(r#"data-editing="true""#), "{editing}");
    assert!(editing.contains(r#"aria-label="Age""#), "{editing}");
    assert!(editing.contains(r#"inputmode="decimal""#), "{editing}");
    assert!(editing.contains(r#"value="36""#), "{editing}");

    act(&mut dom, |grid| grid.set_editor_text(&"age".into(), "37"));
    assert!(act(&mut dom, |grid| grid.commit_edit(EditMove::Down)));

    assert_eq!(calls(), ["save Ada 37"]);
    let saved = html(&dom);
    assert!(!saved.contains("data-editor"), "{saved}");
    assert!(saved.contains(">37<"), "{saved}");
    assert_eq!(act(&mut dom, |grid| grid.edit_status()), EditStatus::Saved);
    // Enter moved focus down a row.
    assert_eq!(act(&mut dom, |grid| grid.focus().row), 2);
}

#[test]
fn a_refused_value_keeps_the_editor_open_with_its_message() {
    let mut dom = local(EditMode::Cell, false);
    act(&mut dom, |grid| grid.start_edit(0, 2));
    act(&mut dom, |grid| grid.set_editor_text(&"age".into(), "old"));
    assert!(!act(&mut dom, |grid| grid.commit_edit(EditMove::Stay)));

    let refused = html(&dom);
    assert!(refused.contains(r#"aria-invalid="true""#), "{refused}");
    assert!(refused.contains(r#"role="alert""#), "{refused}");
    assert!(refused.contains("Enter a number"), "{refused}");
    assert!(refused.contains("aria-describedby"), "{refused}");
    assert!(calls().is_empty());

    // Typing again clears the message; Escape-style cancel restores the cell.
    act(&mut dom, |grid| grid.set_editor_text(&"age".into(), "4"));
    assert!(!html(&dom).contains("Enter a number"));
    act(&mut dom, GridHandle::cancel_edit);
    let cancelled = html(&dom);
    assert!(!cancelled.contains("data-editor"), "{cancelled}");
    assert!(cancelled.contains(">36<"), "{cancelled}");
}

#[test]
fn a_column_validator_refuses_an_empty_name() {
    let mut dom = local(EditMode::Cell, false);
    act(&mut dom, |grid| grid.start_edit(1, 1));
    act(&mut dom, |grid| grid.set_editor_text(&"name".into(), "  "));
    assert!(!act(&mut dom, |grid| grid.commit_edit(EditMove::Stay)));
    assert!(html(&dom).contains("A name is required"));
}

#[test]
fn a_read_only_column_cannot_be_edited_and_says_so() {
    let mut dom = local(EditMode::Cell, false);
    assert!(!act(&mut dom, |grid| grid.start_edit(0, 0)));
    let rendered = html(&dom);
    assert_eq!(
        rendered.matches(r#"aria-readonly="true""#).count(),
        3,
        "one per row in the id column: {rendered}"
    );
}

#[test]
fn a_failed_save_puts_the_old_value_back_and_says_why() {
    let mut dom = local(EditMode::Cell, true);
    act(&mut dom, |grid| grid.start_edit(0, 2));
    act(&mut dom, |grid| grid.set_editor_text(&"age".into(), "37"));
    act(&mut dom, |grid| grid.commit_edit(EditMove::Stay));

    let rendered = html(&dom);
    assert!(rendered.contains(">36<"), "{rendered}");
    assert!(!rendered.contains(">37<"), "{rendered}");
    assert!(rendered.contains("Could not save: Disk full"), "{rendered}");
    assert!(rendered.contains(r#"data-state="failed""#), "{rendered}");
}

#[test]
fn tab_commits_and_edits_the_next_editable_cell() {
    let mut dom = local(EditMode::Cell, false);
    act(&mut dom, |grid| grid.start_edit(0, 1));
    act(&mut dom, |grid| {
        grid.set_editor_text(&"name".into(), "Ada L.")
    });
    assert!(act(&mut dom, |grid| grid.commit_edit(EditMove::Next)));
    let target = act(&mut dom, |grid| grid.edit_target()).unwrap();
    assert_eq!(
        (target.row_index, target.column),
        (Some(0), Some("age".into()))
    );

    // From the last editable cell of a row, on to the next row's first.
    assert!(act(&mut dom, |grid| grid.commit_edit(EditMove::Next)));
    let target = act(&mut dom, |grid| grid.edit_target()).unwrap();
    assert_eq!(
        (target.row_index, target.column),
        (Some(1), Some("name".into()))
    );
    assert_eq!(calls(), ["save Ada L. 36"]);
}

#[test]
fn a_row_edit_shows_every_editable_cell_and_saves_once() {
    let mut dom = local(EditMode::Row, false);
    assert!(act(&mut dom, |grid| grid.start_edit(2, 0)));
    let editing = html(&dom);
    assert_eq!(editing.matches("data-editor").count(), 2, "{editing}");
    assert!(editing.contains(r#"value="Cleo""#), "{editing}");

    act(&mut dom, |grid| {
        grid.set_editor_text(&"name".into(), "Clea");
        grid.set_editor_text(&"age".into(), "30");
    });
    assert!(act(&mut dom, |grid| grid.commit_edit(EditMove::Stay)));
    assert_eq!(calls(), ["save Clea 30"]);
}

#[test]
fn an_unchanged_row_is_not_saved() {
    let mut dom = local(EditMode::Row, false);
    act(&mut dom, |grid| grid.start_edit(0, 1));
    assert!(act(&mut dom, |grid| grid.commit_edit(EditMove::Stay)));
    assert!(calls().is_empty());
}

#[test]
fn the_dialog_is_a_labelled_modal_form() {
    let mut dom = local(EditMode::Dialog, false);
    act(&mut dom, |grid| grid.start_edit(1, 2));
    let open = html(&dom);
    assert!(open.contains(r#"role="dialog""#), "{open}");
    assert!(open.contains(r#"aria-modal="true""#), "{open}");
    assert!(open.contains("Edit row"), "{open}");
    // Fields are named by labels, not aria-label.
    assert!(open.contains(r#"<label for=""#), "{open}");
    assert!(!open.contains(r#"aria-label="Age""#), "{open}");
    // The cells themselves stay as they are.
    assert!(!open.contains(r#"data-editing="true""#), "{open}");

    act(&mut dom, |grid| grid.set_editor_text(&"age".into(), "42"));
    act(&mut dom, |grid| grid.commit_edit(EditMove::Stay));
    assert_eq!(calls(), ["save Ben 42"]);
    assert!(!html(&dom).contains(r#"role="dialog""#));
}

#[test]
fn a_new_row_is_checked_in_full_and_created() {
    let mut dom = local(EditMode::Cell, false);
    assert!(act(&mut dom, GridHandle::start_create));
    let open = html(&dom);
    assert!(open.contains("New row"), "{open}");

    // The empty name from `new_row` fails its validator.
    assert!(!act(&mut dom, |grid| grid.commit_edit(EditMove::Stay)));
    let refused = html(&dom);
    assert!(refused.contains("A name is required"), "{refused}");
    assert!(
        refused.contains("Please correct the marked fields"),
        "{refused}"
    );

    act(&mut dom, |grid| {
        grid.set_editor_text(&"name".into(), "Dora")
    });
    assert!(act(&mut dom, |grid| grid.commit_edit(EditMove::Stay)));
    assert_eq!(calls(), ["create Dora"]);
    assert!(html(&dom).contains("Dora"));
}

#[test]
fn deleting_asks_first_and_then_calls_back() {
    let mut dom = local(EditMode::Cell, false);
    act(&mut dom, |grid| grid.request_delete(&[2]));
    let asking = html(&dom);
    assert!(asking.contains(r#"role="alertdialog""#), "{asking}");
    assert!(asking.contains("Delete this row?"), "{asking}");
    assert!(calls().is_empty());

    act(&mut dom, GridHandle::cancel_delete);
    assert!(!html(&dom).contains("alertdialog"));

    act(&mut dom, |grid| grid.request_delete(&[1, 2]));
    assert!(html(&dom).contains("Delete 2 rows?"));
    act(&mut dom, GridHandle::confirm_delete);
    assert_eq!(calls(), ["delete [1, 2]"]);
    let after = html(&dom);
    assert!(!after.contains("Ben"), "{after}");
    assert!(after.contains("Cleo"), "{after}");
}

#[test]
fn a_batch_collects_changes_until_saved_or_discarded() {
    let mut dom = local(EditMode::Batch, false);
    act(&mut dom, |grid| grid.start_edit(0, 2));
    act(&mut dom, |grid| grid.set_editor_text(&"age".into(), "50"));
    act(&mut dom, |grid| grid.commit_edit(EditMove::Stay));
    act(&mut dom, |grid| grid.request_delete(&[3]));
    act(&mut dom, GridHandle::confirm_delete);

    let pending = html(&dom);
    assert!(calls().is_empty());
    assert!(pending.contains(">50<"), "{pending}");
    assert_eq!(
        pending.matches(r#"data-changed="true""#).count(),
        1,
        "{pending}"
    );
    assert!(pending.contains(r#"data-deleted="true""#), "{pending}");
    assert!(pending.contains("2 unsaved changes"), "{pending}");
    // A row marked deleted cannot be edited.
    assert!(!act(&mut dom, |grid| grid.start_edit(2, 1)));

    act(&mut dom, GridHandle::save_changes);
    assert_eq!(calls(), ["batch 1 0 1"]);
    assert_eq!(act(&mut dom, |grid| grid.pending_changes()), 0);

    act(&mut dom, |grid| grid.start_edit(1, 2));
    act(&mut dom, |grid| grid.set_editor_text(&"age".into(), "1"));
    act(&mut dom, |grid| grid.commit_edit(EditMove::Stay));
    act(&mut dom, GridHandle::discard_changes);
    let discarded = html(&dom);
    assert!(discarded.contains(">41<"), "{discarded}");
    assert!(!discarded.contains("data-changed"), "{discarded}");
}

#[test]
fn the_edit_follows_its_row_when_the_order_changes() {
    let mut dom = local(EditMode::Cell, false);
    act(&mut dom, |grid| grid.start_edit(0, 1));
    // Sort by age, descending: Ada (36) moves from the first row to the second.
    act(&mut dom, |grid| {
        grid.toggle_sort("age", false);
        grid.toggle_sort("age", false);
    });
    let target = act(&mut dom, |grid| grid.edit_target()).unwrap();
    assert_eq!(target.row_index, Some(1));
    assert!(html(&dom).contains(r#"value="Ada""#));
}

/// A server whose saves take a while and then fail.
#[derive(Clone, Copy)]
struct Server;

impl DataSource<User> for Server {
    type Error = String;

    async fn fetch(&self, _query: GridQuery) -> Result<Page<User>, String> {
        Ok(Page::new(users(), 3))
    }
}

#[component]
fn Remote() -> Element {
    let cols = use_hook(columns);
    let mut grid = use_grid_remote(Server, cols, GridOptions::paged(10));
    let editing = use_hook(|| Editing {
        on_save: Some(EventHandler::new(|save: Save<User>| async move {
            sleep(Duration::from_millis(20)).await;
            save.fail("Salary band exceeded");
        })),
        ..Editing::default()
    });
    grid.set_editing(editing);
    use_hook(move || GRID.with(|slot| *slot.borrow_mut() = Some(grid)));

    rsx! {
        GridEditStatus { grid }
        GridRoot { grid,
            GridBody { grid }
        }
    }
}

#[tokio::test(start_paused = true)]
async fn a_failed_remote_save_shows_the_edit_until_it_fails_then_the_original() {
    let mut dom = VirtualDom::new(Remote);
    dom.rebuild_in_place();
    let loaded = settle(&mut dom).await;
    assert!(loaded.contains(">41<"), "{loaded}");

    act(&mut dom, |grid| grid.start_edit(1, 2));
    act(&mut dom, |grid| grid.set_editor_text(&"age".into(), "99"));
    act(&mut dom, |grid| grid.commit_edit(EditMove::Stay));

    // Shown at once, before the server answered.
    let optimistic = html(&dom);
    assert!(optimistic.contains(">99<"), "{optimistic}");
    assert!(optimistic.contains(r#"data-saving="true""#), "{optimistic}");
    assert!(optimistic.contains("Saving…"), "{optimistic}");

    let failed = settle(&mut dom).await;
    assert!(failed.contains(">41<"), "{failed}");
    assert!(!failed.contains(">99<"), "{failed}");
    assert!(!failed.contains("data-saving"), "{failed}");
    assert!(
        failed.contains("Could not save: Salary band exceeded"),
        "{failed}"
    );
}
