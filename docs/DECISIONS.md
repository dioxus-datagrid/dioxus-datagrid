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

---

## ADR-0006 — Filter gelten auch für versteckte Spalten, die Suche nicht

**Kontext.** Sichtbarkeit hat zwei Quellen: `ColumnSpec::visible` (Voreinstellung der Spalte) und
`GridState::hidden_columns` (Laufzeit). Unklar war, ob eine versteckte Spalte noch filtert und
durchsucht wird.

**Entscheidung.** Getrennt beantwortet, weil die beiden Fälle unterschiedlich entstehen:

- **Spaltenfilter gelten weiter.** Der Nutzer hat sie bewusst gesetzt. Sie beim Ausblenden still
  fallenzulassen würde das Ergebnis unbemerkt verbreitern.
- **Die globale Suche überspringt versteckte Spalten.** Ein Treffer, den der Nutzer nirgends
  sehen kann, ist schlimmer als kein Treffer.

**Sonderfall.** Ist überhaupt keine Spalte durchsuchbar, wird der Suchbegriff ignoriert statt alle
Zeilen auszublenden — ein leeres Grid sähe kaputt aus.

**Alternativen.** *Beides auf Sichtbarkeit stützen* — verworfen, verliert bewusst gesetzte Filter.
*Beides ignorieren* — verworfen, macht die Suche unerklärlich.

---

## ADR-0007 — Sortier-Benchmark: 111 ms statt der geforderten 50 ms

**Kontext.** `PLAN.md` Phase 1 fordert: `compute_view` über 100.000 Zeilen mit 2 Sortspalten unter
50 ms im Release-Build. Die erste lauffähige Fassung brauchte **1089 ms**.

**Was optimiert wurde.** Vier Runden, jede gemessen:

| Schritt | 2 Textspalten, 100k |
|---|---|
| Ausgangsstand | 1089 ms |
| Byte-weises Case-Folding für ASCII statt `char::to_lowercase` | 160 ms |
| `memcmp`-Schnellpfad für gleiche Strings + `sort_unstable` mit Index-Tiebreak | 137 ms |
| Kompakte Komparator-Daten statt Neudurchlauf des Sortierplans | 132 ms |
| Keys und Index gepackt sortieren statt indirekt über eine Permutation | **111 ms** |

Zusammen **9,8× schneller**. Das Kriterium ist damit trotzdem verfehlt.

**Wo die verbleibende Zeit liegt.** Der Bench misst zusätzlich Varianten, die das trennen:

| Fall | 100k Zeilen |
|---|---|
| 2 numerische Spalten | **32 ms** ✅ |
| 1 numerische + 1 Textspalte | 71 ms |
| 1 Textspalte | 87 ms |
| 2 Textspalten (Kriteriumsfall) | 111 ms |
| kein Sort, kein Filter | 0,04 ms |

Der rein numerische Fall hält das Kriterium. Er zeigt aber auch die Untergrenze der Architektur:
**32 ms ohne jede String-Arbeit** sind bereits zwei Drittel des Budgets. Ursache ist die Größe der
Sortierschlüssel — `SortValue` ist wegen der `Text(String)`-Variante 24 Byte groß, also bei 100k
Zeilen × 2 Spalten ein Puffer von 4,8 MB, weit jenseits des L2-Cache.

Der Aufschlag für Text (111 − 32 = 79 ms) besteht aus rund 200.000 `String`-Allokationen
(≈ 18 ms, gemessen über den Filter-Bench) und dem Vergleichen von 100.000 einzeln allokierten
Strings, also einem Cache-Miss pro Vergleichsseite bei über einer Million Vergleichen.

**Entscheidung.** Vorerst bei 111 ms bleiben und das offenlegen, statt das Kriterium still zu
verfehlen oder den Benchmark auf den numerischen Fall zurechtzuschneiden.

**Was die 50 ms erreichen könnte** — beides mit Konsequenzen, die über Phase 1 hinausreichen und
deshalb Marius gehören:

1. **Geliehene Sortierschlüssel:** `SortValue<'a>` mit `Text(&'a str)` statt `Text(String)`. Spart
   die 200.000 Allokationen (≈ 18 ms) und schrumpft den Schlüssel auf 16 Byte. Kostet die
   Möglichkeit, berechnete Schlüssel zurückzugeben (`format!("{last}, {first}")`), und ändert die
   in `PLAN.md` Abschnitt 4.1 festgelegte Signatur sowie den `Column<T>`-Builder aus Phase 2.
2. **Text-Arena:** die Schlüsseltexte intern in einen zusammenhängenden Puffer kopieren und im
   Schlüssel nur `(start, len)` halten. Rein intern, keine API-Änderung, behebt aber nur die
   Cache-Misses, nicht die Allokationen. Geschätzt ~70–80 ms.

Beide zusammen lägen plausibel bei 50–60 ms. Keine Variante ist sicher unter 50 ms.

**Einzuordnen.** 111 ms betreffen den vollständigen Neuaufbau über 100.000 Zeilen. Er läuft, wenn
sich Sortierung, Filter oder Daten ändern — nicht beim Scrollen und nicht beim Paginieren.
ADR-0001 sorgt dafür, dass Scrollen die Virtualisierungs-Mathematik benutzt, die Zeit dafür liegt
im Mikrosekundenbereich.
