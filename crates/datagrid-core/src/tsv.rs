//! Tab-separated text, the shape a spreadsheet reads from the clipboard.
//!
//! Excel, Numbers and LibreOffice all take a block of cells from the clipboard
//! as tab-separated rows, and all of them read a field the same way as in a CSV:
//! a field that holds a tab, a line break or a quotation mark is wrapped in
//! quotation marks, and a quotation mark inside it is doubled. Without that a
//! cell holding `a\tb` would silently become two cells.

use std::borrow::Cow;

/// What separates two rows: a carriage return and a line feed.
///
/// What a spreadsheet itself puts on the clipboard, and what the Windows
/// programs a user pastes into expect. Everything that reads the shorter form
/// reads this one too.
const ROW_SEPARATOR: &str = "\r\n";

/// Quotes a field if it would otherwise be misread.
///
/// A field only needs quoting when it holds a tab, a line break or a quotation
/// mark; the common case is returned untouched, without an allocation.
///
/// ```
/// use datagrid_core::tsv_field;
///
/// assert_eq!(tsv_field("Berlin"), "Berlin");
/// assert_eq!(tsv_field("a\tb"), "\"a\tb\"");
/// assert_eq!(tsv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
/// ```
#[must_use]
pub fn tsv_field(text: &str) -> Cow<'_, str> {
    if !text.contains(['\t', '\n', '\r', '"']) {
        return Cow::Borrowed(text);
    }
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for character in text.chars() {
        if character == '"' {
            quoted.push('"');
        }
        quoted.push(character);
    }
    quoted.push('"');
    Cow::Owned(quoted)
}

/// Joins rows of cells into tab-separated text.
///
/// Every row is one line, every cell one field, each quoted only if it has to
/// be. There is no trailing line break: a spreadsheet would read it as an empty
/// row.
///
/// ```
/// use datagrid_core::to_tsv;
///
/// let text = to_tsv([["Name", "City"], ["Zoe", "Berlin"]]);
/// assert_eq!(text, "Name\tCity\r\nZoe\tBerlin");
/// ```
#[must_use]
pub fn to_tsv<Rows, Row, Cell>(rows: Rows) -> String
where
    Rows: IntoIterator<Item = Row>,
    Row: IntoIterator<Item = Cell>,
    Cell: AsRef<str>,
{
    let mut text = String::new();
    // Counted rather than asking whether anything has been written: a first row
    // of empty cells writes nothing, and the row after it would lose its break.
    for (line, row) in rows.into_iter().enumerate() {
        if line > 0 {
            text.push_str(ROW_SEPARATOR);
        }
        for (index, cell) in row.into_iter().enumerate() {
            if index > 0 {
                text.push('\t');
            }
            text.push_str(&tsv_field(cell.as_ref()));
        }
    }
    text
}
