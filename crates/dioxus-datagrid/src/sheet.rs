//! A sheet: a panel that slides up from the bottom edge of a narrow screen and
//! holds what does not fit beside the table.

use crate::GridHandle;
use datagrid_core::GridRow as GridRowKey;
use dioxus::prelude::*;

/// A panel over the bottom of the screen with a title and a close button, for
/// what a narrow screen has no room for beside the table: the column filters.
///
/// Renders nothing while `open` is false. Open, it is a `role="dialog"` named by
/// its title, with `data-sheet`, and a `position: fixed` layer behind it
/// (`data-sheet-backdrop`) that closes it when tapped; position and style both
/// yourself. `Escape` or the close button call `on_close`.
///
/// The sheet is not modal (`aria-modal="false"`): what goes in it is not known,
/// and a focus trap needs to know where the last control is. Focus starts on the
/// close button; giving it back to what opened the sheet is up to `on_close`.
#[component]
pub fn GridSheet<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// Whether the sheet is shown.
    open: bool,
    /// Called when the sheet asks to be closed.
    on_close: EventHandler<()>,
    /// The title, which is also the sheet's accessible name. Defaults to the
    /// locale's `filters`.
    #[props(default)]
    title: Option<String>,
    #[props(extends = GlobalAttributes)] attributes: Vec<Attribute>,
    children: Element,
) -> Element {
    let title_id = use_hook(|| format!("dg-sheet-{}", crate::filter_menu::next_id()));
    if !open {
        return rsx! {};
    }
    let locale = grid.locale();
    let locale = locale.read().clone();
    let title = title.unwrap_or_else(|| locale.filters.to_string());

    rsx! {
        div {
            "data-sheet-backdrop": "",
            aria_hidden: "true",
            style: "position: fixed; inset: 0;",
            onclick: move |_| on_close.call(()),
        }
        div {
            role: "dialog",
            aria_modal: "false",
            aria_labelledby: "{title_id}",
            "data-sheet": "",
            style: "position: fixed; inset-inline: 0; bottom: 0;",
            onkeydown: move |event: KeyboardEvent| {
                if event.key() == Key::Escape {
                    event.prevent_default();
                    on_close.call(());
                }
            },
            ..attributes,
            div { "data-sheet-head": "",
                h2 { id: "{title_id}", "{title}" }
                button {
                    r#type: "button",
                    "data-sheet-close": "",
                    onmounted: move |event| {
                        let data = event.data();
                        spawn(async move {
                            let _ = data.set_focus(true).await;
                        });
                    },
                    onclick: move |_| on_close.call(()),
                    "{locale.close}"
                }
            }
            div { "data-sheet-body": "", {children} }
        }
    }
}

/// What does not fit beside a table on a narrow screen: shown as it is on a wide
/// one, and in a [`GridSheet`] behind a button on a [compact](GridHandle::is_compact)
/// one.
///
/// The button is `aria-haspopup="dialog"` and `aria-expanded`, and gets the focus
/// back when the sheet closes. The sheet closes itself when the grid stops being
/// compact. `sheet_class` goes on the sheet; the button takes `button_class`.
#[component]
pub fn GridCompactSheet<T: GridRowKey + PartialEq + 'static>(
    grid: GridHandle<T>,
    /// The text of the button, which is also the sheet's title. Defaults to the
    /// locale's `filters`.
    #[props(default)]
    label: Option<String>,
    /// The class of the button.
    #[props(default)]
    button_class: Option<String>,
    /// The class of the sheet.
    #[props(default)]
    sheet_class: Option<String>,
    children: Element,
) -> Element {
    let mut open = use_signal(|| false);
    let mut opener = use_signal(|| None::<std::rc::Rc<MountedData>>);
    use_effect(move || {
        if !grid.is_compact() {
            open.set(false);
        }
    });

    if !grid.is_compact() {
        return rsx! { {children} };
    }
    let label = label.unwrap_or_else(|| grid.locale().read().filters.to_string());

    rsx! {
        button {
            r#type: "button",
            class: button_class,
            aria_haspopup: "dialog",
            aria_expanded: "{open()}",
            onmounted: move |event| opener.set(Some(event.data())),
            onclick: move |_| open.set(!open()),
            "{label}"
        }
        GridSheet {
            grid,
            open: open(),
            title: label.clone(),
            on_close: move |_| {
                open.set(false);
                if let Some(opener) = opener.peek().clone() {
                    spawn(async move {
                        let _ = opener.set_focus(true).await;
                    });
                }
            },
            class: sheet_class,
            {children}
        }
    }
}
