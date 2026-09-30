//! Server-rendered checks on reordering rows: what the handles say, what a
//! move reports, and where the grid refuses to offer one.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{CellFocus, GridRow, GridState, SortState};
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{GridBody, GridHeader, GridRoot};
use dioxus_datagrid::{Column, GridOptions, RowMove, use_grid};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
struct Row {
    id: u32,
    name: String,
}

impl GridRow for Row {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

fn rows() -> Vec<Row> {
    ["a", "b", "c", "d"]
        .iter()
        .enumerate()
        .map(|(index, name)| Row {
            id: u32::try_from(index).unwrap_or_default() + 1,
            name: (*name).to_owned(),
        })
        .collect()
}

fn columns() -> Vec<Column<Row>> {
    vec![
        Column::new("order", "").drag_handle(),
        Column::new("name", "Name").value_text(|row: &Row| row.name.as_str()),
    ]
}

/// What the grid reported, and the rows as they stand after it.
type Moves = Rc<RefCell<Vec<(usize, usize)>>>;

#[derive(Clone, PartialEq, Props)]
struct Setup {
    /// Without it the grid has nothing to report a move to.
    #[props(default = true)]
    movable: bool,
    /// Sorts the grid, which is an order of its own.
    #[props(default)]
    sorted: bool,
    /// A drag to make before rendering, as positions on the page.
    #[props(default)]
    drag: Option<(usize, usize)>,
    /// Whether to let go at the end of it. Without it the grid renders as it
    /// looks while the pointer is still down.
    #[props(default = true)]
    drop: bool,
    /// A row to move with the keyboard, and by how much.
    #[props(default)]
    keyboard: Option<(usize, isize)>,
}

#[component]
fn Grid(setup: Setup, moved: Moves) -> Element {
    let mut data = use_signal(rows);
    let cols = use_hook(columns);
    let mut grid = use_grid(
        data,
        cols,
        GridOptions {
            initial_state: Some(GridState {
                sort: if setup.sorted {
                    vec![SortState::desc("name")]
                } else {
                    Vec::new()
                },
                ..GridState::new()
            }),
            ..GridOptions::default()
        },
    );

    if setup.movable {
        let moved = moved.clone();
        grid.set_row_move(use_hook(move || {
            Callback::new(move |change: RowMove<Row>| {
                moved.borrow_mut().push((change.from, change.to));
                data.with_mut(|rows| change.apply(rows));
            })
        }));
    }

    use_hook(move || {
        let mut grid = grid;
        if let Some((from, to)) = setup.drag {
            grid.start_row_drag(from);
            grid.drag_row_over(to);
            if setup.drop {
                grid.finish_row_drag();
            }
        }
        if let Some((row, step)) = setup.keyboard {
            grid.set_focus(CellFocus::new(row + 1, 1));
            grid.move_row_by(row, step);
        }
    });

    let order: Vec<String> = (0..grid.view().read().len())
        .filter_map(|row| grid.with_row(row, |row: &Row| row.name.clone()))
        .collect();

    rsx! {
        GridRoot { grid,
            GridHeader { grid }
            GridBody { grid }
        }
        pre { "{order.join(\" \")}" }
    }
}

struct Outcome {
    html: String,
    order: String,
    moves: Vec<(usize, usize)>,
}

fn render(setup: Setup) -> Outcome {
    let moved: Moves = Rc::new(RefCell::new(Vec::new()));

    let mut dom = VirtualDom::new_with_props(
        Grid,
        GridProps {
            setup,
            moved: moved.clone(),
        },
    );
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    let order = html
        .split("<pre>")
        .nth(1)
        .and_then(|rest| rest.split("</pre>").next())
        .unwrap_or_default()
        .to_owned();

    Outcome {
        html,
        order,
        moves: moved.borrow().clone(),
    }
}

#[test]
fn every_row_gets_a_handle() {
    let outcome = render(Setup::builder().build());

    assert_eq!(outcome.html.matches("data-row-handle").count(), 4);
    assert!(
        outcome.html.contains(r#"aria-label="Move row""#),
        "{}",
        outcome.html
    );
    // Not a tab stop, and it says which keys do the same thing.
    assert!(
        outcome
            .html
            .contains(r#"aria-keyshortcuts="Alt+Shift+ArrowUp"#)
    );
    assert!(outcome.html.contains(r#"tabindex="-1""#));
}

#[test]
fn a_grid_that_cannot_reorder_draws_no_handles() {
    let outcome = render(Setup::builder().movable(false).build());

    assert!(
        !outcome.html.contains("data-row-handle"),
        "{}",
        outcome.html
    );
    // The column is still a column, with a cell in every row.
    assert_eq!(outcome.html.matches(r#"role="gridcell""#).count(), 8);
}

#[test]
fn a_sorted_grid_offers_no_handles_at_all() {
    let outcome = render(Setup::builder().sorted(true).build());

    // The rows are in an order of the grid's making; moving one inside it
    // would mean nothing to the rows underneath.
    assert!(
        !outcome.html.contains("data-row-handle"),
        "{}",
        outcome.html
    );
    assert_eq!(outcome.order, "d c b a");
}

#[test]
fn a_dragged_row_and_the_row_it_is_over_both_say_so() {
    // Still in flight: taken from the first row, now over the third.
    let outcome = render(Setup::builder().drag(Some((0, 2))).drop(false).build());

    assert!(
        outcome.html.contains(r#"data-dragging="true""#),
        "{}",
        outcome.html
    );
    // Coming from above, so it would land below the row it is over.
    assert!(
        outcome.html.contains(r#"data-drop="after""#),
        "{}",
        outcome.html
    );
    // And nothing has moved while the pointer is still down.
    assert!(outcome.moves.is_empty());
    assert_eq!(outcome.order, "a b c d");
}

#[test]
fn a_row_dragged_upwards_would_land_above_the_row_it_is_over() {
    let outcome = render(Setup::builder().drag(Some((3, 1))).drop(false).build());

    assert!(
        outcome.html.contains(r#"data-drop="before""#),
        "{}",
        outcome.html
    );
}

#[test]
fn a_drag_that_went_nowhere_moves_nothing() {
    let outcome = render(Setup::builder().drag(Some((1, 1))).build());

    assert!(outcome.moves.is_empty());
    assert_eq!(outcome.order, "a b c d");
    assert!(!outcome.html.contains("data-drop"), "{}", outcome.html);
}

#[test]
fn a_drag_reports_where_the_row_went() {
    let outcome = render(Setup::builder().drag(Some((0, 2))).build());

    assert_eq!(outcome.moves, [(0, 2)]);
    assert_eq!(outcome.order, "b c a d");
}

#[test]
fn the_keyboard_moves_a_row_one_place() {
    let down = render(Setup::builder().keyboard(Some((1, 1))).build());
    assert_eq!(down.moves, [(1, 2)]);
    assert_eq!(down.order, "a c b d");

    let up = render(Setup::builder().keyboard(Some((3, -1))).build());
    assert_eq!(up.moves, [(3, 2)]);
    assert_eq!(up.order, "a b d c");
}

#[test]
fn the_keyboard_stops_at_the_ends() {
    let above = render(Setup::builder().keyboard(Some((0, -1))).build());
    assert!(above.moves.is_empty());
    assert_eq!(above.order, "a b c d");

    let below = render(Setup::builder().keyboard(Some((3, 1))).build());
    assert!(below.moves.is_empty());
    assert_eq!(below.order, "a b c d");
}

#[test]
fn a_sorted_grid_moves_nothing_from_the_keyboard_either() {
    let outcome = render(Setup::builder().sorted(true).keyboard(Some((0, 1))).build());

    assert!(outcome.moves.is_empty());
    assert_eq!(outcome.order, "d c b a");
}
