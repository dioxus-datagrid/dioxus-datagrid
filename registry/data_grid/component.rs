//! PHASE 0 STUB — replaced in phase 3 by the real styled component.
//!
//! It exists only so `scripts/registry-smoke.sh` can verify the registry
//! mechanics (manifest discovery, file copy, `mod.rs` wiring, global assets,
//! cargo dependency injection) before any grid code is written.

use dioxus::prelude::*;

/// Placeholder for the styled data grid.
#[component]
pub fn DataGrid() -> Element {
    rsx! {
        div { class: "dg-root", "data_grid placeholder — implemented in phase 3" }
    }
}
