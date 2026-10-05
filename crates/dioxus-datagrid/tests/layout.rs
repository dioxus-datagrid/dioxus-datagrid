//! Checks on the track list the grid lays its rows out on, and on joining a
//! caller's `class` with a component's own.

#![allow(clippy::unwrap_used)]

use datagrid_core::{ColumnWidth, GridRow};
use dioxus::core::AttributeValue;
use dioxus::prelude::*;
use dioxus_datagrid::{Column, GridOptions, merge_class, use_grid};

#[derive(Clone, PartialEq)]
struct Row {
    id: u32,
}

impl GridRow for Row {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }
}

/// The template of a grid with these columns, as `column_template` writes it.
fn template(columns: Vec<(ColumnWidth, Option<f32>)>) -> String {
    #[derive(Clone, PartialEq, Props)]
    struct Setup {
        columns: Vec<(ColumnWidth, Option<f32>)>,
    }

    #[component]
    fn Grid(setup: Setup) -> Element {
        let rows = use_signal(|| vec![Row { id: 1 }]);
        let cols = use_hook(move || {
            setup
                .columns
                .iter()
                .enumerate()
                .map(|(index, &(width, min))| {
                    let id = format!("c{index}");
                    let column = Column::<Row>::new(id.clone(), id).width(width);
                    match min {
                        Some(min) => column.min_width(min),
                        None => column,
                    }
                })
                .collect::<Vec<_>>()
        });
        let grid = use_grid(rows, cols, GridOptions::default());

        rsx! {
            pre { "{grid.column_template(\"minmax(6rem, auto)\")}" }
        }
    }

    let mut dom = VirtualDom::new_with_props(
        Grid,
        GridProps {
            setup: Setup { columns },
        },
    );
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    html.trim_start_matches("<pre>")
        .trim_end_matches("</pre>")
        .to_owned()
}

#[test]
fn a_fixed_column_is_laid_out_at_its_width() {
    assert_eq!(
        template(vec![(ColumnWidth::Px(88.0), None)]),
        "88px",
        "a width in pixels is one the grid need not think about"
    );
}

#[test]
fn an_auto_column_falls_back_to_the_track_it_is_given() {
    assert_eq!(
        template(vec![(ColumnWidth::Auto, None)]),
        "minmax(6rem, auto)"
    );
    assert_eq!(
        template(vec![(ColumnWidth::Auto, Some(120.0))]),
        "minmax(120px, auto)",
        "its own minimum beats the fallback"
    );
}

#[test]
fn a_fraction_column_keeps_a_floor() {
    // A bare `1fr` is `minmax(auto, 1fr)`, and a cell that hides its overflow
    // has an auto minimum of nothing: with the fixed columns already filling
    // the grid, the column would collapse to zero.
    assert_eq!(
        template(vec![(ColumnWidth::Fraction(1.0), None)]),
        "minmax(48px, 1fr)"
    );
    assert_eq!(
        template(vec![(ColumnWidth::Fraction(2.5), Some(200.0))]),
        "minmax(200px, 2.5fr)",
        "a column that says how narrow it may get is taken at its word"
    );
}

#[test]
fn every_visible_column_gets_a_track() {
    assert_eq!(
        template(vec![
            (ColumnWidth::Px(40.0), None),
            (ColumnWidth::Fraction(1.0), None),
            (ColumnWidth::Auto, None),
        ]),
        "40px minmax(48px, 1fr) minmax(6rem, auto)"
    );
}

/// A component in the shape of the registry's `DataGrid`: a class of its own,
/// and the caller's attributes spread onto the same element.
#[component]
fn Styled(#[props(extends = GlobalAttributes)] attributes: Vec<Attribute>) -> Element {
    let mut attributes = attributes;
    let class = merge_class(&mut attributes, "dg-wrapper");

    rsx! {
        div { class, ..attributes }
    }
}

fn styled(attributes: Vec<Attribute>) -> String {
    let mut dom = VirtualDom::new_with_props(Styled, StyledProps { attributes });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn a_component_without_a_caller_class_keeps_its_own() {
    assert_eq!(styled(Vec::new()), r#"<div class="dg-wrapper"></div>"#);
}

#[test]
fn a_caller_class_joins_the_component_class_instead_of_replacing_it() {
    // Both, in the order they were written, and once: two `class` attributes
    // on one element mean one of them loses — the first for a browser reading
    // this HTML, the last for `setAttribute`.
    let html = styled(vec![Attribute::new("class", "mine wide", None, false)]);

    assert_eq!(html, r#"<div class="dg-wrapper mine wide"></div>"#);
}

#[test]
fn other_attributes_are_left_where_they_were() {
    let html = styled(vec![
        Attribute::new("class", "mine", None, false),
        Attribute::new("id", "grid", None, false),
    ]);

    assert_eq!(html, r#"<div class="dg-wrapper mine" id="grid"></div>"#);
}

#[test]
fn a_class_that_is_not_there_contributes_nothing() {
    // What `class: if condition { "mine" }` leaves behind when the condition
    // does not hold.
    let html = styled(vec![
        Attribute::new("class", AttributeValue::None, None, false),
        Attribute::new("class", "  ", None, false),
    ]);

    assert_eq!(html, r#"<div class="dg-wrapper"></div>"#);
}

#[test]
fn merging_without_a_base_class_is_the_callers_class_alone() {
    let mut attributes = vec![Attribute::new("class", "mine", None, false)];

    assert_eq!(merge_class(&mut attributes, ""), "mine");
    assert!(attributes.is_empty());
}
