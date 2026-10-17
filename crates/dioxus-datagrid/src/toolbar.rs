//! The pieces a toolbar is made of that are not a button of their own: how many
//! rows there are, and which columns to show.

use crate::GridHandle;
use datagrid_core::GridRow as GridRowKey;
use dioxus::prelude::*;

/// How many rows match, and how many of them are selected.
///
/// A polite live region would read every keystroke of the search aloud, so this
/// is plain text: it is there to be found, not announced.
#[component]
pub fn GridCount<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let locale = grid.locale();
    let locale = locale.read().clone();
    let selected = grid.selected_count();

    rsx! {
        span { ..attributes,
            "{locale.row_count(grid.filtered_len())}"
            if selected > 0 {
                ", {locale.selected_count(selected)}"
            }
        }
    }
}

/// A disclosure that shows and hides columns.
///
/// Lists every column the application did not define as hidden, each with a
/// checkbox. The grid refuses to hide its last visible column, so that box is
/// disabled while it is the only one ticked.
///
/// The element is a `details`; the class goes on it, and the list is a `div`
/// with `data-columns-menu` inside it.
#[component]
pub fn GridColumnPicker<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// The text of the disclosure. Defaults to the grid's
    /// [locale](GridHandle::locale).
    #[props(default)]
    label: Option<String>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
) -> Element {
    let mut grid = grid;
    let label = label.unwrap_or_else(|| grid.locale().read().columns.to_string());
    let pickable = grid.pickable_columns();
    let last_visible = grid.visible_column_count() <= 1;

    rsx! {
        details { ..attributes,
            summary { "{label}" }
            div { "data-columns-menu": "",
                for (id , label , visible) in pickable {
                    label { key: "{id}",
                        input {
                            r#type: "checkbox",
                            checked: visible,
                            disabled: visible && last_visible,
                            onchange: move |event| grid.set_column_hidden(id.clone(), !event.checked()),
                        }
                        "{label}"
                    }
                }
            }
        }
    }
}
