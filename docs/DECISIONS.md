# Architekturentscheidungen

ADR-light: Kontext, Entscheidung, Alternativen. Neueste zuletzt.

---

## ADR-0001 — Virtualisierung über `onscroll` statt `document::eval`

**Kontext.** `PLAN.md` Abschnitt 4.2 fordert Plattformneutralität: kein `web-sys`, kein
`document::eval` für Kernfunktionen. Das offizielle `dioxus-primitives::VirtualList` verletzt das
und hängt seine Scroll-Verfolgung an ein `document::eval`-Skript. Das war Anlass zu prüfen, ob
`onscroll` überhaupt trägt.

**Entscheidung.** Wir benutzen `onscroll` mit `ScrollData`. Der Phase-0-Spike liefert an einem
verschachtelten Container exakt die DOM-Werte (`scroll_top=450 client_height=128
scroll_height=6000`), und `dioxus-core-types` klassifiziert `scroll` korrekt als nicht bubbelnd,
weshalb der Listener direkt am Element hängt.

**Alternativen.**
- *`document::eval` wie das offizielle VirtualList.* Verworfen. Es löst ein Problem, das wir nicht
  haben: variable Zeilenhöhen brauchen Scroll-Korrektur und Scroll-End-Debounce. Bis 0.3 ist die
  Zeilenhöhe fest (`PLAN.md` Nicht-Ziele).
- *`web-sys` direkt.* Verworfen, bricht die Plattformneutralität und ist per CI-Guard untersagt.

**Folge.** Sollte Phase 4 variable Zeilenhöhen nachrüsten, ist diese Entscheidung neu zu bewerten.

---

## ADR-0002 — Drag-Gesten über Listener am Vorfahren statt Pointer Capture

**Kontext.** Phase 5 braucht Spaltenbreite per Drag. Ein Resize-Handle ist wenige Pixel breit.
Dioxus 0.7 exponiert **keine** `setPointerCapture`-API — `MountedData` bietet Fokus, Scroll und
Rect, mehr nicht. Der Spike hat bestätigt, dass `onpointermove` am Handle-Rand aufhört
(`moves=0`, `left_handle=true`), während ein Drag innerhalb des Handles sauber trackt
(`delta_x=181`).

**Entscheidung.** `onpointerdown` startet den Drag am Handle; `onpointermove` und `onpointerup`
hängen am Grid-Root. Im Spike gegengetestet: Drag-Start auf einem 24 px breiten Handle, getrackt
über 605 px (`delta_x=605`, exakt die Drag-Distanz).

**Alternativen.**
- *Window-weites `document::eval`-Pointer-Tracking*, wie `dioxus-primitives::pointer` es macht.
  Verworfen: bricht die Plattformneutralität für eine Funktion, die auch ohne geht.
- *Listener am Handle belassen.* Verworfen, unbrauchbar — siehe Messung.

**Grenze, die wir bewusst akzeptieren.** Verlässt der Zeiger das Grid-Root vollständig, endet die
Verfolgung. Das ist deutlich besser als am Handle-Rand und nach Phase-5-Praxis zu bewerten.

---

## ADR-0003 — Registry-Komponenten unter `registry/`, nicht in der Playground-App

**Kontext.** `DioxusLabs/dioxus-components` legt seine Komponenten in der Preview-App ab
(`preview/src/components/<name>`) und verweist aus dem Root-`component.json` dorthin. Damit sind
Registry-Quelle und Demo-App dasselbe Verzeichnis.

**Entscheidung.** Wir folgen `PLAN.md` Abschnitt 2 und trennen: Registry-Quelle in `registry/`,
Demo in `playground/`. Alles andere (Ordneraufbau, `mod.rs`-Muster, `exclude`-Liste,
`globalAssets`, Theme-Variablen, Klassen-Präfix) übernimmt die Konventionen von dioxus-components.

**Alternativen.**
- *Layout von dioxus-components exakt spiegeln.* Verworfen: es verschränkt die versionierte
  Registry-Quelle mit einer App, die sich frei ändern darf. Die Trennung macht außerdem
  offensichtlich, was Nutzer tatsächlich kopiert bekommen.

**Folge für einen späteren Upstream-Beitrag.** `component.json` und Ordneraufbau sind identisch;
ein Umzug nach `preview/src/components/data_grid` wäre ein Verschieben plus Pfadanpassung im
Root-Manifest, keine inhaltliche Änderung.

---

## ADR-0004 — Theme-Datei heißt `dx-components-theme.css`

**Kontext.** `PLAN.md` Abschnitt 4.3 nennt `dx-components.css` als Quelle der Theme-Variablen.
Die Datei heißt im Repo tatsächlich `preview/assets/dx-components-theme.css`.

**Entscheidung.** Wir richten uns nach dem Repo. Gestylt wird ausschließlich über dessen Variablen
(`--primary-color-*`, `--secondary-color-*`, `--focused-border-color`, …), keine hartcodierten
Farbwerte.

**Offen für Phase 3.** Ob unsere Komponente das offizielle Theme als `globalAsset` mitliefert.
Dagegen spricht: `globalAssets` werden nach Dateiname kopiert, zwei Komponenten mit gleichnamigem
Asset überschreiben sich. Wer schon eine offizielle Komponente installiert hat, hat die Datei
bereits. Wahrscheinlich liefern wir nur unser eigenes `dx-datagrid-theme.css` und dokumentieren das
offizielle Theme als Voraussetzung.

---

## ADR-0005 — Edition 2024, MSRV 1.85

**Kontext.** `PLAN.md` Abschnitt 8 gibt Edition 2024 vor. `dioxus` 0.7.10 selbst ist Edition 2021
mit MSRV 1.83.

**Entscheidung.** Workspace auf Edition 2024, `rust-version = "1.85"` (das Minimum für Edition
2024). Das liegt über der MSRV von Dioxus, schließt also niemanden aus, der Dioxus 0.7 ohnehin
benutzen kann.

**Alternative.** *Edition 2021, MSRV 1.83* für maximale Reichweite. Verworfen, weil der Plan
Edition 2024 vorgibt und der Gewinn an Reichweite gering ist.
