//! What the grid puts on the clipboard: which cells the selection means, and
//! what each one says when it gets there.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{CellFocus, CellFormat, CellSelectionMode, GridLocale, GridRow, SelectionMode};
use dioxus::prelude::*;
use dioxus_datagrid::{Column, GridOptions, use_grid};

#[derive(Clone, PartialEq)]
struct Row {
    id: u32,
    name: String,
    note: String,
    salary: f64,
}

impl GridRow for Row {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

fn rows() -> Vec<Row> {
    vec![
        Row {
            id: 1,
            name: "Zoe".to_owned(),
            note: "plain".to_owned(),
            salary: 1234.5,
        },
        Row {
            id: 2,
            name: "adam".to_owned(),
            // A tab and a quotation mark, which a spreadsheet would misread.
            note: "a\tb \"c\"".to_owned(),
            salary: 2000.0,
        },
        Row {
            id: 3,
            name: "Mia".to_owned(),
            note: "third".to_owned(),
            salary: 3000.0,
        },
    ]
}

fn columns() -> Vec<Column<Row>> {
    vec![
        Column::new("name", "Name").value_text(|row: &Row| row.name.as_str()),
        Column::new("note", "Note").value_text(|row: &Row| row.note.as_str()),
        Column::new("salary", "Salary")
            .value_of(|row: &Row| row.salary)
            .format(CellFormat::currency("€", 2)),
        // Renders something, but has no value: there is no text to copy.
        Column::new("actions", "Actions").cell(|_row: &Row| rsx! { button { "Edit" } }),
    ]
}

#[derive(Clone, PartialEq, Props)]
struct Setup {
    /// A rectangle to select, in focus coordinates — row 0 is the header.
    #[props(default)]
    range: Option<((usize, usize), (usize, usize))>,
    /// Row keys to select, in the order given.
    #[props(default)]
    rows: Vec<u32>,
    /// Where to put the focus, in focus coordinates.
    #[props(default)]
    focus: Option<(usize, usize)>,
    #[props(default)]
    german: bool,
}

#[component]
fn Grid(setup: Setup) -> Element {
    let data = use_signal(rows);
    let cols = use_hook(columns);
    let mut options = GridOptions::default()
        .selection(SelectionMode::Multi)
        .cell_selection(CellSelectionMode::Range);
    if setup.german {
        options.locale = GridLocale::german();
    }
    let grid = use_grid(data, cols, options);

    use_hook(move || {
        let mut grid = grid;
        if let Some((anchor, focus)) = setup.range {
            grid.select_cell(CellFocus::new(anchor.0, anchor.1));
            grid.extend_cell_selection(CellFocus::new(focus.0, focus.1));
        }
        for key in &setup.rows {
            grid.select(*key);
        }
        if let Some((row, col)) = setup.focus {
            grid.set_focus(CellFocus::new(row, col));
        }
    });

    // The text itself is the subject of these tests, so it is what is rendered.
    rsx! {
        pre { {grid.copy_text().unwrap_or_else(|| "<nothing>".to_owned())} }
    }
}

fn copied(setup: Setup) -> String {
    #[component]
    fn Harness(setup: Setup) -> Element {
        rsx! { Grid { setup } }
    }

    let mut dom = VirtualDom::new_with_props(Harness, HarnessProps { setup });
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    unescape(html.trim_start_matches("<pre>").trim_end_matches("</pre>"))
}

/// Undoes the escaping the renderer applies to text, so the test compares the
/// text the grid produced rather than its HTML spelling.
fn unescape(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("&#") {
        text.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find(';').unwrap();
        let code: u32 = after[..end].parse().unwrap();
        text.push(char::from_u32(code).unwrap());
        rest = &after[end + 1..];
    }
    text.push_str(rest);
    text.replace("&amp;", "&")
}

#[test]
fn a_rectangle_copies_its_cells() {
    // The first two rows, Name and Note.
    let text = copied(Setup::builder().range(Some(((1, 0), (2, 1)))).build());

    // The tab and the quotation marks are quoted, so the block stays 2 by 2.
    assert_eq!(text, "Zoe\tplain\r\nadam\t\"a\tb \"\"c\"\"\"");
}

#[test]
fn a_rectangle_copies_only_the_columns_it_covers() {
    let text = copied(Setup::builder().range(Some(((1, 2), (3, 2)))).build());

    assert_eq!(text, "€1,234.50\r\n€2,000.00\r\n€3,000.00");
}

#[test]
fn a_cell_copies_what_the_grid_shows_it_as() {
    let english = copied(Setup::builder().range(Some(((1, 2), (1, 2)))).build());
    let german = copied(
        Setup::builder()
            .range(Some(((1, 2), (1, 2))))
            .german(true)
            .build(),
    );

    // Formatted by the grid's locale, not the raw number.
    assert_eq!(english, "€1,234.50");
    // A non-breaking space before the sign, as the German format has it.
    assert_eq!(german, "1.234,50\u{a0}€");
}

#[test]
fn selected_rows_copy_every_visible_column_in_view_order() {
    // Picked bottom first; copied top first, as they are shown.
    let text = copied(Setup::builder().rows(vec![3, 1]).build());

    assert_eq!(text, "Zoe\tplain\t€1,234.50\t\r\nMia\tthird\t€3,000.00\t");
}

#[test]
fn a_column_with_no_value_copies_an_empty_cell() {
    let text = copied(Setup::builder().rows(vec![1]).build());

    // Four columns, so three tabs; the last one is the Actions column.
    assert_eq!(text.matches('\t').count(), 3);
    assert!(text.ends_with('\t'), "{text}");
}

#[test]
fn a_rectangle_wins_over_the_selected_rows() {
    let text = copied(
        Setup::builder()
            .range(Some(((1, 0), (1, 0))))
            .rows(vec![1, 2, 3])
            .build(),
    );

    assert_eq!(text, "Zoe");
}

#[test]
fn with_nothing_selected_the_focused_cell_is_copied() {
    let text = copied(Setup::builder().focus(Some((2, 1))).build());

    assert_eq!(text, "a\tb \"c\"", "one cell needs no quoting of its own");
}

#[test]
fn a_focus_on_the_header_copies_nothing() {
    let text = copied(Setup::builder().focus(Some((0, 0))).build());

    assert_eq!(text, "<nothing>");
}
