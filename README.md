# dioxus-datagrid

A typed, accessible data grid for **Dioxus 0.7** — sorting, filtering, search, paging, selection
and keyboard navigation — that runs on web, desktop and mobile.

> **Status: Phase 0 (Verifikation & Setup) abgeschlossen.** Es gibt noch keine benutzbare
> Komponente. `PLAN.md` beschreibt den Weg dorthin, `docs/VERIFICATION.md` was davon bereits gegen
> die echten APIs geprüft ist.

## Aufbau

Schwere Logik lebt versioniert auf crates.io, damit Bugfixes alle Nutzer erreichen. Die dünne
gestylte Hülle wird ins Nutzerprojekt kopiert und gehört danach dem Nutzer.

| | |
|---|---|
| [`crates/datagrid-core`](crates/datagrid-core) | Pure Rust. Sortierung, Filter, Paging, Selection, Virtualisierungs-Mathematik. Keine Dioxus-Abhängigkeit. |
| [`crates/dioxus-datagrid`](crates/dioxus-datagrid) | Hooks, unstyled Primitives, ARIA-Grid-Pattern. Kein `web-sys`. |
| [`registry/`](registry) | Gestylte Komponente für `dx components add data_grid`. |
| [`playground/`](playground) | Dioxus-App für manuelle Tests. Aktuell der Phase-0-Plattform-Spike. |

## Entwicklung

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Playground starten:

```bash
dx serve --web --package playground
```

Registry-Smoke-Test (prüft `dx components add` end to end):

```bash
bash scripts/registry-smoke.sh
```

## Dokumentation

- [`PLAN.md`](PLAN.md) — verbindliche Arbeitsgrundlage, Phasen und Akzeptanzkriterien
- [`docs/VERIFICATION.md`](docs/VERIFICATION.md) — Phase-0-Ergebnisse, jeweils mit Quelle und Messwert
- [`docs/DECISIONS.md`](docs/DECISIONS.md) — Architekturentscheidungen
- [`CLAUDE.md`](CLAUDE.md) — Arbeitsregeln

## Lizenz

MIT OR Apache-2.0, nach Wahl.
