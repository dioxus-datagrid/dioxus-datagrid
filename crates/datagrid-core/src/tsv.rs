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

/// Reads tab-separated text back into rows of cells, the way a spreadsheet
/// writes it to the clipboard.
///
/// Rows end at a line break — `\r\n` as Excel writes it, a bare `\n` as Google
/// Sheets and most editors do — and a single trailing break ends the last row
/// rather than starting an empty one. A field is quoted only when it starts with
/// a quotation mark; then a tab or a line break inside it belongs to the field
/// and `""` is one quotation mark. Rows may be ragged: what the text says is
/// what comes back.
///
/// Empty text is no rows at all, which is why this is not quite the inverse of
/// [`to_tsv`]: a block of one empty cell writes as empty text.
///
/// ```
/// use datagrid_core::from_tsv;
///
/// assert_eq!(from_tsv("Zoe\tBerlin\r\nAdam\tKiel"), [["Zoe", "Berlin"], ["Adam", "Kiel"]]);
/// assert_eq!(from_tsv("\"a\tb\"\tc"), [["a\tb", "c"]]);
/// assert!(from_tsv("").is_empty());
/// ```
#[must_use]
pub fn from_tsv(text: &str) -> Vec<Vec<String>> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cell = String::new();
    // Whether the field being read is a quoted one, and whether nothing has
    // been read into it yet — a quotation mark only opens a field at its start.
    let mut quoted = false;
    let mut fresh = true;
    let mut characters = text.chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '"' if quoted && characters.peek() == Some(&'"') => {
                characters.next();
                cell.push('"');
            }
            '"' if quoted => quoted = false,
            '"' if fresh => {
                quoted = true;
                fresh = false;
            }
            '\t' if !quoted => {
                row.push(std::mem::take(&mut cell));
                fresh = true;
            }
            '\r' | '\n' if !quoted => {
                if character == '\r' && characters.peek() == Some(&'\n') {
                    characters.next();
                }
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
                fresh = true;
            }
            other => {
                cell.push(other);
                fresh = false;
            }
        }
    }
    row.push(cell);
    rows.push(row);

    // The text ended with a line break: that break closed the last row, it did
    // not open an empty one.
    if rows.len() > 1
        && rows
            .last()
            .is_some_and(|row| row.len() == 1 && row.first().is_some_and(String::is_empty))
    {
        rows.pop();
    }
    rows
}
