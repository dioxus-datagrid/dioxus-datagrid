//! Pasting: tab-separated text from the clipboard, written into editable cells.
//!
//! Nothing here fetches anything. Reading the clipboard was refused in every
//! environment measured (`docs/VERIFICATION.md` §15), so the text can only
//! arrive the way the browser hands it over: in a `paste` event. An eval listens
//! for one and passes the text to [`GridHandle::paste_text`], which writes it —
//! see `docs/DECISIONS.md` ADR-0033.

use crate::{EditStatus, GridHandle};
use datagrid_core::{CellFocus, GridRow, ValueKind, from_tsv};
use dioxus::prelude::*;

/// Watches for a paste and hands the text over, until the grid says stop.
///
/// The listener sits on the document because a paste with focus on a grid cell
/// is not aimed at anything the grid could listen on alone. Whatever does take
/// text keeps its own paste, which is why the target is checked here rather than
/// in Rust — an editor's input has to paste the way an input does.
const PASTE_LISTENER: &str = r#"
    const listener = (event) => {
        const target = event.target;
        if (target && target.closest
            && target.closest("input, textarea, select, [contenteditable]")) {
            return;
        }
        const text = event.clipboardData ? event.clipboardData.getData("text/plain") : "";
        if (!text) {
            return;
        }
        try {
            dioxus.send(text);
        } catch (error) {
            // The grid this belonged to is gone; so is the reason to listen.
            document.removeEventListener("paste", listener);
        }
    };
    document.addEventListener("paste", listener);
    try {
        // Anything the grid sends means: stop.
        await dioxus.recv();
    } finally {
        document.removeEventListener("paste", listener);
    }
"#;

/// What a paste did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PasteReport {
    /// How many rows were written.
    pub rows: usize,
    /// How many cells were written, across those rows.
    pub cells: usize,
    /// How many rows were left as they were because a value in them was
    /// refused. A row is pasted whole or not at all, so that a validator still
    /// sees a row that makes sense.
    pub refused: usize,
}

impl PasteReport {
    /// Whether the paste changed nothing and refused nothing — there was
    /// nowhere to put it.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rows == 0 && self.refused == 0
    }
}

/// Installs a grid's paste listener for as long as the grid is on the page.
///
/// One per grid rather than one per cell: the channel lives as long as its
/// script waits, so it belongs in a hook of the root, and each grid decides for
/// itself whether a paste was meant for it.
pub(crate) fn use_paste_listener<T: GridRow + PartialEq + 'static>(grid: GridHandle<T>) {
    let eval = use_hook(|| document::eval(PASTE_LISTENER));

    use_hook(move || {
        let mut eval = eval;
        let mut grid = grid;
        spawn(async move {
            // Ends when the channel closes, which is what a document that cannot
            // run scripts — a server rendering a page — does at once.
            while let Ok(text) = eval.recv::<String>().await {
                grid.paste_text(&text);
            }
        });
    });

    use_drop(move || {
        let _ = eval.send("stop");
    });
}

impl<T: GridRow + PartialEq> GridHandle<T> {
    /// Writes tab-separated `text` into the grid and reports what that did.
    ///
    /// Where it lands: at the top-left corner of the selected rectangle, or at
    /// the focused cell when no rectangle is selected. From there the block
    /// spreads right and down; a single pasted cell fills the whole selected
    /// rectangle instead, as a spreadsheet does. Cells past the last column or
    /// the last row are dropped — a paste never adds rows.
    ///
    /// What it skips: a column that cannot be edited keeps its value but still
    /// takes its place in the block, so the rest of a row stays under the
    /// columns it was copied from. Rows marked deleted are left alone, and a row
    /// whose values are refused stays as it was, counted in
    /// [`PasteReport::refused`].
    ///
    /// Each written row is saved the way a committed edit is: recorded in the
    /// batch in [`EditMode::Batch`](crate::EditMode), handed to
    /// [`on_save`](crate::Editing::on_save) otherwise — once per row.
    ///
    /// The grid cannot fetch the clipboard itself (`docs/VERIFICATION.md` §15),
    /// so this is where the text a `paste` event carried arrives; an application
    /// with a source of its own may call it too.
    pub fn paste_text(&mut self, text: &str) -> PasteReport {
        let mut report = PasteReport::default();
        // Nothing to paste into, or an editor that already has the text: an open
        // input pastes as an input, and the grid stays out of it.
        let Some(editing) = self.editing() else {
            return report;
        };
        // Peeked at the session rather than asked through `is_editing`, whose
        // memo may not have caught up inside the render that started the edit.
        if self.edit_session.peek().is_some() {
            return report;
        }
        let block = from_tsv(text);
        if block.is_empty() {
            return report;
        }

        let header_rows = self.header_rows();
        let range = self.cell_range();
        let start = range.map_or_else(|| self.focus(), |range| range.top_left());
        // A header cell is not a place to paste into.
        let Some(first_row) = start.row.checked_sub(header_rows) else {
            return report;
        };

        // One cell pasted into a rectangle fills it; otherwise the block keeps
        // its own size.
        let one_cell = block.len() == 1 && block.first().is_some_and(|row| row.len() == 1);
        let fill = match &range {
            Some(range) if one_cell && !range.is_single() => Some(range),
            _ => None,
        };
        let height = fill.map_or(block.len(), |range| range.rows().count());
        let width = fill.map_or_else(
            || block.iter().map(Vec::len).max().unwrap_or_default(),
            |range| range.columns().count(),
        );

        // Group headers, footers and totals are not rows to paste into: the
        // block lands on the data rows from the target downwards.
        let rows: Vec<usize> = self
            .view()
            .read()
            .data_rows()
            .map(|(position, _)| position)
            .filter(|position| *position >= first_row)
            .take(height)
            .collect();
        let columns = self.visible_columns();
        let locale = self.locale.peek().clone();

        for (line, row_index) in rows.iter().copied().enumerate() {
            let Some(original) = self.row_at(row_index) else {
                continue;
            };
            if self.is_row_deleted(row_index) {
                continue;
            }

            let mut draft = original.clone();
            let mut written = 0;
            let mut refused = false;
            for offset in 0..width {
                let Some(column) = columns.get(start.col + offset) else {
                    // Past the last column: the rest of the block is dropped.
                    break;
                };
                let cell = if fill.is_some() {
                    block.first().and_then(|row| row.first())
                } else {
                    block.get(line).and_then(|row| row.get(offset))
                };
                let Some(cell) = cell else {
                    continue;
                };
                if !column.spec().is_editable() {
                    continue;
                }
                let kind = self.value_kind(column.id()).unwrap_or(ValueKind::Text);
                if column.spec().apply_edit(&mut draft, cell, kind).is_err() {
                    refused = true;
                    break;
                }
                written += 1;
            }

            let checked = !refused
                && editing
                    .validate_row
                    .is_none_or(|validate| validate.call(draft.clone()).is_ok());
            if !checked {
                report.refused += 1;
                continue;
            }
            if written == 0 || draft == original {
                continue;
            }
            self.save_row(&editing, original, draft);
            report.rows += 1;
            report.cells += written;
        }

        if let (Some(&first), Some(&last)) = (rows.first(), rows.last())
            && !report.is_empty()
        {
            // The pasted block stays selected, as it does in a spreadsheet — and
            // when something was refused, that is where to look.
            let last_column =
                (start.col + width.saturating_sub(1)).min(columns.len().saturating_sub(1));
            self.select_cell(CellFocus::new(first + header_rows, start.col));
            self.extend_cell_selection(CellFocus::new(last + header_rows, last_column));
        }
        if report.refused > 0 {
            self.edit_status
                .set(EditStatus::Failed(locale.paste_refused(report.refused)));
        }
        report
    }
}
