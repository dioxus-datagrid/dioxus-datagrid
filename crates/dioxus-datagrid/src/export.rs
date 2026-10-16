//! Exporting what the grid shows, and handing a file to the user.
//!
//! The table is built by [`datagrid_core::export_table`]; this module feeds it
//! from the grid's own rows, columns and state, and gives the application a way
//! to save the result. Writing a format is `datagrid-export`'s job, so that this
//! crate carries no format libraries.
//!
//! Saving a file has no platform-neutral API in Dioxus 0.7, as the clipboard has
//! none: the way out is `document::eval` (`docs/DECISIONS.md` ADR-0041). The
//! content travels over the eval channel rather than being formatted into the
//! script, so that a cell holding a quotation mark cannot become code.

use crate::GridHandle;
use datagrid_core::{
    ColumnSpec, ExportScope, ExportTable, GridQuery, GridRow, GridState, export_table,
};
use dioxus::prelude::*;

/// Hands a file to the browser, or the webview, as a download.
///
/// A `Blob` and a link with the `download` attribute, clicked. The name is only a
/// suggestion; the browser decides where the file goes and may ask.
const SAVE_FILE: &str = r#"
    const [name, type, text] = await dioxus.recv();
    const blob = new Blob([text], { type });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = name;
    link.style.display = "none";
    document.body.appendChild(link);
    link.click();
    link.remove();
    // Revoked late: some browsers read the blob after the click has returned.
    setTimeout(() => URL.revokeObjectURL(url), 60000);
    return true;
"#;

/// Offers `content` to the user as a file called `file_name`.
///
/// `mime` is the media type, such as `"text/csv;charset=utf-8"`. Does nothing
/// the application can observe: whether the browser saved, asked or refused is
/// the user's business with it.
///
/// ```rust,ignore
/// let csv = datagrid_export::to_csv(&table, &CsvOptions::default());
/// save_file("people.csv", "text/csv;charset=utf-8", csv);
/// ```
pub fn save_file(file_name: &str, mime: &str, content: String) {
    let name = file_name.to_owned();
    let mime = mime.to_owned();
    spawn(async move {
        let eval = document::eval(SAVE_FILE);
        if eval.send((name, mime, content)).is_ok() {
            let _ = eval.join::<bool>().await;
        }
    });
}

/// How many rows [`GridHandle::export_all`] asks a server for at a time.
pub const EXPORT_PAGE_SIZE: usize = 1_000;

/// How many rows an export of a server's rows takes unless told otherwise: what
/// a browser tab holds comfortably, and what a spreadsheet opens.
pub const DEFAULT_EXPORT_ROWS: usize = 100_000;

/// Why [`GridHandle::export_all`] has no table to give.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportError {
    /// The server did not answer a request for a page; what it said.
    Source(String),
    /// More rows match than the export was allowed to take.
    TooMany {
        /// How many rows match.
        total: usize,
        /// How many were allowed.
        max: usize,
    },
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(message) => f.write_str(message),
            Self::TooMany { total, max } => {
                write!(f, "{total} rows match, and an export takes at most {max}")
            }
        }
    }
}

impl std::error::Error for ExportError {}

impl<T: GridRow + PartialEq + 'static> GridHandle<T> {
    /// The table an export of this grid writes, or `None` for what this grid
    /// cannot say at once.
    ///
    /// [`ExportScope::View`] is what is on screen: the current page, and groups
    /// as open or closed as they are shown. [`ExportScope::Filtered`] is every
    /// row that passes the filters, on every page — which a grid that loads its
    /// rows from a server does not have, so there it is `None` rather than the
    /// one page that happens to be loaded: [`export_all`](GridHandle::export_all)
    /// fetches them. For such a grid, `View` is the page on screen as the server
    /// sent it, without grouping.
    ///
    /// Column headings are the columns' own labels. Pass the table to a writer
    /// from `datagrid-export`.
    #[must_use]
    pub fn export_table(&self, scope: ExportScope) -> Option<ExportTable> {
        if self.remote && scope == ExportScope::Filtered {
            return None;
        }
        let rows = self.data.read();
        Some(self.table_of(
            &rows,
            if self.remote {
                // The rows are the server's answer, in the order it chose.
                ExportScope::Filtered
            } else {
                scope
            },
        ))
    }

    /// The table of every row that passes the filters, fetching them from the
    /// server a page at a time when the grid loads its rows from one.
    ///
    /// A grid with every row in memory answers at once, as
    /// [`export_table`](GridHandle::export_table) does for
    /// [`ExportScope::Filtered`]. A remote grid asks its source for page after
    /// page, with the sort, filters and search it shows now, until it has
    /// them all: [`EXPORT_PAGE_SIZE`] rows a request.
    ///
    /// - **A limit.** More than `max_rows` matching rows is
    ///   [`ExportError::TooMany`], found out from the first page, before the rest
    ///   are fetched. [`DEFAULT_EXPORT_ROWS`] is a reasonable number.
    /// - **A failure** of any request is [`ExportError::Source`], and no table:
    ///   half a file that looks whole is worse than none.
    /// - **Grouping is left out.** A server pages across group headings, and
    ///   stitching them back together is a different job; the rows come flat, in
    ///   the order the sort gives. Totals are worked out from the rows.
    /// - **A snapshot.** The query is taken when the future first runs, which is
    ///   when it is awaited or spawned; sorting or filtering the grid after that
    ///   changes nothing about this export.
    ///
    /// The future does not borrow the handle, so a click handler can spawn it.
    pub fn export_all(
        &self,
        max_rows: usize,
    ) -> impl std::future::Future<Output = Result<ExportTable, ExportError>> + 'static {
        let grid = *self;
        async move {
            let Some(fetch) = grid.fetch.peek().clone() else {
                // Every row is here.
                let rows = grid.data.peek();
                return Ok(grid.table_of(&rows, ExportScope::Filtered));
            };

            let specs: Vec<ColumnSpec<T>> = grid
                .columns
                .peek()
                .iter()
                .map(|column| column.spec().clone())
                .collect();
            let state = grid.state.peek().clone();
            let mut query = GridQuery {
                page: 0,
                page_size: EXPORT_PAGE_SIZE,
                // Flat: see the method's documentation.
                group_by: Vec::new(),
                groups_collapsed: false,
                toggled_groups: Vec::new(),
                ..GridQuery::from_state(&state, EXPORT_PAGE_SIZE).with_aggregates(&specs)
            };

            let mut rows: Vec<T> = Vec::new();
            // A server that keeps answering with rows its total says are not
            // there would otherwise be asked for ever.
            let most_pages = max_rows / EXPORT_PAGE_SIZE + 2;
            for _ in 0..most_pages {
                let page = fetch(query.clone()).await.map_err(ExportError::Source)?;
                if page.total > max_rows {
                    return Err(ExportError::TooMany {
                        total: page.total,
                        max: max_rows,
                    });
                }
                let empty = page.rows.is_empty();
                rows.extend(page.rows);
                if empty || rows.len() >= page.total {
                    break;
                }
                query.page += 1;
            }

            Ok(grid.table_of(&rows, ExportScope::Filtered))
        }
    }

    /// The table of `rows`, which are the grid's own or all that a server has
    /// sent.
    fn table_of(&self, rows: &[T], scope: ExportScope) -> ExportTable {
        let columns = self.columns.peek();
        let specs: Vec<ColumnSpec<T>> =
            columns.iter().map(|column| column.spec().clone()).collect();
        let label = |id: &datagrid_core::ColumnId| {
            columns.iter().find(|column| column.id() == id).map_or_else(
                || id.as_str().to_owned(),
                |column| column.label().to_owned(),
            )
        };

        let mut state = self.state.peek().clone();
        if self.remote {
            // The rows are already the server's answer: sorted, filtered and
            // paged. Doing any of it again would take rows out of a page that
            // has them in the right order.
            state = GridState {
                column_widths: state.column_widths,
                hidden_columns: state.hidden_columns,
                column_order: state.column_order,
                pinned_columns: state.pinned_columns,
                ..GridState::new()
            };
        }

        export_table(rows, &specs, label, &state, &self.locale.peek(), scope)
    }
}

/// Puts `html` on a page of its own for the printer: the page's other content
/// is hidden while printing, and `html` is shown in its place.
///
/// The grid itself is not what gets printed. A paged or virtualized grid has
/// only some of its rows in the document, so what is printed is a table made
/// from [`export_table`](GridHandle::export_table), which has them all, and
/// whose header repeats on every sheet. `html` is a fragment such as
/// `datagrid_export::to_html` writes, with its `style` option on.
///
/// The fragment is inserted as markup, so it must be markup the caller
/// trusts: `to_html` escapes every cell, and that is the one to use. It
/// travels over the eval channel rather than inside the script.
///
/// The container is removed once printing ends. Where a browser never reports
/// it, the next print replaces it, and it is hidden on screen meanwhile.
pub fn print_html(html: String) {
    spawn(async move {
        let eval = document::eval(PRINT_HTML);
        if eval.send(html).is_ok() {
            let _ = eval.join::<bool>().await;
        }
    });
}

const PRINT_HTML: &str = r#"
    const html = await dioxus.recv();
    const old = document.getElementById("datagrid-print");
    if (old) old.remove();
    const container = document.createElement("div");
    container.id = "datagrid-print";
    container.innerHTML = html;
    const rules = document.createElement("style");
    rules.textContent =
        "@media screen { #datagrid-print { display: none; } }" +
        "@media print { body > *:not(#datagrid-print) { display: none !important; } }";
    container.prepend(rules);
    document.body.appendChild(container);
    window.addEventListener("afterprint", () => container.remove(), { once: true });
    window.print();
    return true;
"#;
