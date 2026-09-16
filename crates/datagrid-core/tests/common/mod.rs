//! Shared fixtures for the integration tests.

#![allow(dead_code, clippy::indexing_slicing)]

use datagrid_core::{ColumnSpec, GridRow, View};

/// A row type with one field per interesting column kind: sortable text,
/// sortable numbers, filterable text and an optional value.
#[derive(Clone, Debug, PartialEq)]
pub struct User {
    pub id: u32,
    pub name: String,
    pub email: String,
    pub age: u32,
    pub nickname: Option<String>,
}

impl User {
    pub fn new(id: u32, name: &str, age: u32) -> Self {
        Self {
            id,
            name: name.to_owned(),
            email: format!("{}@example.com", name.to_lowercase()),
            age,
            nickname: None,
        }
    }

    #[must_use]
    pub fn with_nickname(mut self, nickname: &str) -> Self {
        self.nickname = Some(nickname.to_owned());
        self
    }
}

impl GridRow for User {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

/// Five users in a deliberately unsorted order, with ties on `age` so that
/// stability and multi-column sorting are observable.
pub fn sample_rows() -> Vec<User> {
    vec![
        User::new(1, "Zoe", 30),
        User::new(2, "adam", 25),
        User::new(3, "Mia", 30),
        User::new(4, "Carol", 41),
        User::new(5, "bob", 41),
    ]
}

/// The standard column set: name is sortable and filterable, age is sortable,
/// email is filterable only.
pub fn sample_columns() -> Vec<ColumnSpec<User>> {
    vec![
        ColumnSpec::new("name")
            .sort_by(|user: &User| user.name.clone())
            .filter_by(|user: &User| user.name.clone()),
        ColumnSpec::new("age").sort_by(|user: &User| user.age),
        ColumnSpec::new("email").filter_by(|user: &User| user.email.clone()),
    ]
}

/// The names of the rows a view selected, in display order.
pub fn names_of<'a>(rows: &'a [User], view: &View) -> Vec<&'a str> {
    view.indices
        .iter()
        .filter_map(|&index| rows.get(index))
        .map(|user| user.name.as_str())
        .collect()
}

/// The ids of the rows a view selected, in display order.
pub fn ids_of(rows: &[User], view: &View) -> Vec<u32> {
    view.indices
        .iter()
        .filter_map(|&index| rows.get(index))
        .map(|user| user.id)
        .collect()
}
