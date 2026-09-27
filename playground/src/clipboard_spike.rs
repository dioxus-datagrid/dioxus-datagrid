//! Phase 12 spike: what the clipboard lets through `document::eval`.
//!
//! Dioxus has no clipboard API of its own, and its `ClipboardData` is an empty
//! struct — `onpaste` says *that* something was pasted, never *what*. So copy
//! and paste have to go through `document::eval`, and this page answers the
//! questions that decide how:
//!
//! 1. Does `navigator.clipboard.writeText` succeed from inside an eval that a
//!    click handler started, or is the user gesture already gone by then?
//! 2. Does the old `textarea` + `execCommand("copy")` route still work as a
//!    fallback?
//! 3. Does `navigator.clipboard.readText` work, or does it need a permission
//!    the grid cannot ask for?
//! 4. Can an eval install a lasting `paste` listener that hands the text back
//!    to Rust over the eval channel?
//!
//! Everything a run finds is rendered into the page, so the same page can be
//! driven by Playwright in WebKit, where the answers are expected to differ.
//! The results belong in `docs/VERIFICATION.md`; this module goes away once
//! they are recorded.

use dioxus::prelude::*;

/// The payload every write test puts on the clipboard: two TSV rows, with the
/// characters that a naive `format!` into JavaScript would break on.
const PAYLOAD: &str = "Name\tNote\nZoe \"Z\" Bauer\tline one\\line two";

/// What one probe found, rendered as one line.
#[derive(Clone, PartialEq, Default)]
struct Probe {
    /// Whether the browser let it happen.
    ok: bool,
    /// Whatever the probe wants to say: the error, the text read back, the
    /// state of the user activation.
    detail: String,
}

impl Probe {
    fn from_json(value: &serde_json::Value) -> Self {
        Self {
            ok: value.get("ok").and_then(serde_json::Value::as_bool) == Some(true),
            detail: value
                .get("detail")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("no detail")
                .to_owned(),
        }
    }

    fn failed(detail: impl Into<String>) -> Self {
        Self {
            ok: false,
            detail: detail.into(),
        }
    }
}

/// Runs one probe: sends [`PAYLOAD`] into the script, waits for its verdict.
///
/// The text travels over the eval channel rather than being formatted into the
/// source, which is what the real implementation will have to do: a TSV payload
/// carries quotes, tabs and newlines.
async fn probe(script: &str, send_payload: bool) -> Probe {
    let eval = document::eval(script);
    if send_payload && let Err(error) = eval.send(PAYLOAD) {
        return Probe::failed(format!("send failed: {error}"));
    }
    match eval.join::<serde_json::Value>().await {
        Ok(value) => Probe::from_json(&value),
        Err(error) => Probe::failed(format!("eval failed: {error}")),
    }
}

/// The clipboard API, from inside an eval a click handler started. The verdict
/// carries `navigator.userActivation.isActive`, which is the question behind
/// the question: a gesture that the eval has already lost would explain a
/// refusal.
const WRITE_API: &str = r#"
    const text = await dioxus.recv();
    const active = navigator.userActivation ? navigator.userActivation.isActive : null;
    const secure = window.isSecureContext;
    if (!navigator.clipboard) {
        return { ok: false, detail: `no navigator.clipboard, secure=${secure}` };
    }
    try {
        await navigator.clipboard.writeText(text);
        return { ok: true, detail: `written, userActivation=${active}, secure=${secure}` };
    } catch (error) {
        return { ok: false, detail: `${error}, userActivation=${active}, secure=${secure}` };
    }
"#;

/// The route from before the clipboard API: a hidden textarea and
/// `execCommand`. Deprecated everywhere, still implemented everywhere.
const WRITE_EXEC_COMMAND: &str = r#"
    const text = await dioxus.recv();
    const area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.top = "-1000px";
    document.body.appendChild(area);
    area.select();
    try {
        const ok = document.execCommand("copy");
        return { ok, detail: ok ? "execCommand copy returned true" : "execCommand copy returned false" };
    } catch (error) {
        return { ok: false, detail: String(error) };
    } finally {
        area.remove();
    }
"#;

/// Reading, which is the half that browsers guard hardest.
const READ_API: &str = r#"
    const active = navigator.userActivation ? navigator.userActivation.isActive : null;
    if (!navigator.clipboard || !navigator.clipboard.readText) {
        return { ok: false, detail: "no navigator.clipboard.readText" };
    }
    let permission = "unknown";
    try {
        const status = await navigator.permissions.query({ name: "clipboard-read" });
        permission = status.state;
    } catch (error) {
        permission = `query failed: ${error}`;
    }
    try {
        const text = await navigator.clipboard.readText();
        return { ok: true, detail: `read ${text.length} chars, permission=${permission}` };
    } catch (error) {
        return { ok: false, detail: `${error}, permission=${permission}, userActivation=${active}` };
    }
"#;

/// A `paste` listener that outlives the eval that installed it and hands every
/// pasted text back over the channel. The script never returns, so the
/// evaluator — and with it the channel — stays alive.
const PASTE_LISTENER: &str = r#"
    document.addEventListener("paste", (event) => {
        const text = event.clipboardData ? event.clipboardData.getData("text/plain") : "";
        dioxus.send(text);
    });
    dioxus.send("listening");
    await new Promise(() => {});
"#;

/// The spike page: one button per probe, every verdict rendered.
#[component]
pub fn ClipboardSpike() -> Element {
    let mut write_api = use_signal(|| None::<Probe>);
    let mut write_exec = use_signal(|| None::<Probe>);
    let mut read_api = use_signal(|| None::<Probe>);
    let mut pasted = use_signal(|| None::<Probe>);

    // Installed once, for as long as the page lives.
    use_hook(move || {
        spawn(async move {
            let mut eval = document::eval(PASTE_LISTENER);
            loop {
                match eval.recv::<String>().await {
                    // The listener reports itself once, then every paste. Both
                    // count as working: what would fail is silence.
                    Ok(text) => pasted.set(Some(Probe {
                        ok: true,
                        detail: if text == "listening" {
                            "listener installed, nothing pasted yet".to_owned()
                        } else {
                            format!("pasted {} chars: {text:?}", text.len())
                        },
                    })),
                    Err(error) => {
                        pasted.set(Some(Probe::failed(format!("channel closed: {error}"))));
                        return;
                    }
                }
            }
        });
    });

    rsx! {
        section { class: "spike", "data-testid": "clipboard-spike",
            h2 { "Clipboard spike" }
            p {
                "Phase 12. Every verdict is rendered, so Playwright can read the same page in "
                "WebKit. The payload carries a tab, a newline, a quote and a backslash."
            }
            div { class: "spike-row",
                button {
                    "data-testid": "spike-write-api",
                    onclick: move |_| async move {
                        write_api.set(Some(probe(WRITE_API, true).await));
                    },
                    "navigator.clipboard.writeText"
                }
                ProbeLine { testid: "spike-write-api-result", probe: write_api() }
            }
            div { class: "spike-row",
                button {
                    "data-testid": "spike-write-exec",
                    onclick: move |_| async move {
                        write_exec.set(Some(probe(WRITE_EXEC_COMMAND, true).await));
                    },
                    "textarea + execCommand"
                }
                ProbeLine { testid: "spike-write-exec-result", probe: write_exec() }
            }
            div { class: "spike-row",
                button {
                    "data-testid": "spike-read-api",
                    onclick: move |_| async move {
                        read_api.set(Some(probe(READ_API, false).await));
                    },
                    "navigator.clipboard.readText"
                }
                ProbeLine { testid: "spike-read-api-result", probe: read_api() }
            }
            div { class: "spike-row",
                span { "Paste anywhere (Ctrl+V):" }
                ProbeLine { testid: "spike-paste-result", probe: pasted() }
            }
        }
    }
}

/// One verdict: whether it worked, and what the browser said.
#[component]
fn ProbeLine(testid: String, probe: Option<Probe>) -> Element {
    let (state, detail) = match &probe {
        None => ("pending", "not run".to_owned()),
        Some(probe) if probe.ok => ("ok", probe.detail.clone()),
        Some(probe) => ("failed", probe.detail.clone()),
    };

    rsx! {
        output { "data-testid": "{testid}", "data-state": state, "{state}: {detail}" }
    }
}
