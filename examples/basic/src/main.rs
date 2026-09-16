//! The smallest useful grid: sorting, filtering, search, paging and selection.
//!
//! Run it with `dx serve --package example-basic` for web, or add
//! `--platform desktop` for a desktop window.

use datagrid_core::{GridRow, SelectionMode};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{GridBody, GridHeader, GridPagination, GridRoot, GridSearch};
use dioxus_datagrid::{Column, GridOptions, use_grid};

const STYLE: Asset = asset!("/assets/grid.css");

fn main() {
    dioxus::launch(App);
}

#[derive(Clone, PartialEq)]
struct User {
    id: u32,
    name: String,
    email: String,
    department: String,
    age: u32,
}

impl GridRow for User {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

/// A small fixed data set, enough to show ties, paging and case-insensitive
/// sorting without needing a server.
fn sample_users() -> Vec<User> {
    const PEOPLE: [(&str, &str, u32); 12] = [
        ("Zoe Bauer", "engineering", 30),
        ("adam Fischer", "sales", 25),
        ("Mia Kaufmann", "engineering", 30),
        ("Carol Weber", "support", 41),
        ("bob Schneider", "finance", 41),
        ("Ingrid Vogel", "engineering", 28),
        ("Hugo Brandt", "legal", 52),
        ("Lena Hoffmann", "support", 34),
        ("Nils Krause", "sales", 45),
        ("Ava Richter", "research", 23),
        ("Theo Lang", "finance", 38),
        ("Yara Nowak", "research", 31),
    ];

    PEOPLE
        .iter()
        .enumerate()
        .map(|(index, (name, department, age))| {
            let local = name.to_lowercase().replace(' ', ".");
            User {
                id: index as u32 + 1,
                name: (*name).to_owned(),
                email: format!("{local}@example.com"),
                department: (*department).to_owned(),
                age: *age,
            }
        })
        .collect()
}

#[component]
fn App() -> Element {
    let users = use_signal(sample_users);

    let columns = use_hook(|| {
        vec![
            Column::new("name", "Name")
                .cell(|user: &User| rsx! { "{user.name}" })
                .sort_by_text(|user: &User| user.name.as_str())
                .filter_by(|user: &User| user.name.clone()),
            Column::new("email", "Email")
                .cell(|user: &User| rsx! { "{user.email}" })
                .sort_by_text(|user: &User| user.email.as_str())
                .filter_by(|user: &User| user.email.clone()),
            Column::new("department", "Department")
                .cell(|user: &User| rsx! { "{user.department}" })
                .sort_by_text(|user: &User| user.department.as_str())
                .filter_by(|user: &User| user.department.clone()),
            Column::new("age", "Age")
                .cell(|user: &User| rsx! { "{user.age}" })
                .sort_by_value(|user: &User| user.age),
        ]
    });

    let grid = use_grid(
        users,
        columns,
        GridOptions::paged(5).selection(SelectionMode::Multi),
    );

    rsx! {
        document::Link { rel: "stylesheet", href: STYLE }

        main { class: "page",
            h1 { "dioxus-datagrid" }
            p { class: "hint",
                "Click a header to sort, shift-click to add a second sort column. "
                "Tab into the grid, then move with the arrow keys; space selects a row."
            }

            div { class: "toolbar",
                GridSearch { grid, class: "search", placeholder: "Search all columns" }
                span { class: "count",
                    "{grid.filtered_len()} of {users.read().len()} rows"
                    if grid.selected_count() > 0 {
                        ", {grid.selected_count()} selected"
                    }
                }
            }

            GridRoot { grid, class: "dg",
                GridHeader { grid, class: "dg-head" }
                GridBody { grid, class: "dg-body" }
            }

            GridPagination { grid, class: "dg-pagination" }
        }
    }
}
