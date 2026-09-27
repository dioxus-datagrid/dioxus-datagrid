//! Tab-separated text: what a spreadsheet reads back as the block it was given.

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use datagrid_core::{to_tsv, tsv_field};
use proptest::prelude::*;

/// Reads tab-separated text back into rows of cells, the way a spreadsheet
/// does: quoted fields keep their tabs and line breaks, `""` is one quotation
/// mark.
///
/// Deliberately written from the format's rules rather than from the producer,
/// so that a round trip proves something.
fn parse(text: &str) -> Vec<Vec<String>> {
    let mut rows = vec![Vec::new()];
    let mut cell = String::new();
    let mut quoted = false;
    let mut characters = text.chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '"' if quoted && characters.peek() == Some(&'"') => {
                characters.next();
                cell.push('"');
            }
            '"' => quoted = !quoted,
            '\t' if !quoted => {
                let finished = std::mem::take(&mut cell);
                if let Some(row) = rows.last_mut() {
                    row.push(finished);
                }
            }
            '\r' if !quoted && characters.peek() == Some(&'\n') => {
                characters.next();
                let finished = std::mem::take(&mut cell);
                if let Some(row) = rows.last_mut() {
                    row.push(finished);
                }
                rows.push(Vec::new());
            }
            other => cell.push(other),
        }
    }
    if let Some(row) = rows.last_mut() {
        row.push(cell);
    }
    rows
}

#[test]
fn plain_cells_are_joined_by_tabs_and_line_breaks() {
    let text = to_tsv([["Name", "City"], ["Zoe", "Berlin"]]);

    assert_eq!(text, "Name\tCity\r\nZoe\tBerlin");
    assert!(
        !text.ends_with("\r\n"),
        "a trailing break would be an empty row"
    );
}

#[test]
fn a_field_is_left_alone_unless_it_would_be_misread() {
    assert_eq!(tsv_field("Berlin"), "Berlin");
    assert_eq!(tsv_field(""), "");
    assert_eq!(tsv_field("1.234,50 €"), "1.234,50 €");
    // Borrowed, not copied, for the common case.
    assert!(matches!(tsv_field("Berlin"), std::borrow::Cow::Borrowed(_)));
}

#[test]
fn a_tab_inside_a_cell_is_quoted_rather_than_splitting_it() {
    let text = to_tsv([["a\tb", "c"]]);

    assert_eq!(text, "\"a\tb\"\tc");
    assert_eq!(parse(&text), [["a\tb", "c"]]);
}

#[test]
fn a_line_break_inside_a_cell_is_quoted_rather_than_ending_the_row() {
    let text = to_tsv([["line one\nline two", "next"]]);

    assert_eq!(parse(&text), [["line one\nline two", "next"]]);
    assert_eq!(parse(&text).len(), 1, "still one row");
}

#[test]
fn a_quotation_mark_is_doubled() {
    let text = to_tsv([[r#"say "hi""#]]);

    assert_eq!(text, r#""say ""hi""""#);
    assert_eq!(parse(&text), [[r#"say "hi""#]]);
}

#[test]
fn an_empty_block_is_empty_text() {
    let empty: [[&str; 0]; 0] = [];
    assert_eq!(to_tsv(empty), "");
    assert_eq!(to_tsv([[""]]), "");
}

#[test]
fn a_single_cell_is_just_its_text() {
    assert_eq!(to_tsv([["Berlin"]]), "Berlin");
}

proptest! {
    /// Whatever the cells hold, a spreadsheet reads back the block it was given.
    #[test]
    fn a_block_survives_the_round_trip(
        block in prop::collection::vec(
            prop::collection::vec("[a-z\t\n\r\"]{0,6}", 1..4),
            1..4,
        ),
    ) {
        // A lone carriage return is not a row separator here and no producer
        // emits one; excluding it keeps the reader honest about what it parses.
        let block: Vec<Vec<String>> = block
            .into_iter()
            .map(|row| row.into_iter().map(|cell| cell.replace('\r', "\n")).collect())
            .collect();

        let text = to_tsv(&block);
        prop_assert_eq!(parse(&text), block);
    }
}
