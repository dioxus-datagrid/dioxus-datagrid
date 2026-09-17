//! A styled data grid, composed from the unstyled primitives in
//! `dioxus-datagrid`.
//!
//! This file is yours once `dx components add` copies it in — change the markup
//! and the classes freely. Sorting, filtering, paging, selection and keyboard
//! navigation live in the `dioxus-datagrid` crate, so they keep improving
//! without you having to re-copy this file.

use dioxus::prelude::*;
use dioxus_datagrid::primitives::{
    GridBody, GridColumnFilter, GridHeader, GridPagination, GridRoot, GridSearch, VirtualGridBody,
};
use dioxus_datagrid::{Column, ColumnWidth, GridOptions, GridRow, SelectionMode, use_grid};

const THEME: Asset = asset!("/assets/dx-components-theme.css");
const STYLE: Asset = asset!("/src/components/data_grid/style.css");

/// Props for [`DataGrid`].
#[derive(Props, Clone, PartialEq)]
pub struct DataGridProps<T: GridRow + PartialEq + 'static> {
    /// The rows to show.
    pub data: ReadSignal<Vec<T>>,
    /// The columns to show them in.
    pub columns: ReadSignal<Vec<Column<T>>>,
    /// Rows per page. Omit to show every row at once.
    #[props(default)]
    pub page_size: Option<usize>,
    /// Whether and how rows can be selected.
    #[props(default)]
    pub selection: SelectionMode,
    /// Whether to show the search box above the grid.
    #[props(default = true)]
    pub searchable: bool,
    /// Whether to show a filter input for every filterable column.
    #[props(default)]
    pub column_filters: bool,
    /// Placeholder for the search box.
    #[props(default = String::from("Search"))]
    pub search_placeholder: String,
    /// What to show when no row matches.
    #[props(default = String::from("No matching rows"))]
    pub empty_message: String,
    /// Called whenever the set of selected rows changes.
    #[props(default)]
    pub on_selection_change: Option<EventHandler<Vec<T::Key>>>,
    /// Renders only the rows in view, for large data sets. Every row is then
    /// exactly this many pixels tall. Needs `height`, and is usually combined
    /// with no `page_size`.
    #[props(default)]
    pub row_height: Option<f64>,
    /// With `row_height`: extra rows rendered above and below the visible ones.
    /// Raise it if fast scrolling, typically a touch fling, shows blank rows
    /// before the next render catches up.
    #[props(default = 5)]
    pub overscan: usize,
    /// A CSS height for the grid, such as `"480px"`. The grid scrolls within it.
    #[props(default)]
    pub height: Option<String>,
    #[props(extends = GlobalAttributes)]
    pub attributes: Vec<Attribute>,
}

/// A sortable, filterable, pageable data grid.
///
/// ```rust,ignore
/// DataGrid {
///     data: users,
///     columns,
///     page_size: 25,
///     selection: SelectionMode::Multi,
///     on_selection_change: move |keys: Vec<u32>| tracing::info!("{keys:?}"),
/// }
/// ```
#[component]
pub fn DataGrid<T: GridRow + PartialEq + 'static>(props: DataGridProps<T>) -> Element {
    let mut options = GridOptions::default().selection(props.selection);
    options.page_size = props.page_size;

    let grid = use_grid(props.data, props.columns, options);

    // Report selection changes outward. Reading the keys here is what subscribes
    // the effect to them.
    let on_change = props.on_selection_change;
    use_effect(move || {
        let keys = grid.selected_keys();
        if let Some(handler) = &on_change {
            handler.call(keys);
        }
    });

    let is_empty = grid.filtered_len() == 0;
    let height_style = props
        .height
        .as_deref()
        .map_or_else(String::new, |height| format!("height: {height};"));

    // Every row is a CSS subgrid of this one track definition, so the header and
    // the cells stay aligned without anyone measuring anything.
    let template = grid
        .visible_columns()
        .iter()
        .map(|column| match column.spec().width {
            ColumnWidth::Auto => "minmax(6rem, auto)".to_owned(),
            ColumnWidth::Px(width) => format!("{width}px"),
            ColumnWidth::Fraction(fraction) => format!("{fraction}fr"),
        })
        .collect::<Vec<_>>()
        .join(" ");

    rsx! {
        document::Link { rel: "stylesheet", href: THEME }
        document::Link { rel: "stylesheet", href: STYLE }

        div { class: "dg-wrapper", ..props.attributes,

            if props.searchable {
                div { class: "dg-toolbar",
                    GridSearch {
                        grid,
                        class: "dg-search",
                        placeholder: props.search_placeholder,
                    }
                    span { class: "dg-count",
                        "{grid.filtered_len()} rows"
                        if grid.selected_count() > 0 {
                            ", {grid.selected_count()} selected"
                        }
                    }
                }
            }

            // Outside role="grid" on purpose: a filter row inside it would count
            // towards aria-rowcount and shift every aria-rowindex by one.
            if props.column_filters {
                div { class: "dg-filters",
                    for (index , column) in grid.visible_columns().into_iter().enumerate() {
                        if column.spec().is_filterable() {
                            label { key: "{column.id()}", class: "dg-filter",
                                span { "{column.label()}" }
                                GridColumnFilter { grid, column_index: index, class: "dg-search" }
                            }
                        }
                    }
                }
            }

            div { class: "dg-frame",
                // The grid root is the scroll container: a virtualized body reads
                // its scroll position from there.
                GridRoot {
                    grid,
                    class: "dg",
                    style: "--dg-template: {template}; {height_style}",
                    GridHeader { grid, class: "dg-head" }
                    if let Some(row_height) = props.row_height {
                        VirtualGridBody {
                            grid,
                            row_height,
                            overscan: props.overscan,
                            class: "dg-body",
                        }
                    } else {
                        GridBody { grid, class: "dg-body" }
                    }
                }
            }

            if is_empty {
                p { class: "dg-empty", role: "status", "{props.empty_message}" }
            }

            GridPagination { grid, class: "dg-pagination" }
        }
    }
}
