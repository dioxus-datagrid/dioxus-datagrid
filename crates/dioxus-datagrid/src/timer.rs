//! Waiting, without `web-sys`.
//!
//! Dioxus has no timer of its own. Every platform it runs on already brings one
//! into the dependency tree: `dioxus-web` depends on `gloo-timers`, and desktop,
//! mobile and server renderers run on a Tokio runtime with its time driver
//! enabled. This is the same split `dioxus-sdk-time` makes.

use std::time::Duration;

/// Completes after `duration`.
#[cfg(not(target_family = "wasm"))]
pub(crate) async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

/// Completes after `duration`.
#[cfg(target_family = "wasm")]
pub(crate) async fn sleep(duration: Duration) {
    gloo_timers::future::sleep(duration).await;
}
