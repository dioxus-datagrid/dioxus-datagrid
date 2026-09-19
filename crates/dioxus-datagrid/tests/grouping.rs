//! Grouped rows, aggregates and the group panel, rendered and driven.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use dioxus::core::NoOpMutations;
use dioxus::prelude::*;
use dioxus_datagrid::primitives::{
    GridBody, GridFooter, GridGroupPanel, GridHeader, GridRoot, VirtualGridBody,
};
use dioxus_datagrid::{
    Aggregate, CellFocus, Column, ColumnId, EditMode, Editing, GridHandle, GridOptions, GridRow,
    GroupKeyPress, Save, Value, ViewRow, use_grid,
};
use std::cell::RefCell;

#[derive(Clone, Debug, PartialEq)]
struct Person {
    id: u32,
    name: String,
    team: String,
    salary: i64,
}

impl GridRow for Person {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

fn people() -> Vec<Person> {
    [
        (1, "Ada", "Red", 100),
        (2, "Ben", "Blue", 50),
        (3, "Cy", "Red", 20),
        (4, "Di", "Green", 70),
        (5, "Ed", "Blue", 30),
    ]
    .into_iter()
    .map(|(id, name, team, salary)| Person {
        id,
        name: name.to_owned(),
        team: team.to_owned(),
        salary,
    })
    .collect()
}

fn columns() -> Vec<Column<Person>> {
    vec![
        Column::new("name", "Name")
            .value_text(|person: &Person| person.name.as_str())
            .editable(|person: &mut Person, name: String| person.name = name),
        Column::new("team", "Team").value_text(|person: &Person| person.team.as_str()),
        Column::new("salary", "Salary")
            .value_of(|person: &Person| person.salary)
            .aggregate(Aggregate::Sum)
            .aggregate(Aggregate::Max),
    ]
}

thread_local! {
    static GRID: RefCell<Option<GridHandle<Person>>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, PartialEq, Props)]
struct Setup {
    virtualized: bool,
    footer: bool,
    panel: bool,
}

#[component]
fn Grid(setup: Setup) -> Element {
    let mut rows = use_signal(people);
    let cols = use_hook(columns);
    let mut grid = use_grid(rows, cols, GridOptions::default());
    let editing = use_hook(move || Editing {
        mode: EditMode::Cell,
        on_save: Some(EventHandler::new(move |save: Save<Person>| {
            let row = save.row().clone();
            rows.with_mut(|rows| {
                if let Some(slot) = rows.iter_mut().find(|slot| slot.id == row.id) {
                    *slot = row;
                }
            });
        })),
        ..Editing::default()
    });
    grid.set_editing(editing);
    use_hook(move || GRID.with(|slot| *slot.borrow_mut() = Some(grid)));

    rsx! {
        if setup.panel {
            GridGroupPanel { grid }
        }
        GridRoot { grid,
            GridHeader { grid }
            if setup.virtualized {
                VirtualGridBody { grid, row_height: 30.0 }
            } else {
                GridBody { grid }
            }
            if setup.footer {
                GridFooter { grid }
            }
        }
    }
}

fn dom(setup: Setup) -> VirtualDom {
    let mut dom = VirtualDom::new_with_props(Grid, GridProps { setup });
    dom.rebuild_in_place();
    // The footer registers while rendering; the root counts it on the next pass.
    dom.render_immediate(&mut NoOpMutations);
    dom
}

fn plain() -> VirtualDom {
    dom(Setup {
        virtualized: false,
        footer: true,
        panel: true,
    })
}

fn act<R>(dom: &mut VirtualDom, act: impl FnOnce(&mut GridHandle<Person>) -> R) -> R {
    let result = dom.in_scope(ScopeId::ROOT, || {
        GRID.with(|grid| act(grid.borrow_mut().as_mut().unwrap()))
    });
    dom.render_immediate(&mut NoOpMutations);
    result
}

fn html(dom: &VirtualDom) -> String {
    dioxus_ssr::render(dom)
}

fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

#[test]
fn a_grouped_grid_is_a_treegrid_with_levels_and_sets() {
    let mut dom = plain();
    assert!(html(&dom).contains(r#"role="grid""#));

    act(&mut dom, |grid| grid.group_by_column("team", None));
    let page = html(&dom);
    assert!(page.contains(r#"role="treegrid""#), "{page}");
    // Blue, Green, Red: three headers at level 1, in order, with their sizes.
    assert_eq!(count(&page, r#"data-group-row"#), 3);
    assert_eq!(
        count(&page, r#"aria-level="1""#),
        3 + 1,
        "headers and the footer"
    );
    assert!(page.contains(r#"aria-posinset="1" aria-setsize="3""#));
    assert!(page.contains("Team: Blue"));
    assert!(page.contains("2 rows"));
    assert!(page.contains("1 row<"));
    // Data rows and group footers are one level in.
    assert_eq!(count(&page, r#"aria-level="2""#), 5 + 3);
    assert!(page.contains(r#"aria-expanded="true""#));
    // The header's one cell spans the columns.
    assert!(page.contains(r#"aria-colspan="3""#));
    // Header row, 3 groups × (header + footer), 5 people, the totals row.
    assert!(page.contains(r#"aria-rowcount="13""#), "{page}");
}

#[test]
fn the_arrows_on_a_group_header_collapse_and_expand_it() {
    let mut dom = plain();
    act(&mut dom, |grid| {
        grid.group_by_column("team", None);
        grid.set_focus(CellFocus::new(1, 2));
    });
    assert!(act(&mut dom, |grid| grid.group_key(GroupKeyPress::Collapse)));
    let page = html(&dom);
    assert!(page.contains(r#"aria-expanded="false""#));
    // Collapsed, its aggregates move into its header.
    assert!(page.contains("Sum of Salary: 80"), "{page}");
    assert_eq!(
        act(&mut dom, |grid| grid.row_kind(1)),
        Some(ViewRow::GroupHeader(1))
    );

    // Right expands it again; on an expanded group it does nothing more.
    assert!(act(&mut dom, |grid| grid.group_key(GroupKeyPress::Expand)));
    assert!(!html_of(&dom).contains(r#"aria-expanded="false""#));
    assert!(act(&mut dom, |grid| grid.group_key(GroupKeyPress::Expand)));

    // On a data row the arrows are for moving between cells.
    act(&mut dom, |grid| grid.set_focus(CellFocus::new(2, 0)));
    assert!(!act(&mut dom, |grid| grid.group_key(GroupKeyPress::Collapse)));
}

fn html_of(dom: &VirtualDom) -> String {
    html(dom)
}

#[test]
fn left_on_a_collapsed_inner_group_goes_to_the_group_around_it() {
    let mut dom = plain();
    act(&mut dom, |grid| {
        grid.set_group_by(vec!["team".into(), "name".into()]);
        // Blue, then Blue › Ben.
        grid.set_focus(CellFocus::new(2, 0));
    });
    act(&mut dom, |grid| grid.group_key(GroupKeyPress::Collapse));
    assert_eq!(act(&mut dom, |grid| grid.focus().row), 2);
    act(&mut dom, |grid| grid.group_key(GroupKeyPress::Collapse));
    assert_eq!(act(&mut dom, |grid| grid.focus().row), 1);
    let group = act(&mut dom, |grid| grid.group_at(0)).unwrap();
    assert_eq!(group.value, Some(Value::Text("Blue".into())));
    // Enter or Space toggles.
    act(&mut dom, |grid| grid.group_key(GroupKeyPress::Toggle));
    assert!(!act(&mut dom, |grid| grid.group_at(0)).unwrap().expanded);
}

#[test]
fn the_footer_totals_every_row_and_the_keyboard_reaches_it() {
    let mut dom = plain();
    let page = html(&dom);
    assert!(page.contains("data-footer"));
    assert!(page.contains("Totals"));
    assert!(
        page.contains(r#"<span data-aggregate-value="">270</span>"#),
        "{page}"
    );
    assert!(page.contains(r#"<span data-aggregate-value="">100</span>"#));
    // Header, five rows, footer.
    assert!(page.contains(r#"aria-rowcount="7""#));
    assert!(page.contains(r#"aria-rowindex="7""#));
    act(&mut dom, |grid| {
        grid.move_focus(dioxus_datagrid::NavKey::CtrlEnd);
    });
    assert_eq!(act(&mut dom, |grid| grid.focus()), CellFocus::new(6, 2));

    // Without aggregates, no footer and no extra row to reach.
    let mut bare = dom_without_aggregates();
    assert!(!html(&bare).contains("data-footer"));
    assert_eq!(act_bare(&mut bare, |grid| grid.focusable_row_count()), 6);
}

#[component]
fn Bare() -> Element {
    let rows = use_signal(people);
    let cols = use_hook(|| {
        vec![Column::new("name", "Name").value_text(|person: &Person| person.name.as_str())]
    });
    let grid = use_grid(rows, cols, GridOptions::default());
    use_hook(move || BARE.with(|slot| *slot.borrow_mut() = Some(grid)));
    rsx! {
        GridRoot { grid,
            GridBody { grid }
            GridFooter { grid }
        }
    }
}

thread_local! {
    static BARE: RefCell<Option<GridHandle<Person>>> = const { RefCell::new(None) };
}

fn dom_without_aggregates() -> VirtualDom {
    let mut dom = VirtualDom::new(Bare);
    dom.rebuild_in_place();
    dom.render_immediate(&mut NoOpMutations);
    dom
}

fn act_bare<R>(dom: &mut VirtualDom, act: impl FnOnce(&mut GridHandle<Person>) -> R) -> R {
    dom.in_scope(ScopeId::ROOT, || {
        BARE.with(|grid| act(grid.borrow_mut().as_mut().unwrap()))
    })
}

#[test]
fn the_group_panel_groups_by_a_dropped_header_and_offers_a_list() {
    let mut dom = plain();
    let page = html(&dom);
    assert!(page.contains("Drag a column header here to group by it"));
    // Every header can be dragged while the panel is there.
    assert_eq!(count(&page, r#"draggable="true""#), 3);
    assert!(page.contains(r#"<option value="team">Team</option>"#));

    act(&mut dom, |grid| {
        grid.start_column_drag("team".into());
        assert!(grid.drop_dragged_column(None));
    });
    let page = html_of(&dom);
    assert!(page.contains("Stop grouping by Team"));
    assert!(!page.contains(r#"<option value="team">"#));
    // A grouped column is not dragged again.
    assert_eq!(count(&page, r#"draggable="true""#), 2);
    assert!(page.contains("Expand all"));

    act(&mut dom, |grid| grid.group_by_column("salary", None));
    let page = html_of(&dom);
    assert!(page.contains("Group by Salary first"));
    act(&mut dom, |grid| grid.group_by_column("salary", Some(0)));
    assert_eq!(
        act(&mut dom, |grid| grid.group_by()),
        vec![ColumnId::from("salary"), ColumnId::from("team")]
    );

    // Without a drag, a drop does nothing.
    assert!(!act(&mut dom, |grid| grid.drop_dragged_column(None)));
}

#[test]
fn editing_finds_the_data_row_among_group_rows() {
    let mut dom = plain();
    act(&mut dom, |grid| grid.group_by_column("team", None));
    // Row 0 is Blue's header, row 1 Ben.
    assert!(!act(&mut dom, |grid| grid.start_edit(0, 0)));
    assert!(act(&mut dom, |grid| grid.start_edit(1, 0)));
    act(&mut dom, |grid| {
        grid.set_editor_text(&"name".into(), "Benedikt");
        grid.commit_edit(dioxus_datagrid::EditMove::Stay)
    });
    let page = html(&dom);
    assert!(page.contains("Benedikt"));
    // The group still counts the row it had.
    assert!(page.contains("Team: Blue"));
}

#[test]
fn a_virtualized_grouped_body_renders_group_rows_in_its_window() {
    let mut dom = dom(Setup {
        virtualized: true,
        footer: false,
        panel: false,
    });
    act(&mut dom, |grid| {
        grid.record_viewport_height(120.0);
        grid.group_by_column("team", None);
    });
    let page = html(&dom);
    assert!(page.contains("data-group-row"));
    assert!(page.contains(r#"role="treegrid""#));
    // 3 headers, 5 people, 3 footers: 11 rows of 30px.
    assert_eq!(act(&mut dom, |grid| grid.view().read().len()), 11);
}
