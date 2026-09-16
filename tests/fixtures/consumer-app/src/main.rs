//! Consumer fixture for the registry smoke test.
//!
//! `scripts/registry-smoke.sh` runs `dx components add data_grid` against this
//! crate and then checks that it still compiles.

mod components;

use dioxus::prelude::*;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        components::data_grid::DataGrid {}
    }
}
