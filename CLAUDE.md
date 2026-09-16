# Arbeitsregeln

`PLAN.md` ist die verbindliche Arbeitsgrundlage. Die Phasen werden strikt in Reihenfolge
abgearbeitet; eine Phase endet erst, wenn alle ihre Akzeptanzkriterien erfüllt sind.
Abweichungen vom Plan sind erlaubt, wenn Realität (APIs, Tooling) es erfordert, müssen aber in
`docs/DECISIONS.md` begründet werden.

## Keine APIs erfinden

Dioxus-0.7-APIs vor Verwendung gegen die tatsächliche Quelle prüfen — docs.rs, den Dioxus-Quellcode
oder `DioxusLabs/dioxus-components`. Bei Unklarheit einen kleinen Spike im `playground/` bauen und
kompilieren lassen, statt zu raten. Ergebnisse solcher Prüfungen gehören in `docs/VERIFICATION.md`.

## Architekturgrenzen

- `datagrid-core` bleibt frei von Dioxus und von `web-sys`. Pure Rust, deterministisch, testbar.
- `dioxus-datagrid` bleibt frei von `web-sys`. Plattformneutral: Scroll über `onscroll`, Maße über
  `onmounted` + `get_client_rect()`, Resize über `onresize`, Gesten über Pointer-Events.
  CI erzwingt das über `cargo tree -p dioxus-datagrid -e normal | grep web-sys` (muss leer sein).
- Die Registry-Komponente `data_grid` bleibt dünn (< ~250 Zeilen). Neue Logik gehört in die Crates,
  nicht in `component.rs`. Styling ausschließlich über die Theme-Variablen aus
  `dx-components-theme.css`, keine hardcodierten Farbwerte.

## Code

- Edition 2024, MSRV 1.85.
- Keine `unwrap`/`expect`/`panic!` in Bibliothekscode — die Workspace-Lints stellen das auf `warn`;
  Tests dürfen per `#![allow(...)]` im Testmodul ausscheren.
- Alle öffentlichen Items dokumentiert (`missing_docs` ist ein Workspace-Lint).
- Nach jedem Schritt: `cargo fmt --all`, `cargo clippy --workspace --all-targets --all-features
  -- -D warnings`, `cargo test --workspace --all-features`.
- Kleine, thematisch saubere Commits im Conventional-Commits-Format.

## Stoppen und nachfragen vor

- Veröffentlichen auf crates.io
- Anlegen oder Pushen von Git-Tags oder Releases
- Änderungen an Lizenz oder Crate-Namen
- Hinzufügen schwergewichtiger Dependencies

## Am Ende jeder Phase

Kurze Zusammenfassung: umgesetzt, Abweichungen, offene Punkte.
