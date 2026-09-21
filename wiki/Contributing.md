# Contributing

## What you need

Rust stable (MSRV **1.85**, edition 2024), the Dioxus CLI (`cargo install dioxus-cli --version 0.7.10 --locked`),
and Node only if you want to run the end-to-end tests. On Linux the WebView build needs system
packages — `scripts/install-linux-webview-deps.sh` installs them.

## The three commands

Run all three after every change. CI runs exactly the same thing, so a green run here is a green run
there.

```bash
cargo fmt --all
```

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

```bash
cargo test --workspace --all-features
```

## How the project is tested

Five layers, fastest first. Put a test in the highest layer that can actually catch the bug.

| Layer | Where | What it is for |
|---|---|---|
| Core unit and property tests | `crates/datagrid-core/tests/` | Sorting, filtering, operators, grouping, aggregates, paging, selection, virtualization maths, keyboard. Pure functions, no DOM, milliseconds. Property tests (`proptest`) state the invariants: a page covers every row exactly once, groups cover the filtered rows without overlap, aggregates match a naive computation, a server-planned page equals the local one. |
| SSR tests | `crates/dioxus-datagrid/tests/` | What actually reaches the DOM: roles, ARIA attributes, the rendered texts of a locale. Rendered to a string, no browser. |
| Example tests | `examples/fullstack/src/db.rs` | That the SQL translation of a `GridQuery` answers exactly like the local grid, filtered, sorted and grouped. |
| End-to-end | `tests/e2e/` | Playwright against the playground and both server examples, in Chromium and WebKit: interaction, focus, virtualization with 100,000 rows, axe in light and dark. |
| Registry smoke test | `scripts/registry-smoke.sh` | That `dx components add` into a fresh project actually compiles. |

Layout-dependent behaviour — anything that depends on a measured size — cannot be judged by SSR and
belongs in the end-to-end tests (ADR-0016).

## The playground and the end-to-end tests

```bash
dx serve --web --package playground
```

The playground renders the registry component with a switch for every mode, and is what Playwright
drives. It carries a **copy** of `registry/data_grid` (ADR-0011, so that `dx serve` sees a normal
local module). After changing the component, refresh the copy — CI fails if the two drift apart:

```bash
bash scripts/sync-playground-component.sh
```

With the playground already serving on port 8080, the tests reuse it:

```bash
cd tests/e2e && npm ci && npx playwright test
```

`dx serve` watches the playground's own files, not the crate sources. After changing a crate,
restart it.

## House rules

- **No invented APIs.** Check a Dioxus 0.7 API against docs.rs, the Dioxus source or
  `DioxusLabs/dioxus-components` before using it. If it is unclear, build a small spike in
  `playground/` and let it compile rather than guessing. Write the result into
  [`docs/VERIFICATION.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/docs/VERIFICATION.md).
- **No `unwrap`, `expect` or `panic!` in library code.** The workspace lints warn about them; test
  modules may opt out.
- **Every public item documented** — `missing_docs` is a workspace lint.
- **Keep the boundaries** described in [Architecture](Architecture.md): no Dioxus in
  `datagrid-core`, no `web-sys` in `dioxus-datagrid`, no logic in the registry components, no
  hard-coded colours.
- **Small, single-subject commits** in [Conventional Commits](https://www.conventionalcommits.org/)
  form.
- **New behaviour lands with its tests**, and with the ARIA and keyboard notes in
  `docs/ACCESSIBILITY.md` when it is visible to a user.
- **Deviating from the plan is allowed** when reality demands it — but it is written down and
  argued for in `docs/DECISIONS.md` as a numbered decision record.

## How the work is planned

Phases from [`PLAN.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/PLAN.md) (0–6,
done) and [`docs/ROADMAP.md`](https://github.com/dioxus-datagrid/dioxus-datagrid/blob/main/docs/ROADMAP.md)
(7–15, then 1.0) are worked strictly in order, and a phase is finished only when all of its
acceptance criteria hold — each one is a named, runnable test, not a judgement call. Each phase ends
with a summary: what was built, where it deviated, what is still open. See
[Roadmap](Roadmap.md) for the phases in English.

## What CI checks

| Workflow | Job | |
|---|---|---|
| CI | fmt + clippy + test | The three commands above |
| | cargo doc | Documentation builds without warnings |
| | wasm target | The crates build for `wasm32-unknown-unknown` |
| | feature combinations | Default, each feature alone, all features |
| | build (ubuntu / macos / windows) | The examples build on all three |
| | web-sys guard | `dioxus-datagrid` has no `web-sys` in its dependency tree |
| E2E | playground matches registry | No drift between the component and its playground copy |
| | playwright (chromium / webkit) | The end-to-end suite in both browsers |
| Registry | dx components add | `dx components add` from this checkout compiles |
| | dx components add (crates.io) | What a user really gets: the component on `main`, built against the **published** crates |

That last job is expected to fail after a phase that binds the components to new crate APIs, and
going green again is what the [release process](Release-Process.md) is for.
