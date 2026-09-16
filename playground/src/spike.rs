//! Phase 0 platform spike.
//!
//! Every panel here answers one question from `PLAN.md` section 6, phase 0,
//! point 3: does the API exist in Dioxus 0.7, and does it deliver usable data on
//! web, desktop and mobile? Each panel writes its result into a `data-spike-*`
//! attribute so the verification can be read off the DOM instead of a screenshot.

use dioxus::prelude::*;

/// Root of the spike page.
#[component]
pub fn PlatformSpike() -> Element {
    rsx! {
        main { class: "spike",
            h1 { "dioxus-datagrid — phase 0 platform spike" }
            p { class: "hint",
                "Every panel reports the raw values it received. A panel that stays at "
                code { "—" }
                " means the API does not deliver data on this platform."
            }
            ReadSignalPanel { label: "ReadSignal<T> works as a prop type" }
            ScrollPanel {}
            MountedPanel {}
            ResizePanel {}
            PointerPanel {}
        }
    }
}

/// Verifies that `ReadSignal<T>` is the read-only signal prop type in 0.7 and
/// that a plain value is coerced into it at the call site.
#[component]
fn ReadSignalPanel(label: ReadSignal<String>) -> Element {
    rsx! {
        section { class: "panel", "data-spike": "read-signal",
            h2 { "1. ReadSignal" }
            output { "data-spike-value": "{label}", "{label}" }
        }
    }
}

/// Verifies `onscroll` on a nested scroll container: the virtualization math in
/// `datagrid-core` needs `scroll_top`, `client_height` and `scroll_height`.
#[component]
fn ScrollPanel() -> Element {
    let mut scroll_top = use_signal(|| f64::NAN);
    let mut client_height = use_signal(|| 0);
    let mut scroll_height = use_signal(|| 0);
    let mut events = use_signal(|| 0u32);

    let reported = if scroll_top().is_nan() {
        "—".to_string()
    } else {
        format!(
            "scroll_top={:.0} client_height={} scroll_height={}",
            scroll_top(),
            client_height(),
            scroll_height()
        )
    };

    rsx! {
        section { class: "panel", "data-spike": "scroll",
            h2 { "2. onscroll on a nested container" }
            div {
                class: "scrollbox",
                onscroll: move |event| {
                    let data = event.data();
                    scroll_top.set(data.scroll_top());
                    client_height.set(data.client_height());
                    scroll_height.set(data.scroll_height());
                    events += 1;
                },
                for i in 0..200 {
                    div { class: "scrollrow", "row {i}" }
                }
            }
            output {
                "data-spike-value": "{reported}",
                "data-spike-events": "{events}",
                "{reported} ({events} events)"
            }
        }
    }
}

/// Verifies `onmounted` plus `MountedData::get_client_rect()`, the
/// platform-neutral way to measure the viewport without `web-sys`.
///
/// The panel reports the rect twice: once as measured inside the `onmounted`
/// handler, and once measured on demand from the retained `MountedData`. If the
/// two disagree, the mount-time measurement raced the layout and phase 2 must
/// not rely on it.
#[component]
fn MountedPanel() -> Element {
    let mut at_mount = use_signal(|| None::<Rect>);
    let mut on_demand = use_signal(|| None::<Rect>);
    let mut element = use_signal(|| None::<std::rc::Rc<MountedData>>);

    let reported = format!(
        "at_mount={} on_demand={}",
        Rect::describe(at_mount()),
        Rect::describe(on_demand())
    );

    rsx! {
        section { class: "panel", "data-spike": "mounted",
            h2 { "3. onmounted + get_client_rect" }
            div {
                class: "measured",
                onmounted: move |event| async move {
                    let data = event.data();
                    if let Ok(measured) = data.get_client_rect().await {
                        at_mount.set(Some(Rect::from(measured)));
                    }
                    element.set(Some(data));
                },
                "measured element"
            }
            button {
                class: "remeasure",
                onclick: move |_| async move {
                    let Some(data) = element() else { return };
                    if let Ok(measured) = data.get_client_rect().await {
                        on_demand.set(Some(Rect::from(measured)));
                    }
                },
                "re-measure now"
            }
            output { "data-spike-value": "{reported}", "{reported}" }
        }
    }
}

/// The four numbers of a measured rect, kept separate from euclid so the spike
/// can report x/y alongside width/height.
#[derive(Clone, Copy, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl From<dioxus::html::geometry::euclid::Rect<f64, dioxus::html::geometry::Pixels>> for Rect {
    fn from(
        rect: dioxus::html::geometry::euclid::Rect<f64, dioxus::html::geometry::Pixels>,
    ) -> Self {
        Self {
            x: rect.origin.x,
            y: rect.origin.y,
            width: rect.width(),
            height: rect.height(),
        }
    }
}

impl Rect {
    fn describe(rect: Option<Self>) -> String {
        match rect {
            Some(r) => format!(
                "[x={:.1} y={:.1} w={:.1} h={:.1}]",
                r.x, r.y, r.width, r.height
            ),
            None => "—".to_string(),
        }
    }
}

/// Verifies `onresize` (backed by a `ResizeObserver`), which phase 4 needs to
/// keep the virtual viewport in sync when the container is resized.
#[component]
fn ResizePanel() -> Element {
    let mut size = use_signal(|| None::<(f64, f64)>);
    let mut events = use_signal(|| 0u32);

    let reported = match size() {
        Some((w, h)) => format!("content_box width={w:.1} height={h:.1}"),
        None => "—".to_string(),
    };

    rsx! {
        section { class: "panel", "data-spike": "resize",
            h2 { "4. onresize" }
            div {
                class: "resizable",
                onresize: move |event| {
                    if let Ok(measured) = event.data().get_content_box_size() {
                        size.set(Some((measured.width, measured.height)));
                        events += 1;
                    }
                },
                "drag my bottom-right corner"
            }
            output {
                "data-spike-value": "{reported}",
                "data-spike-events": "{events}",
                "{reported} ({events} events)"
            }
        }
    }
}

/// Verifies pointer events and, critically, whether a drag keeps delivering
/// `onpointermove` once the pointer leaves the element it started on. Phase 5
/// (column resize by drag) depends on the answer.
#[component]
fn PointerPanel() -> Element {
    let mut origin = use_signal(|| None::<f64>);
    let mut delta = use_signal(|| 0.0_f64);
    let mut moves = use_signal(|| 0u32);
    let mut left_handle = use_signal(|| false);
    let mut pointer_type = use_signal(String::new);

    let reported = if origin().is_none() && moves() == 0 {
        "—".to_string()
    } else {
        format!(
            "delta_x={:.0} moves={} pointer_type={} left_handle={}",
            delta(),
            moves(),
            pointer_type(),
            left_handle()
        )
    };

    rsx! {
        section { class: "panel", "data-spike": "pointer",
            h2 { "5. Pointer drag (handle-local listeners)" }
            div { class: "draglane",
                div {
                    class: "draghandle",
                    onpointerdown: move |event| {
                        event.prevent_default();
                        origin.set(Some(event.data().client_coordinates().x));
                        pointer_type.set(event.data().pointer_type());
                        delta.set(0.0);
                        moves.set(0);
                        left_handle.set(false);
                    },
                    onpointermove: move |event| {
                        if let Some(start) = origin() {
                            delta.set(event.data().client_coordinates().x - start);
                            moves += 1;
                        }
                    },
                    onpointerup: move |_| {
                        origin.set(None);
                    },
                    onpointerleave: move |_| {
                        if origin().is_some() {
                            left_handle.set(true);
                        }
                    },
                    "drag me sideways, past the handle edge"
                }
            }
            output {
                "data-spike-value": "{reported}",
                "data-spike-moves": "{moves}",
                "{reported}"
            }
        }
        AncestorPointerPanel {}
    }
}

/// The counterpart to [`PointerPanel`]: the handle only *starts* the drag, while
/// `onpointermove`/`onpointerup` live on a large ancestor. This is the candidate
/// technique for phase 5 column resizing, because Dioxus 0.7 exposes no pointer
/// capture API and handle-local listeners stop at the handle's edge.
#[component]
fn AncestorPointerPanel() -> Element {
    let mut origin = use_signal(|| None::<f64>);
    let mut delta = use_signal(|| 0.0_f64);
    let mut moves = use_signal(|| 0u32);
    let mut max_delta = use_signal(|| 0.0_f64);

    let reported = if moves() == 0 {
        "—".to_string()
    } else {
        format!(
            "delta_x={:.0} max_delta_x={:.0} moves={}",
            delta(),
            max_delta(),
            moves()
        )
    };

    rsx! {
        section { class: "panel", "data-spike": "pointer-ancestor",
            h2 { "6. Pointer drag (listeners on a large ancestor)" }
            div {
                class: "draglane wide",
                onpointermove: move |event| {
                    if let Some(start) = origin() {
                        let moved = event.data().client_coordinates().x - start;
                        delta.set(moved);
                        if moved.abs() > max_delta().abs() {
                            max_delta.set(moved);
                        }
                        moves += 1;
                    }
                },
                onpointerup: move |_| {
                    origin.set(None);
                },
                div {
                    class: "draghandle narrow",
                    onpointerdown: move |event| {
                        event.prevent_default();
                        origin.set(Some(event.data().client_coordinates().x));
                        delta.set(0.0);
                        max_delta.set(0.0);
                        moves.set(0);
                    },
                    "⇔"
                }
                span { class: "lanehint", "grab the narrow handle, drag far across this lane" }
            }
            output {
                "data-spike-value": "{reported}",
                "data-spike-moves": "{moves}",
                "{reported}"
            }
        }
    }
}
