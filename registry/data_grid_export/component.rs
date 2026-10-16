//! Export for `data_grid`: the rows as a CSV file, composed from what the
//! `dioxus-datagrid` and `datagrid-export` crates already do.
//!
//! This file is yours once `dx components add` copies it in. What goes into the
//! file — the columns in the order the grid shows them, the rows as sorted,
//! filtered and grouped — is worked out by `dioxus-datagrid`; how CSV is written
//! is `datagrid-export`'s. This component only puts a button in front of them.

use datagrid_export::{CsvOptions, HtmlOptions, to_csv, to_html};
use dioxus::prelude::*;
use dioxus_datagrid::{
    DEFAULT_EXPORT_ROWS, ExportError, ExportScope, ExportTable, GridHandle, GridRow, merge_class,
    print_html, save_file,
};

const STYLE: Asset = asset!("/src/components/data_grid_export/style.css");

/// Props for [`DataGridExport`].
#[derive(Props, Clone, PartialEq)]
pub struct DataGridExportProps {
    /// The name of the downloaded file, without its extension.
    #[props(default = "export".to_owned())]
    pub file_name: String,
    /// Which rows to export. [`ExportScope::Filtered`], every row that passes
    /// the filters, is the default; [`ExportScope::View`] is the page on screen.
    /// A grid that loads its rows from a server fetches them all, a page at a
    /// time, without its groups.
    #[props(default = ExportScope::Filtered)]
    pub scope: ExportScope,
    /// How the CSV is written: its delimiter, whether it starts with a byte
    /// order mark, whether it holds values or the text shown.
    #[props(default)]
    pub csv: CsvOptions,
    /// Whether to show a button that prints the same rows as a table, with its
    /// header repeated on every sheet.
    #[props(default = true)]
    pub print: bool,
    /// The most rows to take from a server. More than this and the export says
    /// so instead of loading them; a grid with every row in memory is not asked.
    #[props(default = DEFAULT_EXPORT_ROWS)]
    pub max_rows: usize,
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
}

/// Buttons that export the `DataGrid` around them.
///
/// ```rust,ignore
/// DataGrid { data: people, columns,
///     DataGridExport { file_name: "people" }
/// }
/// ```
#[component]
pub fn DataGridExport<T: GridRow + PartialEq + 'static>(props: DataGridExportProps) -> Element {
    // Put there by `DataGrid`; outside one there is nothing to export.
    let Some(grid) = try_use_context::<GridHandle<T>>() else {
        return rsx! {};
    };

    let locale = grid.locale();
    let (label, csv_label, print_label) = {
        let locale = locale.read();
        (
            locale.export_label.to_string(),
            locale.export_csv.to_string(),
            locale.print.to_string(),
        )
    };

    // What the user is told while an export is on its way, or after it failed.
    let mut busy = use_signal(|| false);
    let mut problem = use_signal(|| None::<String>);

    let file_name = props.file_name.clone();
    let scope = props.scope;
    let max_rows = props.max_rows;
    let options = props.csv.clone();

    // Runs `then` with the table, once there is one. A server's rows take a
    // while to arrive, so this waits for them, and says what went wrong if they
    // do not.
    let export = Callback::new(move |then: Callback<ExportTable>| {
        if busy() {
            return;
        }
        busy.set(true);
        problem.set(None);
        spawn(async move {
            let table = match scope {
                ExportScope::Filtered => grid.export_all(max_rows).await,
                _ => Ok(grid.export_table(scope).unwrap_or_default()),
            };
            match table {
                Ok(table) => then.call(table),
                Err(error) => {
                    let locale = locale.read();
                    problem.set(Some(match error {
                        ExportError::TooMany { total, max } => locale.export_too_many(total, max),
                        ExportError::Source(message) => locale.export_failed(&message),
                    }));
                }
            }
            busy.set(false);
        });
    });

    let export_csv = move |_| {
        let file_name = file_name.clone();
        let options = options.clone();
        export.call(Callback::new(move |table: ExportTable| {
            save_file(
                &format!("{file_name}.csv"),
                "text/csv;charset=utf-8",
                to_csv(&table, &options),
            );
        }));
    };

    let print_table = move |_| {
        export.call(Callback::new(|table: ExportTable| {
            let options = HtmlOptions {
                style: true,
                ..HtmlOptions::default()
            };
            print_html(to_html(&table, &options));
        }));
    };

    // A `class` of the caller's joins this one rather than fighting it.
    let mut attributes = props.attributes.clone();
    let class = merge_class(&mut attributes, "dg-export");

    rsx! {
        document::Link { rel: "stylesheet", href: STYLE }

        div {
            class,
            role: "group",
            aria_label: label,
            aria_busy: busy().then_some("true"),
            ..attributes,
            button {
                r#type: "button",
                "data-export": "csv",
                disabled: busy(),
                onclick: export_csv,
                "{csv_label}"
            }
            if props.print {
                button {
                    r#type: "button",
                    "data-export": "print",
                    disabled: busy(),
                    onclick: print_table,
                    "{print_label}"
                }
            }
            // Heard by a screen reader as it changes, and empty most of the time.
            span { class: "dg-export-status", role: "status",
                if busy() {
                    "{locale.read().export_busy}"
                } else if let Some(problem) = problem() {
                    "{problem}"
                }
            }
        }
    }
}
