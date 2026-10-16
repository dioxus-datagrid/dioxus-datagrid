//! An HTML table, for printing and for pasting into a page.
//!
//! Everything a cell says is escaped: a grid shows what its users typed, and
//! this text is put into a page. Text appears only as element content; the one
//! attribute that varies, a column's alignment, comes from a fixed list.

use datagrid_core::{CellAlign, ExportAggregate, ExportRow, ExportTable};
use std::fmt::Write as _;

/// How [`to_html`] writes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HtmlOptions {
    /// A heading above the table, written as its `<caption>`.
    pub caption: Option<String>,
    /// Whether to write a whole document with a `<style>` for the page and for
    /// print, rather than the `<table>` alone. A fragment is for putting into a
    /// page that has its own styling.
    pub document: bool,
    /// The language of a whole document, such as `en` or `de`.
    pub lang: Option<String>,
    /// Whether a fragment brings the [`PRINT_CSS`] rules along in a `<style>`
    /// of its own, for a page that has none for it. A whole document always
    /// does.
    pub style: bool,
}

/// The rules a document needs to print well: the header repeats on every page,
/// a row stays whole, numbers line up.
pub const PRINT_CSS: &str = "\
@page { margin: 12mm; }\
table.datagrid { width: 100%; border-collapse: collapse; font: 10pt/1.35 system-ui, sans-serif; }\
table.datagrid caption { margin-bottom: 0.5em; font-weight: 600; text-align: start; }\
table.datagrid th, table.datagrid td { padding: 3pt 6pt; border-bottom: 0.5pt solid #999; }\
table.datagrid thead { display: table-header-group; }\
table.datagrid thead th { border-bottom: 1pt solid #000; text-align: start; }\
table.datagrid tr { break-inside: avoid; }\
table.datagrid .end { text-align: end; }\
table.datagrid .center { text-align: center; }\
table.datagrid .group th { background: #eee; text-align: start; }\
table.datagrid .aggregates td, table.datagrid .totals td { font-weight: 600; }\
table.datagrid .totals td { border-top: 1pt solid #000; }";

/// Writes the table as HTML.
///
/// Column groups become the header's upper rows, a group's heading a row of its
/// own, and aggregates rows that say what they are (`Sum €7,300.50`). Values are
/// the text the grid shows: a page is read, not parsed.
///
/// ```
/// use datagrid_core::{ExportCell, ExportColumn, ExportRow, ExportTable};
/// use datagrid_export::{HtmlOptions, to_html};
///
/// let table = ExportTable {
///     columns: vec![ExportColumn::new("note", "Note")],
///     rows: vec![ExportRow::Data(vec![ExportCell {
///         value: None,
///         text: "<b>&</b>".into(),
///     }])],
/// };
/// let html = to_html(&table, &HtmlOptions::default());
/// assert!(html.contains("&lt;b&gt;&amp;&lt;/b&gt;"));
/// assert!(!html.contains("<b>"));
/// ```
#[must_use]
pub fn to_html(table: &ExportTable, options: &HtmlOptions) -> String {
    let mut out = String::new();
    if options.document {
        out.push_str("<!doctype html>\n<html");
        if let Some(lang) = &options.lang {
            let _ = write!(out, " lang=\"{}\"", escape(lang));
        }
        out.push_str(">\n<head><meta charset=\"utf-8\"><style>");
        out.push_str(PRINT_CSS);
        out.push_str("</style></head>\n<body>\n");
    }

    if options.style && !options.document {
        let _ = writeln!(out, "<style>{PRINT_CSS}</style>");
    }
    out.push_str("<table class=\"datagrid\">\n");
    if let Some(caption) = &options.caption {
        let _ = writeln!(out, "<caption>{}</caption>", escape(caption));
    }
    header(&mut out, table);
    out.push_str("<tbody>\n");
    let width = table.columns.len().max(1);
    for row in &table.rows {
        match row {
            ExportRow::Data(cells) => {
                out.push_str("<tr>");
                for (index, cell) in cells.iter().enumerate() {
                    let _ = write!(
                        out,
                        "<td{}>{}</td>",
                        align_attr(table, index),
                        escape(&cell.text)
                    );
                }
                out.push_str("</tr>\n");
            }
            ExportRow::GroupHeader {
                level,
                caption,
                aggregates,
                ..
            } => {
                // A level moves the heading in, as it does on screen.
                let _ = writeln!(
                    out,
                    "<tr class=\"group\"><th colspan=\"{width}\" scope=\"colgroup\" style=\"padding-inline-start:{}em\">{}</th></tr>",
                    1 + level * 2,
                    escape(caption)
                );
                aggregate_rows(&mut out, table, aggregates, "aggregates");
            }
            ExportRow::Aggregates { label, cells, .. } => {
                let class = if label.is_some() {
                    "totals"
                } else {
                    "aggregates"
                };
                aggregate_rows(&mut out, table, cells, class);
            }
            _ => {}
        }
    }
    out.push_str("</tbody>\n</table>\n");

    if options.document {
        out.push_str("</body>\n</html>\n");
    }
    out
}

/// The header: one row per level of column groups, then the labels.
fn header(out: &mut String, table: &ExportTable) {
    let levels = table
        .columns
        .iter()
        .map(|column| column.group_path.len())
        .max()
        .unwrap_or(0);

    out.push_str("<thead>\n");
    for level in 0..levels {
        out.push_str("<tr>");
        let mut column = 0;
        while column < table.columns.len() {
            let here = table.columns.get(column);
            let label = here.and_then(|column| column.group_path.get(level));
            // Neighbours under the same label, on this level and every one above
            // it, are one cell. `get(..=level)` is only there where the column
            // has a group at this level.
            let mut span = 1;
            if label.is_some() {
                let prefix = here.and_then(|column| column.group_path.get(..=level));
                while let Some(next) = table.columns.get(column + span) {
                    if next.group_path.get(..=level) != prefix {
                        break;
                    }
                    span += 1;
                }
            }
            let _ = write!(
                out,
                "<th colspan=\"{span}\" scope=\"colgroup\" class=\"center\">{}</th>",
                label.map_or_else(String::new, |label| escape(label))
            );
            column += span;
        }
        out.push_str("</tr>\n");
    }
    out.push_str("<tr>");
    for (index, column) in table.columns.iter().enumerate() {
        let _ = write!(
            out,
            "<th scope=\"col\"{}>{}</th>",
            align_attr(table, index),
            escape(&column.label)
        );
    }
    out.push_str("</tr>\n</thead>\n");
}

/// Rows of aggregates, one per aggregate a column has, the first row first.
fn aggregate_rows(
    out: &mut String,
    table: &ExportTable,
    columns: &[Vec<ExportAggregate>],
    class: &str,
) {
    let rows = columns.iter().map(Vec::len).max().unwrap_or(0);
    for index in 0..rows {
        let _ = write!(out, "<tr class=\"{class}\">");
        for column in 0..table.columns.len() {
            let text = columns
                .get(column)
                .and_then(|aggregates| aggregates.get(index))
                .filter(|aggregate| aggregate.value.is_some())
                .map(|aggregate| format!("{} {}", aggregate.name, aggregate.text))
                .unwrap_or_default();
            let _ = write!(
                out,
                "<td{}>{}</td>",
                align_attr(table, column),
                escape(&text)
            );
        }
        out.push_str("</tr>\n");
    }
}

/// ` class="end"` for a column aligned at the end, and the like.
fn align_attr(table: &ExportTable, column: usize) -> &'static str {
    match table.columns.get(column).map(|column| column.align) {
        Some(CellAlign::End) => " class=\"end\"",
        Some(CellAlign::Center) => " class=\"center\"",
        _ => "",
    }
}

/// Escapes text for element content and for a quoted attribute.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}
