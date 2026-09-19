//! Editing: text read as a column's kind of value and written into a row,
//! validation, and batches of unsaved changes.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{
    Changes, ColumnId, ColumnSpec, EditError, FromValue, GridLocale, GridRow, Value, ValueKind,
    changed_columns,
};
use proptest::prelude::*;

#[derive(Clone, Debug, PartialEq)]
struct User {
    id: u32,
    name: String,
    age: u32,
    score: Option<f64>,
    active: bool,
    team: String,
}

impl GridRow for User {
    type Key = u32;
    fn key(&self) -> u32 {
        self.id
    }
}

fn user(id: u32) -> User {
    User {
        id,
        name: format!("User {id}"),
        age: 30,
        score: Some(1.5),
        active: true,
        team: "Red".into(),
    }
}

fn columns() -> Vec<ColumnSpec<User>> {
    vec![
        ColumnSpec::new("name")
            .value_text(|user: &User| user.name.as_str())
            .editable(|user: &mut User, name: String| user.name = name)
            .validate(|user: &User| {
                if user.name.trim().is_empty() {
                    Err("A name is required".into())
                } else {
                    Ok(())
                }
            }),
        ColumnSpec::new("age")
            .value_of(|user: &User| user.age)
            .editable(|user: &mut User, age: u32| user.age = age),
        ColumnSpec::new("score")
            .value_of(|user: &User| user.score)
            .editable(|user: &mut User, score: Option<f64>| user.score = score),
        ColumnSpec::new("active")
            .value_of(|user: &User| user.active)
            .editable(|user: &mut User, active: bool| user.active = active),
        ColumnSpec::new("team")
            .value_text(|user: &User| user.team.as_str())
            .editable(|user: &mut User, team: String| user.team = team)
            .choices(["Red", "Blue"]),
        ColumnSpec::new("id").value_of(|user: &User| user.id),
    ]
}

fn column(id: &str) -> ColumnSpec<User> {
    columns()
        .into_iter()
        .find(|column| column.id == id)
        .unwrap()
}

fn edit(id: &str, text: &str, kind: ValueKind) -> Result<User, EditError> {
    let mut row = user(1);
    column(id).apply_edit(&mut row, text, kind).map(|()| row)
}

#[test]
fn text_is_read_as_the_columns_kind() {
    assert_eq!(edit("age", "41", ValueKind::Number).unwrap().age, 41);
    assert_eq!(edit("age", " 41 ", ValueKind::Number).unwrap().age, 41);
    assert_eq!(
        edit("score", "2,25", ValueKind::Number).unwrap().score,
        Some(2.25)
    );
    assert_eq!(
        edit("score", "3", ValueKind::Number).unwrap().score,
        Some(3.0)
    );
    assert!(!edit("active", "nein", ValueKind::Bool).unwrap().active);
    assert_eq!(
        edit("name", " Ada ", ValueKind::Text).unwrap().name,
        " Ada "
    );
}

#[test]
fn bad_text_is_refused_with_a_reason() {
    assert_eq!(
        edit("age", "old", ValueKind::Number),
        Err(EditError::Invalid(ValueKind::Number))
    );
    // Out of range for u32, or not a whole number.
    assert_eq!(
        edit("age", "-1", ValueKind::Number),
        Err(EditError::Invalid(ValueKind::Number))
    );
    assert_eq!(
        edit("age", "2.5", ValueKind::Number),
        Err(EditError::Invalid(ValueKind::Number))
    );
    assert_eq!(
        edit("active", "maybe", ValueKind::Bool),
        Err(EditError::Invalid(ValueKind::Bool))
    );
}

#[test]
fn only_optional_fields_take_an_empty_cell() {
    assert_eq!(
        edit("age", "  ", ValueKind::Number),
        Err(EditError::Required)
    );
    assert_eq!(edit("score", "", ValueKind::Number).unwrap().score, None);
    // Text has an empty value of its own; the validator decides.
    assert_eq!(
        edit("name", "", ValueKind::Text),
        Err(EditError::Message("A name is required".into()))
    );
}

#[test]
fn choices_limit_the_values() {
    assert_eq!(edit("team", "Blue", ValueKind::Text).unwrap().team, "Blue");
    assert_eq!(
        edit("team", "Green", ValueKind::Text),
        Err(EditError::NotAChoice)
    );
}

#[test]
fn a_column_without_a_setter_is_read_only() {
    let id = column("id");
    assert!(!id.is_editable());
    assert!(column("name").is_editable());
    let mut row = user(1);
    assert!(id.apply_edit(&mut row, "7", ValueKind::Number).is_err());
    assert_eq!(row.id, 1);
}

#[test]
fn edit_text_reads_back_through_apply_edit() {
    let english = GridLocale::english();
    let german = GridLocale::german();
    let row = user(1);
    assert_eq!(column("score").edit_text(&row, &english), "1.5");
    assert_eq!(column("score").edit_text(&row, &german), "1,5");
    assert_eq!(column("age").edit_text(&row, &german), "30");
    let mut empty = row.clone();
    empty.score = None;
    assert_eq!(column("score").edit_text(&empty, &english), "");

    // What an editor starts with is accepted unchanged, in either locale.
    for locale in [&english, &german] {
        for spec in columns().iter().filter(|spec| spec.is_editable()) {
            let kind = spec.value_kind(std::slice::from_ref(&row)).unwrap();
            let mut copy = row.clone();
            spec.apply_edit(&mut copy, &spec.edit_text(&row, locale), kind)
                .unwrap();
            assert_eq!(copy, row, "{}", spec.id);
        }
    }
}

#[test]
fn errors_speak_the_locale() {
    let german = GridLocale::german();
    assert_eq!(
        EditError::Invalid(ValueKind::Number).message(&german),
        "Bitte eine Zahl eingeben"
    );
    assert_eq!(
        EditError::Required.message(&german),
        "Bitte einen Wert eingeben"
    );
    assert_eq!(EditError::from("Too old").message(&german), "Too old");
}

#[test]
fn from_value_converts_the_field_types() {
    assert_eq!(
        i8::from_value(Some(Value::Int(300))),
        Err(EditError::Invalid(ValueKind::Number))
    );
    assert_eq!(u64::from_value(Some(Value::Float(4.0))), Ok(4));
    assert_eq!(f32::from_value(Some(Value::Text("1,5".into()))), Ok(1.5));
    assert_eq!(Option::<i32>::from_value(None), Ok(None));
    assert_eq!(String::from_value(Some(Value::Int(3))), Ok("3".into()));
    assert_eq!(bool::from_value(None), Err(EditError::Required));
}

#[cfg(feature = "chrono")]
#[test]
fn dates_are_edited_in_iso_and_local_forms() {
    use chrono::NaiveDate;
    let date = NaiveDate::from_ymd_opt(2026, 9, 18).unwrap();
    assert_eq!(
        NaiveDate::from_value(Some(Value::Text("2026-09-18".into()))),
        Ok(date)
    );
    assert_eq!(
        NaiveDate::from_value(Some(Value::Text("18.09.2026".into()))),
        Ok(date)
    );
    assert_eq!(
        NaiveDate::from_value(Some(Value::Text("someday".into()))),
        Err(EditError::Invalid(ValueKind::Date))
    );
}

#[test]
fn changed_columns_compares_values() {
    let before = user(1);
    let mut after = before.clone();
    after.age = 31;
    after.score = None;
    let changed = changed_columns(&columns(), &before, &after);
    assert_eq!(changed, [ColumnId::from("age"), ColumnId::from("score")]);
    assert!(changed_columns(&columns(), &before, &before).is_empty());
}

#[test]
fn changes_keep_one_entry_per_row() {
    let mut changes = Changes::new();
    let original = user(1);
    let mut first = original.clone();
    first.age = 40;
    let mut second = first.clone();
    second.name = "Ada".into();

    changes.update(original.clone(), first.clone());
    changes.update(first, second.clone());
    assert_eq!(changes.updated, vec![(original.clone(), second.clone())]);
    assert_eq!(changes.current(&1), Some(&second));
    assert_eq!(changes.original(&1), Some(&original));
    assert_eq!(
        changes.changed_columns(&columns(), &1),
        [ColumnId::from("name"), ColumnId::from("age")]
    );

    // Edited back to how it was: nothing to save.
    changes.update(second, original.clone());
    assert!(changes.is_empty());
}

#[test]
fn deleting_an_edited_row_deletes_the_original() {
    let mut changes = Changes::new();
    let original = user(1);
    let mut edited = original.clone();
    edited.age = 50;
    changes.update(original.clone(), edited.clone());
    changes.delete(edited);
    assert!(changes.updated.is_empty());
    assert_eq!(changes.deleted, vec![original]);
    assert!(changes.is_deleted(&1));
    changes.restore(&1);
    assert!(changes.is_empty());
}

#[test]
fn an_added_row_that_is_deleted_leaves_no_trace() {
    let mut changes = Changes::new();
    changes.add(user(9));
    let mut edited = user(9);
    edited.age = 12;
    changes.update(user(9), edited.clone());
    assert_eq!(changes.added, vec![edited.clone()]);
    assert!(changes.updated.is_empty());
    changes.delete(edited);
    assert!(changes.is_empty());
}

#[derive(Clone, Debug)]
enum Step {
    Update(u32, u32),
    Delete(u32),
    Restore(u32),
}

fn arb_steps() -> impl Strategy<Value = Vec<Step>> {
    prop::collection::vec(
        prop_oneof![
            (0_u32..4, 28_u32..33).prop_map(|(id, age)| Step::Update(id, age)),
            (0_u32..4).prop_map(Step::Delete),
            (0_u32..4).prop_map(Step::Restore),
        ],
        0..20,
    )
}

proptest! {
    /// Whatever the order of edits, deletions and restores, the changes
    /// describe exactly the difference between the rows as they were and as
    /// they are now.
    #[test]
    fn changes_match_the_difference(steps in arb_steps()) {
        let originals: Vec<User> = (0..4).map(user).collect();
        let mut rows = originals.clone();
        let mut deleted = [false; 4];
        let mut changes = Changes::new();

        for step in steps {
            match step {
                Step::Update(id, age) if !deleted[id as usize] => {
                    let before = rows[id as usize].clone();
                    rows[id as usize].age = age;
                    changes.update(before, rows[id as usize].clone());
                }
                Step::Update(..) => {}
                Step::Delete(id) if !deleted[id as usize] => {
                    deleted[id as usize] = true;
                    changes.delete(rows[id as usize].clone());
                }
                Step::Delete(_) => {}
                Step::Restore(id) if deleted[id as usize] => {
                    // A restored row comes back as it was before any change.
                    deleted[id as usize] = false;
                    rows[id as usize] = originals[id as usize].clone();
                    changes.restore(&id);
                }
                Step::Restore(_) => {}
            }
        }

        for id in 0..4_u32 {
            let index = id as usize;
            prop_assert_eq!(changes.is_deleted(&id), deleted[index]);
            let changed = !deleted[index] && rows[index] != originals[index];
            prop_assert_eq!(changes.current(&id).is_some(), changed);
            if changed {
                prop_assert_eq!(changes.current(&id), Some(&rows[index]));
                prop_assert_eq!(changes.original(&id), Some(&originals[index]));
            }
        }
        prop_assert!(changes.added.is_empty());
    }
}
