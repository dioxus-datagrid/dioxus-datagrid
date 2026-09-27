//! Pasting: where a block of tab-separated text lands, what it writes, and what
//! it refuses.
//!
//! The clipboard is out of reach of a test harness — and of the grid itself,
//! which is why pasting starts from text the browser handed over
//! (`docs/DECISIONS.md` ADR-0033). These tests hand it over directly.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{CellFocus, CellSelectionMode, GridRow, SelectionMode};
use dioxus::prelude::*;
use dioxus_datagrid::{
    Column, EditMode, Editing, GridOptions, PasteReport, Save, SaveBatch, use_grid,
};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
struct Row {
    id: u32,
    name: String,
    department: String,
    age: u32,
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
            department: "engineering".to_owned(),
            age: 30,
        },
        Row {
            id: 2,
            name: "Adam".to_owned(),
            department: "sales".to_owned(),
            age: 41,
        },
        Row {
            id: 3,
            name: "Mia".to_owned(),
            department: "engineering".to_owned(),
            age: 28,
        },
    ]
}

fn columns() -> Vec<Column<Row>> {
    vec![
        Column::new("name", "Name")
            .value_text(|row: &Row| row.name.as_str())
            .editable(|row: &mut Row, name: String| row.name = name),
        // Not editable, and in the middle: a block pasted across it keeps its
        // shape.
        Column::new("department", "Department").value_text(|row: &Row| row.department.as_str()),
        Column::new("age", "Age")
            .value_of(|row: &Row| row.age)
            .editable(|row: &mut Row, age: u32| row.age = age),
    ]
}

/// What a run of the harness collects outside the `VirtualDom`.
type Cell<V> = Rc<RefCell<V>>;

#[derive(Clone, PartialEq, Props)]
struct Setup {
    /// The text to paste.
    text: String,
    /// Where the focus is, in focus coordinates — row 0 is the header.
    #[props(default)]
    focus: Option<(usize, usize)>,
    /// A rectangle to select first, in focus coordinates.
    #[props(default)]
    range: Option<((usize, usize), (usize, usize))>,
    #[props(default = EditMode::Cell)]
    mode: EditMode,
    /// Refuses a row without a name, as an application's own check would.
    #[props(default)]
    validate_row: bool,
    /// Starts an edit of the first cell before pasting.
    #[props(default)]
    editing: bool,
}

/// Pastes into a grid and renders what came of it.
#[component]
fn Grid(setup: Setup, saved: Cell<Vec<Row>>, report: Cell<PasteReport>) -> Element {
    let mut data = use_signal(rows);
    let cols = use_hook(columns);
    let mut grid = use_grid(
        data,
        cols,
        GridOptions::default()
            .selection(SelectionMode::Multi)
            .cell_selection(CellSelectionMode::Range),
    );

    grid.set_editing(Editing {
        mode: setup.mode,
        // A local grid's own save: the application stores the row, which is what
        // makes the grid show it. Without that the grid would take the edit back.
        on_save: Some(EventHandler::new({
            let saved = saved.clone();
            move |save: Save<Row>| {
                let row = save.row().clone();
                saved.borrow_mut().push(row.clone());
                let mut stored = data.write();
                if let Some(slot) = stored.iter_mut().find(|other| other.key() == row.key()) {
                    *slot = row;
                }
            }
        })),
        on_batch_save: Some(EventHandler::new({
            let saved = saved.clone();
            move |batch: SaveBatch<Row>| {
                for (_, current) in &batch.changes().updated {
                    saved.borrow_mut().push(current.clone());
                }
            }
        })),
        validate_row: setup.validate_row.then(|| {
            Callback::new(|row: Row| {
                if row.name.trim().is_empty() {
                    Err("A name is required".to_owned())
                } else {
                    Ok(())
                }
            })
        }),
        ..Editing::default()
    });

    use_hook({
        let report = report.clone();
        let setup = setup.clone();
        move || {
            let mut grid = grid;
            if let Some((anchor, focus)) = setup.range {
                grid.select_cell(CellFocus::new(anchor.0, anchor.1));
                grid.extend_cell_selection(CellFocus::new(focus.0, focus.1));
            }
            if let Some((row, col)) = setup.focus {
                grid.set_focus(CellFocus::new(row, col));
            }
            if setup.editing {
                grid.start_edit(0, 0);
            }
            *report.borrow_mut() = grid.paste_text(&setup.text);
        }
    });

    // The rows as the grid now shows them, one per line, so the rendered page
    // can be read back as the state of the grid.
    let shown = (0..grid.view().read().len())
        .filter_map(|row| {
            grid.with_row(row, |row: &Row| {
                format!("{}|{}|{}", row.name, row.department, row.age)
            })
        })
        .collect::<Vec<_>>()
        .join("\n");
    let selection = grid
        .cell_range()
        .map(|range| {
            let start = range.top_left();
            let end = range.bottom_right();
            format!("{},{}-{},{}", start.row, start.col, end.row, end.col)
        })
        .unwrap_or_default();
    let status = format!("{:?}", grid.edit_status());

    rsx! {
        pre { "{shown}" }
        output { "{selection}" }
        div { "{status}" }
    }
}

/// What a paste left behind.
struct Outcome {
    shown: Vec<String>,
    saved: Vec<Row>,
    selection: String,
    status: String,
    report: PasteReport,
}

fn paste(setup: Setup) -> Outcome {
    let saved: Cell<Vec<Row>> = Rc::new(RefCell::new(Vec::new()));
    let report: Cell<PasteReport> = Rc::new(RefCell::new(PasteReport::default()));

    let mut dom = VirtualDom::new_with_props(
        Grid,
        GridProps {
            setup,
            saved: saved.clone(),
            report: report.clone(),
        },
    );
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);

    let between = |open: &str, close: &str| {
        html.split(open)
            .nth(1)
            .and_then(|rest| rest.split(close).next())
            .unwrap_or_default()
            .to_owned()
    };

    Outcome {
        shown: between("<pre>", "</pre>")
            .lines()
            .map(str::to_owned)
            .collect(),
        saved: saved.borrow().clone(),
        selection: between("<output>", "</output>"),
        status: between("<div>", "</div>"),
        report: *report.borrow(),
    }
}

#[test]
fn a_block_lands_at_the_focused_cell() {
    let outcome = paste(
        Setup::builder()
            .text("Zora\tengineering\t31\r\nAdi\tsales\t42".to_owned())
            .focus(Some((1, 0)))
            .build(),
    );

    assert_eq!(outcome.shown[0], "Zora|engineering|31");
    assert_eq!(outcome.shown[1], "Adi|sales|42");
    assert_eq!(outcome.shown[2], "Mia|engineering|28", "untouched");
    // Two rows, four cells: the department column is not editable and keeps its
    // value, but still holds the block's shape.
    assert_eq!(
        outcome.report,
        PasteReport {
            rows: 2,
            cells: 4,
            refused: 0
        }
    );
    assert_eq!(outcome.saved.len(), 2, "one save per row");
}

#[test]
fn a_block_lands_at_the_rectangles_top_left_corner() {
    let outcome = paste(
        Setup::builder()
            .text("Mio\r\nMara".to_owned())
            // Picked bottom-up: the corner decides where the block starts, not
            // the anchor the user happened to begin at.
            .range(Some(((3, 0), (2, 0))))
            .build(),
    );

    assert_eq!(outcome.shown[0], "Zoe|engineering|30", "above the corner");
    assert_eq!(outcome.shown[1], "Mio|sales|41");
    assert_eq!(outcome.shown[2], "Mara|engineering|28");
}

#[test]
fn one_cell_fills_the_selected_rectangle() {
    let outcome = paste(
        Setup::builder()
            .text("Nobody".to_owned())
            .range(Some(((1, 0), (3, 0))))
            .build(),
    );

    assert_eq!(outcome.report.rows, 3);
    assert!(
        outcome.shown.iter().all(|row| row.starts_with("Nobody|")),
        "{:?}",
        outcome.shown
    );
}

#[test]
fn a_block_wider_than_the_grid_loses_what_hangs_over() {
    let outcome = paste(
        Setup::builder()
            .text("Zora\tsales\t31\textra".to_owned())
            .focus(Some((1, 0)))
            .build(),
    );

    assert_eq!(outcome.shown[0], "Zora|engineering|31");
    assert_eq!(outcome.report.rows, 1);
}

#[test]
fn a_block_longer_than_the_grid_does_not_add_rows() {
    let outcome = paste(
        Setup::builder()
            .text("a\r\nb\r\nc\r\nd\r\ne".to_owned())
            .focus(Some((1, 0)))
            .build(),
    );

    assert_eq!(outcome.shown.len(), 3);
    assert_eq!(outcome.report.rows, 3);
}

#[test]
fn a_value_the_column_refuses_leaves_its_whole_row_alone() {
    let outcome = paste(
        Setup::builder()
            // The first row's age is not a number; the second row is fine.
            .text("Zora\tengineering\tmaybe\r\nAdi\tsales\t42".to_owned())
            .focus(Some((1, 0)))
            .build(),
    );

    assert_eq!(
        outcome.shown[0], "Zoe|engineering|30",
        "whole or not at all"
    );
    assert_eq!(outcome.shown[1], "Adi|sales|42");
    assert_eq!(outcome.report.rows, 1);
    assert_eq!(outcome.report.refused, 1);
    // And the grid says so, in the locale's words.
    assert!(
        outcome.status.contains("Could not paste one row"),
        "{}",
        outcome.status
    );
}

#[test]
fn a_row_the_application_refuses_is_counted_too() {
    let outcome = paste(
        Setup::builder()
            .text("  ".to_owned())
            .focus(Some((1, 0)))
            .validate_row(true)
            .build(),
    );

    assert_eq!(outcome.shown[0], "Zoe|engineering|30");
    assert_eq!(outcome.report.refused, 1);
}

#[test]
fn the_pasted_block_stays_selected() {
    let outcome = paste(
        Setup::builder()
            .text("Zora\tengineering\t31\r\nAdi\tsales\t42".to_owned())
            .focus(Some((1, 0)))
            .build(),
    );

    // Two rows from the first data row, three columns wide.
    assert_eq!(outcome.selection, "1,0-2,2");
}

#[test]
fn a_batch_collects_the_pasted_rows_instead_of_saving_them() {
    let outcome = paste(
        Setup::builder()
            .text("Zora\r\nAdi".to_owned())
            .focus(Some((1, 0)))
            .mode(EditMode::Batch)
            .build(),
    );

    assert_eq!(outcome.shown[0], "Zora|engineering|30");
    assert_eq!(outcome.shown[1], "Adi|sales|41");
    assert!(outcome.saved.is_empty(), "nothing saved until the batch is");
}

#[test]
fn a_paste_into_a_header_writes_nothing() {
    let outcome = paste(
        Setup::builder()
            .text("Zora".to_owned())
            .focus(Some((0, 0)))
            .build(),
    );

    assert!(outcome.report.is_empty());
    assert_eq!(outcome.shown[0], "Zoe|engineering|30");
}

#[test]
fn an_open_editor_keeps_its_own_paste() {
    let outcome = paste(
        Setup::builder()
            .text("Zora".to_owned())
            .focus(Some((1, 0)))
            .editing(true)
            .build(),
    );

    assert!(outcome.report.is_empty(), "the input has the text, not us");
    assert_eq!(outcome.shown[0], "Zoe|engineering|30");
}

#[test]
fn empty_text_does_nothing() {
    let outcome = paste(Setup::builder().text(String::new()).build());

    assert!(outcome.report.is_empty());
    assert!(outcome.saved.is_empty());
}
