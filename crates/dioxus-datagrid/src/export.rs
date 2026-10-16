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
use datagrid_core::{ColumnSpec, ExportScope, ExportTable, GridRow, GridState, export_table};
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

impl<T: GridRow + PartialEq> GridHandle<T> {
    /// The table an export of this grid writes, or `None` for what this grid
    /// cannot say.
    ///
    /// [`ExportScope::View`] is what is on screen: the current page, and groups
    /// as open or closed as they are shown. [`ExportScope::Filtered`] is every
    /// row that passes the filters, on every page — which a grid that loads its
    /// rows from a server does not have, so there it is `None` rather than the
    /// one page that happens to be loaded. For such a grid, the page on screen
    /// is exported as the server sent it, without grouping.
    ///
    /// Column headings are the columns' own labels. Pass the table to a writer
    /// from `datagrid-export`.
    #[must_use]
    pub fn export_table(&self, scope: ExportScope) -> Option<ExportTable> {
        if self.remote && scope == ExportScope::Filtered {
            return None;
        }

        let columns = self.columns.read();
        let specs: Vec<ColumnSpec<T>> =
            columns.iter().map(|column| column.spec().clone()).collect();
        let label = |id: &datagrid_core::ColumnId| {
            columns.iter().find(|column| column.id() == id).map_or_else(
                || id.as_str().to_owned(),
                |column| column.label().to_owned(),
            )
        };

        let mut state = self.state.read().clone();
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

        let locale = self.locale.read();
        let rows = self.data.read();
        Some(export_table(
            &rows,
            &specs,
            label,
            &state,
            &locale,
            if self.remote {
                ExportScope::Filtered
            } else {
                scope
            },
        ))
    }
}
