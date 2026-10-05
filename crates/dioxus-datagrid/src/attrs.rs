//! Making one element out of a component's own attributes and the caller's.

use dioxus::core::AttributeValue;
use dioxus::prelude::*;

/// Takes the `class` attributes out of a spread and joins them with `base`.
///
/// A component that writes a class of its own *and* spreads
/// `#[props(extends = GlobalAttributes)]` attributes onto the same element puts
/// two `class` attributes on it, and then one of them loses: a browser reading
/// server-rendered HTML keeps the first, while `setAttribute` keeps the last.
/// Either way a caller's `class` either disappears or takes the component's
/// styling with it.
///
/// So the component joins them itself and spreads what is left:
///
/// ```
/// # use dioxus::prelude::*;
/// # use dioxus_datagrid::merge_class;
/// #[component]
/// fn Card(#[props(extends = GlobalAttributes)] attributes: Vec<Attribute>) -> Element {
///     let mut attributes = attributes;
///     let class = merge_class(&mut attributes, "card");
///
///     rsx! {
///         div { class, ..attributes }
///     }
/// }
/// ```
///
/// Classes arrive in the order they were written, the base first. A `class`
/// that is not text — one left out by a condition, for instance — contributes
/// nothing, and no empty class survives.
#[must_use]
pub fn merge_class(attributes: &mut Vec<Attribute>, base: &str) -> String {
    let mut classes = String::from(base);
    attributes.retain(|attribute| {
        if attribute.name != "class" {
            return true;
        }
        if let AttributeValue::Text(text) = &attribute.value {
            let text = text.trim();
            if !text.is_empty() {
                if !classes.is_empty() {
                    classes.push(' ');
                }
                classes.push_str(text);
            }
        }
        false
    });
    classes
}
