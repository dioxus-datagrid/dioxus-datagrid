//! Copying the selection to the clipboard as tab-separated text.
//!
//! Dioxus has no clipboard API and its `ClipboardData` carries no text, so the
//! way out is `document::eval` — see `docs/DECISIONS.md` ADR-0033, which the
//! spike in `docs/VERIFICATION.md` §15 argues for. The text travels over the
//! eval channel rather than being formatted into the script: a copied cell can
//! hold quotation marks, and a script built by `format!` would be an injection
//! point into someone else's data.

use crate::GridHandle;
use datagrid_core::{GridRow, to_tsv};
use dioxus::prelude::*;

/// Puts the text on the clipboard, or says it could not.
///
/// Two routes, because no single one is allowed everywhere: `writeText` is the
/// modern API and needs a permission that a headless browser refuses, while
/// `execCommand` is deprecated and universally implemented. The measurements
/// are in `docs/VERIFICATION.md` §15.
const WRITE_CLIPBOARD: &str = r#"
    const text = await dioxus.recv();
    if (navigator.clipboard && navigator.clipboard.writeText) {
        try {
            await navigator.clipboard.writeText(text);
            return true;
        } catch (error) {
            // Refused: fall through to the older route rather than give up.
        }
    }
    const area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.top = "-1000px";
    document.body.appendChild(area);
    area.select();
    try {
        return document.execCommand("copy");
    } catch (error) {
        return false;
    } finally {
        area.remove();
    }
"#;

impl<T: GridRow + PartialEq> GridHandle<T> {
    /// The text one cell copies: its column's value, formatted as the grid
    /// shows it.
    ///
    /// A column with no value copies nothing, even if it renders something with
    /// [`Column::cell`](crate::Column::cell) — a rendered `Element` is markup,
    /// not text, and guessing at it would be worse than an empty cell.
    #[must_use]
    pub fn cell_text(&self, row_index: usize, column_index: usize) -> String {
        let columns = self.visible_columns();
        let Some(column) = columns.get(column_index) else {
            return String::new();
        };
        let Some(value) = column.spec().value.clone() else {
            return String::new();
        };
        let locale = self.locale();
        let locale = locale.read();
        let format = column.spec().format.clone();
        self.with_row(row_index, |row| locale.format(&value(row), &format))
            .unwrap_or_default()
    }

    /// The tab-separated text the current selection would copy, or `None` when
    /// there is nothing to copy.
    ///
    /// What is copied, in this order: the selected rectangle of cells; failing
    /// that the selected rows, with every visible column; failing that the
    /// focused cell. Column headers are not included — a paste into a sheet that
    /// already has headings would gain a stray row.
    #[must_use]
    pub fn copy_text(&self) -> Option<String> {
        let header_rows = self.header_rows();
        let columns = self.visible_column_count();

        if let Some(range) = self.cell_range() {
            let rows = range.rows().filter_map(|row| row.checked_sub(header_rows));
            let block: Vec<Vec<String>> = rows
                .map(|row| {
                    range
                        .columns()
                        .map(|column| self.cell_text(row, column))
                        .collect()
                })
                .collect();
            return (!block.is_empty()).then(|| to_tsv(block));
        }

        if self.selected_count() > 0 {
            // In the order the rows are shown, not the order they were picked.
            let block: Vec<Vec<String>> = (0..self.view().read().len())
                .filter(|row| self.key_at(*row).is_some_and(|key| self.is_selected(&key)))
                .map(|row| {
                    (0..columns)
                        .map(|column| self.cell_text(row, column))
                        .collect()
                })
                .collect();
            return (!block.is_empty()).then(|| to_tsv(block));
        }

        let focus = self.focus();
        let row = focus.row.checked_sub(header_rows)?;
        Some(self.cell_text(row, focus.col))
    }

    /// Copies the selection to the clipboard as tab-separated text.
    ///
    /// Does nothing when there is nothing to copy. The write itself happens in
    /// the background and cannot be waited for; whether the browser allowed it
    /// is not something the grid can act on, and asking the user for a clipboard
    /// permission to copy a row would be worse than the occasional silent
    /// refusal.
    pub fn copy_selection(&self) {
        let Some(text) = self.copy_text() else {
            return;
        };
        spawn(async move {
            let eval = document::eval(WRITE_CLIPBOARD);
            if eval.send(text).is_ok() {
                let _ = eval.join::<bool>().await;
            }
        });
    }

    /// Whether a copy would put anything on the clipboard, for a menu entry or
    /// a toolbar button that should be disabled otherwise.
    #[must_use]
    pub fn can_copy(&self) -> bool {
        self.cell_range().is_some()
            || self.selected_count() > 0
            || self.focus().row >= self.header_rows()
    }
}
