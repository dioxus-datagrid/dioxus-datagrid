# PLAN: dioxus-datagrid

> **Für Claude Code:** Dieses Dokument ist die verbindliche Arbeitsgrundlage. Lies es vollständig,
> bevor du Code schreibst. Arbeite die Phasen strikt in Reihenfolge ab und beginne mit **Phase 0**.
> Jede Phase endet erst, wenn alle Akzeptanzkriterien erfüllt sind. Abweichungen von diesem Plan
> sind erlaubt, wenn die Realität (APIs, Tooling) es erfordert, müssen aber in `docs/DECISIONS.md`
> begründet werden.

---

## 1. Ziel

Ein wiederverwendbares, typsicheres Data Grid für **Dioxus 0.7**, funktional orientiert an
Syncfusion SfGrid, das auf **Web, Desktop und Mobile** läuft und über den Dioxus-CLI-Mechanismus
eingebunden werden kann:

```sh
dx components add data_grid
```

Verbreitung über eine **eigene Registry** (GitHub-Repo). Aufbau und Konventionen orientieren sich
bewusst an `DioxusLabs/dioxus-components`, damit ein späterer Upstream-Beitrag möglich bleibt.

### Architekturprinzip

- **Schwere Logik** lebt versioniert auf crates.io (Bugfixes erreichen alle Nutzer).
- **Dünne gestylte Hülle** (Markup + CSS) wird per `dx components add` ins Nutzerprojekt kopiert
  und gehört danach dem Nutzer.

### Nicht-Ziele (bis einschließlich 0.3)

Inline-Editing, Gruppierung, Pivot, Export, Virtualisierung mit variabler Zeilenhöhe,
Unterstützung des nativen Renderers (Blitz). Desktop und Mobile laufen über WebView, daher gilt
DOM/CSS-Parität als Annahme.

---

## 2. Repository-Struktur

```
dioxus-datagrid/
├── Cargo.toml                    # Workspace
├── CLAUDE.md                     # Arbeitsregeln (siehe Abschnitt 8), verweist auf PLAN.md
├── PLAN.md                       # dieses Dokument
├── docs/
│   ├── VERIFICATION.md           # Ergebnisse Phase 0
│   ├── DECISIONS.md              # Architekturentscheidungen (ADR-light)
│   └── ACCESSIBILITY.md          # umgesetztes ARIA-Grid-Pattern + Tastaturbelegung
├── crates/
│   ├── datagrid-core/            # crates.io: pure Rust, KEINE Dioxus-Abhängigkeit
│   └── dioxus-datagrid/          # crates.io: Hooks, unstyled Primitives, a11y, Virtualisierung
├── registry/
│   ├── component.json            # Registry-Manifest
│   └── data_grid/
│       ├── component.json        # deklariert Cargo-Dependency auf dioxus-datagrid
│       ├── component.rs          # gestylte Komponente (dünn!)
│       └── style.css
├── examples/
│   ├── basic/
│   ├── virtualized/              # 100.000 Zeilen
│   └── server/                   # simulierte async DataSource
├── playground/                   # Dioxus-App für manuelle Tests + Playwright-Ziel
├── tests/
│   ├── e2e/                      # Playwright
│   └── fixtures/consumer-app/    # Minimal-App für Registry-Smoke-Test
└── .github/workflows/
```

Arbeitsname `dioxus-datagrid` / `datagrid-core`. Verfügbarkeit auf crates.io wird in Phase 0
geprüft; bei Konflikt Namen workspaceweit ersetzen und in `DECISIONS.md` festhalten.

---

## 3. Ziel-API (Nordstern)

Die folgenden Snippets beschreiben die gewünschte Developer Experience. Signaturen dürfen
abweichen, wenn Dioxus 0.7 es erfordert.

### 3.1 Gestylte Komponente (aus der Registry)

```rust
#[derive(Clone, PartialEq)]
struct User { id: u32, name: String, email: String, age: u32 }

impl GridRow for User {
    type Key = u32;
    fn key(&self) -> u32 { self.id }
}

#[component]
fn App() -> Element {
    let users = use_signal(load_users);
    let columns = use_hook(|| vec![
        Column::new("name", "Name")
            .cell(|u: &User| rsx! { "{u.name}" })
            .sort_by(|u| u.name.clone())
            .filter_by(|u| u.name.clone()),
        Column::new("age", "Alter")
            .cell(|u: &User| rsx! { "{u.age}" })
            .sort_by(|u| u.age),
    ]);

    rsx! {
        DataGrid {
            data: users,
            columns,
            page_size: 25,
            selection: SelectionMode::Multi,
            on_selection_change: move |keys: Vec<u32>| { /* ... */ },
        }
    }
}
```

### 3.2 Headless (volle Kontrolle über Markup)

```rust
let grid = use_grid(users, columns, GridOptions { page_size: Some(25), ..Default::default() });

rsx! {
    GridRoot { grid,
        GridHeader { grid }
        GridBody { grid }
        GridPagination { grid }
    }
}
```

`GridHandle<T>` ist `Copy` (signal-basiert) und bietet u. a.:
`visible_rows()`, `toggle_sort(column_id, additive: bool)`, `set_filter(column_id, text)`,
`set_search(text)`, `set_page(index)`, `select(key)`, `toggle_select(key)`, `clear_selection()`,
`state()` / `set_state(GridState)`.

---

## 4. Crate-Verantwortlichkeiten

### 4.1 `datagrid-core` (keine Dioxus-, keine web-sys-Abhängigkeit)

```rust
pub trait GridRow: Clone + 'static {
    type Key: Clone + Eq + std::hash::Hash + 'static;
    fn key(&self) -> Self::Key;
}

pub enum SortValue { None, Bool(bool), Int(i64), Float(f64), Text(String) }
// Ord-Implementierung: None zuletzt, Float via total_cmp, Text case-insensitiv (konfigurierbar)
// From-Impls für gängige Typen (String, &str, u32, i64, f64, bool, Option<V>)

pub enum SortDirection { Asc, Desc }
pub struct SortState { pub column: ColumnId, pub direction: SortDirection }

pub struct ColumnSpec<T> {
    pub id: ColumnId,                                   // Cow<'static, str>
    pub sort_key: Option<Rc<dyn Fn(&T) -> SortValue>>,
    pub filter_text: Option<Rc<dyn Fn(&T) -> String>>,
    pub width: ColumnWidth,                             // Auto | Px(f32) | Fraction(f32)
    pub min_width: Option<f32>,
    pub visible: bool,
}

pub struct GridState {                                  // serde optional (Feature `serde`)
    pub sort: Vec<SortState>,                           // Multi-Sort, Reihenfolge = Priorität
    pub column_filters: Vec<(ColumnId, String)>,
    pub search: Option<String>,
    pub page: Option<PageState>,                        // None = kein Paging
    pub column_widths: Vec<(ColumnId, f32)>,
    pub hidden_columns: Vec<ColumnId>,
}

pub struct View { pub indices: Vec<usize>, pub filtered_len: usize, pub page_count: usize }

/// Filter -> stabile Sortierung -> Paging. Liefert Indizes, klont keine Zeilen.
pub fn compute_view<T>(rows: &[T], columns: &[ColumnSpec<T>], state: &GridState) -> View;

pub enum SelectionMode { None, Single, Multi }
pub struct Selection<K> { /* HashSet<K> + Anker für Shift-Range */ }

/// Virtualisierungs-Mathematik, pure Funktion.
pub fn visible_range(scroll_top: f64, viewport_height: f64, row_height: f64,
                     total_rows: usize, overscan: usize) -> std::ops::Range<usize>;

/// Tastaturnavigation als Zustandsmaschine.
pub struct CellFocus { pub row: usize, pub col: usize }
pub enum NavKey { Up, Down, Left, Right, Home, End, CtrlHome, CtrlEnd, PageUp, PageDown }
pub fn navigate(focus: CellFocus, key: NavKey, rows: usize, cols: usize, page_rows: usize) -> CellFocus;

/// Serverseitige Daten (Phase 6).
pub struct GridQuery { pub sort: Vec<SortState>, pub column_filters: Vec<(ColumnId, String)>,
                       pub search: Option<String>, pub page: usize, pub page_size: usize }
pub struct Page<T> { pub rows: Vec<T>, pub total: usize }
pub trait DataSource<T> {
    type Error: std::fmt::Display;
    fn fetch(&self, query: GridQuery) -> impl std::future::Future<Output = Result<Page<T>, Self::Error>>;
}
```

Entscheidung: `Rc` statt `Arc`, da Dioxus pro VirtualDom single-threaded ist und WASM kein `Send`
verlangt. Kein `Send`-Bound auf Futures.

### 4.2 `dioxus-datagrid`

- `Column<T>`: Builder, kapselt `ColumnSpec<T>` plus `cell: Rc<dyn Fn(&T) -> Element>` und
  optionalen `header: Rc<dyn Fn() -> Element>`. `PartialEq` über `id` + `Rc::ptr_eq`.
- `use_grid(...) -> GridHandle<T>`: Signals für `GridState`, `use_memo` für `View`.
- Unstyled Primitives nach dem Muster von `dioxus-primitives`, verbunden über Context:
  `GridRoot`, `GridHeader`, `GridHeaderCell`, `GridBody`, `GridRow`, `GridCell`,
  `GridPagination`, `GridSearch`, `VirtualGridBody` (Phase 4), `ColumnResizeHandle` (Phase 5).
- Alle Primitives akzeptieren `class` und spreaden zusätzliche Attribute (`..attributes`).
- **Plattformneutral:** kein `web-sys`, kein `document::eval` für Kernfunktionen. Scroll über
  `onscroll`, Maße über `onmounted` + `get_client_rect()`, Resize-Gesten über Pointer-Events.
- Feature-Flags: `serde` (State-Persistenz), `virtualization` (default an, falls Größe unkritisch).

### 4.3 Registry-Komponente `data_grid`

- Komponiert ausschließlich Primitives aus `dioxus-datagrid`. Ziel: `component.rs` < ~250 Zeilen.
- Styling ausschließlich über die CSS-Variablen des offiziellen Themes (`dx-components.css`),
  damit das Grid neben offiziellen Komponenten stimmig aussieht. Keine eigenen Farbwerte
  hardcoden, keine Tailwind-Pflicht.
- Klassen-Präfix `dg-` (z. B. `dg-row`, `dg-cell--sorted`, `dg-row--selected`).
- Sticky Header, horizontales Scrollen im Container, Tap-Targets ≥ 44px auf Touch
  (`@media (pointer: coarse)`), Light/Dark über die Theme-Variablen.

---

## 5. Barrierefreiheit (Pflicht ab Phase 2)

Umsetzung nach WAI-ARIA Authoring Practices, **Data Grid Pattern**:

- `role="grid"` am Root, `row`, `columnheader`, `gridcell`.
- `aria-sort` an sortierbaren Headern, `aria-selected` an Zeilen bei Selection.
- `aria-rowcount` / `aria-rowindex` (zwingend wegen Paging und Virtualisierung),
  `aria-colcount` / `aria-colindex`.
- Roving `tabindex`: genau eine Zelle mit `tabindex="0"`.
- Tasten: Pfeile, Home/End, Ctrl+Home/End, PageUp/PageDown, Space (Zeile wählen),
  Shift+Space / Shift+Pfeil (Range), Enter auf Header (Sortierung), Shift+Enter (additiv).
- Dokumentation in `docs/ACCESSIBILITY.md`.

---

## 6. Phasen

### Phase 0: Verifikation & Setup

Nichts aus Abschnitt 3–4 implementieren, bevor diese Punkte geprüft und in
`docs/VERIFICATION.md` dokumentiert sind:

1. Aktuelle stabile Dioxus-0.7-Version und MSRV ermitteln; Workspace darauf pinnen (`dioxus = "0.7"`).
2. Name der Read-only-Signal-Prop-Typs in 0.7 prüfen (`ReadSignal` vs. `ReadOnlySignal`).
3. Verfügbarkeit von `onmounted`/`get_client_rect()`, `onscroll`-Daten (`scroll_top`),
   `onresize` und Pointer-Events auf Web **und** Desktop prüfen (kleiner Spike im Playground).
4. `dx components schema` ausführen, Schema speichern unter `docs/component.schema.json`.
5. Repo `DioxusLabs/dioxus-components` analysieren: Aufbau von Root-`component.json`,
   Komponenten-Manifest, Ablage von `.rs`/`.css`, Theme-Variablen in `dx-components.css`,
   Playwright-Setup. Konventionen übernehmen.
6. Prüfen, wie eine **eigene** Registry referenziert wird (Config-Keys in `Dioxus.toml`,
   ursprünglich `component.registry` / `component.component_dir`; CLI-Flags via
   `dx components --help`), inkl. lokalem Pfad oder Git-URL für den Smoke-Test.
7. crates.io-Namen prüfen.

**Akzeptanz:** `VERIFICATION.md` beantwortet jeden Punkt mit Quelle/Befehl. Workspace kompiliert
leer. CI-Grundgerüst (fmt, clippy, test) grün.

### Phase 1: `datagrid-core`

Implementieren: `SortValue` + `Ord`, Multi-Sort (stabil), Spaltenfilter (contains,
case-insensitiv), globale Suche über alle filterbaren Spalten, Paging, `compute_view`,
`Selection` (Single/Multi/Range), `visible_range`, `navigate`.

**Akzeptanz:**
- Unit-Tests für jede öffentliche Funktion; `proptest` für Invarianten: Sortierung ist stabil
  und eine Permutation, Filter liefert Teilmenge, Paging deckt `filtered_len` lückenlos ab,
  `visible_range` bleibt in Grenzen.
- `compute_view` über 100.000 Zeilen mit 2 Sortspalten < 50 ms im Release-Build (Criterion-Bench).
- `#![warn(missing_docs)]` ohne Warnungen, keine `unwrap`/`expect` in Bibliothekscode.

### Phase 2: `dioxus-datagrid` (Hooks + Primitives)

Implementieren: `Column<T>`-Builder, `use_grid`, alle Primitives außer Virtualisierung und
Resize, vollständige a11y laut Abschnitt 5, Beispiel `examples/basic`.

**Akzeptanz:**
- SSR-Tests mit `dioxus-ssr`: gerenderte ARIA-Attribute korrekt (sortiert/unsortiert,
  selektiert, rowindex bei Seite 2).
- `examples/basic` läuft mit `dx serve` auf Web und Desktop.
- Kein `web-sys` im Dependency-Tree von `dioxus-datagrid` (CI-Check via `cargo tree`).

### Phase 3: Registry + Playground → **Release 0.1.0**

Implementieren: `registry/`-Struktur gemäß Phase-0-Erkenntnissen, `data_grid` mit Styling,
Playground-App, Playwright-Tests, Registry-Smoke-Test.

**Akzeptanz:**
- In `tests/fixtures/consumer-app` funktioniert `dx components add data_grid` gegen die lokale
  bzw. Git-Registry, anschließend `cargo check` grün (Skript `scripts/registry-smoke.sh`, in CI).
- Playwright: Sortieren per Klick und Tastatur, Filtern, Suchen, Paging, Selection
  (Single, Multi, Shift-Range), Tastaturnavigation.
- README mit Installation (Registry eintragen, Komponente hinzufügen), Headless-Nutzung,
  Screenshot. `CHANGELOG.md` angelegt.
- Crates published (erst nach expliziter Freigabe durch Marius, siehe Abschnitt 8).

### Phase 4: Virtualisierung → 0.2.0

Implementieren: `VirtualGridBody` mit fester Zeilenhöhe, Overscan, Spacer oben/unten,
korrekte `aria-rowindex`, Fokus bleibt beim Scrollen erhalten; `examples/virtualized` mit 100.000 Zeilen.

**Akzeptanz:** flüssiges Scrollen (manuell geprüft auf Web, Desktop, Android- oder iOS-Emulator),
DOM enthält nie mehr als sichtbare Zeilen + 2 × Overscan, Playwright-Test für
Scroll + Tastaturnavigation über Viewport-Grenzen.

### Phase 5: Spalten-Features → 0.3.0 (Teil 1)

Spaltenbreite per Drag (Pointer-Events, Touch-tauglich), Spalten ein-/ausblenden,
`GridState`-Persistenz mit Feature `serde` (Beispiel: localStorage auf Web über Nutzer-Code,
nicht in der Library).

**Akzeptanz:** Resize respektiert `min_width`, State-Roundtrip-Test (serialize → deserialize → identische View).

### Phase 6: Serverseitige Daten → 0.3.0 (Teil 2)

`use_grid_remote(source: impl DataSource<T>, columns, options)`: State-Änderungen erzeugen
`GridQuery`, Laden über Dioxus-Resources, Debounce für Suche/Filter (300 ms),
Lade- und Fehlerzustand in Primitives, veraltete Antworten verwerfen.

**Akzeptanz:** `examples/server` mit simulierter Latenz; Test, dass eine langsamere ältere
Antwort eine neuere nicht überschreibt.

---

## 7. CI (GitHub Actions)

- `cargo fmt --check`, `cargo clippy --all-targets --all-features -D warnings`
- `cargo test --workspace --all-features`
- `cargo check -p dioxus-datagrid --target wasm32-unknown-unknown`
- Build-Check Desktop auf Linux, macOS, Windows
- `cargo doc --no-deps` ohne Warnungen
- Playwright gegen Web-Build des Playgrounds (ab Phase 3)
- `scripts/registry-smoke.sh` (ab Phase 3)
- `web-sys`-Guard: `cargo tree -p dioxus-datagrid -e normal | grep web-sys` muss leer sein

---

## 8. Arbeitsregeln für Claude Code (in `CLAUDE.md` übernehmen)

- **Keine APIs erfinden.** Dioxus-0.7-APIs auf docs.rs oder im Dioxus-Quellcode prüfen, bevor
  sie verwendet werden. Bei Unklarheit kleinen Spike im Playground bauen.
- Kleine, thematisch saubere Commits (Conventional Commits). Nach jedem Schritt Tests laufen lassen.
- Edition 2024, keine `unwrap`/`expect` in Bibliothekscode, öffentliche Items dokumentiert.
- `datagrid-core` bleibt frei von Dioxus; `dioxus-datagrid` bleibt frei von `web-sys`.
- Registry-Komponente bleibt dünn: neue Logik gehört in die Crates, nicht in `component.rs`.
- Architekturentscheidungen kurz in `docs/DECISIONS.md` festhalten (Kontext, Entscheidung, Alternativen).
- **Stoppen und nachfragen** vor: Veröffentlichen auf crates.io, Anlegen/Pushen von Git-Tags
  oder Releases, Änderungen an Lizenz oder Crate-Namen, Hinzufügen schwergewichtiger Dependencies.
- Am Ende jeder Phase: kurze Zusammenfassung (umgesetzt, Abweichungen, offene Punkte).

---

## 9. Offene Entscheidungen (Marius)

- Endgültiger Projekt- und Crate-Name
- GitHub-Owner (persönlich oder Organisation)
- Lizenz: Vorschlag MIT OR Apache-2.0 (Rust-Konvention, identisch zu dioxus-components)
- Mindestumfang Mobile-Test: Android-Emulator, iOS-Simulator oder beides
