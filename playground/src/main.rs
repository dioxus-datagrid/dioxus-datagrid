//! Playground app.
//!
//! Until phase 2 lands, this only hosts the phase 0 platform spike
//! (see `spike.rs`), which verifies that the event APIs the data grid design
//! depends on actually exist and deliver usable data on every target platform.

mod spike;

use dioxus::prelude::*;

const SPIKE_CSS: Asset = asset!("/assets/spike.css");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "stylesheet", href: SPIKE_CSS }
        spike::PlatformSpike {}
    }
}
