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

> **Nachtrag (Phase 5).** In der Praxis nicht haltbar: die letzte Spalte ließ sich so nicht
> verbreitern. Ergänzt um ein Overlay während des Ziehens, siehe ADR-0018.

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

---

## ADR-0008 — Geliehene Sortierschlüssel (`SortValue<'a>`)

**Kontext.** Umsetzung von Option 1 aus ADR-0007, entschieden von Marius, bevor Phase 2 den
`Column<T>`-Builder darauf aufbaut. `SortValue::Text(String)` wird zu `SortValue::Text(&'a str)`,
`SortKeyFn<T>` wird higher-ranked über die Zeilen-Lebensdauer.

**API.** Ein einzelnes generisches `sort_by` kann nicht gleichzeitig geliehene und besitzende
Rückgaben annehmen — der Rückgabetyp müsste von `'a` abhängen, was ohne GATs nicht ausdrückbar ist.
Statt einer Trait-Akrobatik gibt es drei benannte Methoden, alle im Spike gegen den Compiler
geprüft:

```rust
ColumnSpec::new("name").sort_by_text(|u: &User| u.name.as_str())   // geliehener Text
ColumnSpec::new("age").sort_by_value(|u: &User| u.age)             // Zahlen, Bools, Options davon
ColumnSpec::new("x").sort_by(|u: &User| u.nick.as_deref().into())  // allgemein, SortValue<'a>
```

**Was es gebracht hat.** 111 ms → **91 ms** (−18 %) im Kriteriumsfall. Deutlich weniger als die in
ADR-0007 geschätzten ~18 ms Allokationsersparnis vermuten ließen.

**Warum weniger als erhofft.** `SortValue` bleibt **24 Byte** statt der erhofften 16: bei fünf
Varianten reicht der Zeiger-Niche von `&str` nicht, um den Diskriminanten unterzubringen. Und ein
`&'a str` zeigt weiterhin in den einzeln allokierten `String` jeder Zeile — die Streuung im Speicher
bleibt also, gespart wird nur das Allozieren.

**Was wir zusätzlich probiert und wieder verworfen haben.** Option 2 aus ADR-0007, die Text-Arena:
Schlüsseltext in einen zusammenhängenden Puffer kopieren, im Schlüssel nur `(start, len)` halten
(16 statt 24 Byte). Gemessen **23 % langsamer** (91 → 128 ms).

Entscheidend war der Nebenbefund: auch der **rein numerische** Fall wurde um 27 % langsamer
(30 → 42 ms), obwohl dort nie Text angefasst wird. Die Sortierung ist also **nicht** durch
Speicherlokalität begrenzt, sondern durch die Arbeit pro Vergleich — womit die Grundannahme hinter
der Arena widerlegt ist. Sie wurde zurückgenommen; der Kommentar an `sort_packed` hält das fest,
damit es niemand erneut probiert.

**Stand.** Kriteriumsfall (2 Textspalten, 100k): **91 ms** gegen ein Ziel von 50 ms.

| Fall | vorher | jetzt |
|---|---|---|
| 2 numerische Spalten | 32 ms | **30 ms** ✅ |
| 1 numerisch + 1 Text | 71 ms | 67 ms |
| 1 Textspalte | 87 ms | 82 ms |
| 2 Textspalten (Kriterium) | 111 ms | **91 ms** ❌ |

Insgesamt seit dem Ausgangsstand **12× schneller** (1089 → 91 ms).

**Offen.** Die 50 ms sind mit beiden vorgeschlagenen Hebeln nicht erreicht, und der zweite war
kontraproduktiv. Die verbleibende Grenze liegt bei ~30 ms für einen rein numerischen Sort — also
in der Vergleichs- und Sortierarbeit selbst, nicht im Umgang mit Text. Ein weiterer Anlauf müsste
dort ansetzen (etwa spezialisierte Komparatoren pro Spaltentyp statt eines Enum-Matches pro
Vergleich) und gehört gemessen, nicht geraten.

---

## ADR-0009 — Registry-Root-Manifest für `--git`

**Kontext.** Die Phase-0-Verifikation hatte nur `dx components add --path ./registry` geprüft.
`--git` klont dagegen das ganze Repo und liest `component.json` im Repo-Root
(`ComponentRegistry::resolve` → `read_component(repo_dir)`). Unser Manifest lag nur unter
`registry/`, der dokumentierte Installationsweg schlug also fehl — gemessen, nicht vermutet.

**Entscheidung.** Ein zusätzliches `component.json` im Repo-Root mit `members: ["registry"]`.
`discover_components` löst Members rekursiv auf und filtert virtuelle Komponenten heraus, sodass
`registry/component.json` und die Struktur aus `PLAN.md` Abschnitt 2 unverändert bleiben.
Der Smoke-Test läuft seitdem mit `--path <repo root>`, also exakt dem Einstiegspunkt von `--git`.

**Alternativen.** *Komponenten ins Repo-Root verschieben* — verworfen, weicht ohne Not von
Abschnitt 2 und ADR-0003 ab. *`registry/component.json` löschen und nur das Root-Manifest
führen* — verworfen, weil `--path ./registry` für lokale Entwicklung bequem bleibt.

---

## ADR-0010 — Theme mitliefern, beide Stylesheets aus der Komponente laden

**Kontext.** Offen aus ADR-0004. Die Komponente stylt ausschließlich über dx-components-Variablen;
ohne `dx-components-theme.css` wäre sie farblos. Nicht jedes Projekt hat bereits eine offizielle
Komponente installiert.

**Entscheidung.** `registry/assets/dx-components-theme.css` ist eine unveränderte Kopie des
offiziellen Themes (MIT OR Apache-2.0, mit Herkunftsvermerk), deklariert als `globalAsset` wie bei
den offiziellen Komponenten. `component.rs` verlinkt das Theme und das eigene `style.css` selbst.
Hat ein Projekt das Theme schon, überschreibt `dx components add` die Datei mit identischem Inhalt,
und der doppelte `<link>` löst auf dieselbe gehashte Asset-URL auf.

**Alternative.** *Nur eigenes CSS mit `var(--x, fallback)`* — verworfen, weil die Fallbacks
hartcodierte Farbwerte wären, die `CLAUDE.md` ausschließt.

**Bekannte Grenze.** `asset!("/src/components/data_grid/style.css")` setzt das Standard-
`components_dir` voraus. Die offiziellen Komponenten haben dieselbe Annahme
(`#[css_module("/src/components/…")]`); `docs.md` nennt sie.

---

## ADR-0011 — Der Playground trägt eine Kopie der Komponente

**Kontext.** Playwright soll die Komponente testen, die Nutzer bekommen. Wir trennen
Registry-Quelle und Playground (ADR-0003), und `dx components add` im Playground würde dessen
`Cargo.toml` auf eine `git`-Abhängigkeit an unser eigenes Repo umschreiben statt den Workspace-Pfad
zu nutzen.

**Entscheidung.** `scripts/sync-playground-component.sh` kopiert `component.rs`, `mod.rs`,
`style.css` und das Theme nach `playground/`, mit „@generated"-Kopfzeile. Die Kopie ist committet,
damit der Workspace aus einem frischen Clone baut. Der CI-Job `playground-in-sync` führt das Skript
aus und schlägt bei einem Diff fehl. Den echten `dx components add`-Weg deckt weiterhin der
Smoke-Test ab.

**Alternativen.** *`#[path]`-Include aus `registry/`* — verworfen, weil `asset!` relativ zum
einbindenden Crate auflöst und das Stylesheet dann nicht fände. *Generiert und gitignored* —
verworfen, weil `cargo check --workspace` dann ohne vorherigen Skriptlauf bricht.

---

## ADR-0012 — Grid-Optionen reagieren auf Änderungen

**Kontext.** Playwright hat gezeigt, dass ein Wechsel von `selection` oder `page_size` am
`DataGrid` wirkungslos blieb. `use_grid` las beide nur beim ersten Rendern, und der Modus lag als
Plain-Wert im `Copy`-Handle — Zellen, die vor dem Wechsel gerendert wurden, behielten Klick-Handler
mit dem alten Modus. Ein `key` am `DataGrid` im Playground hat keinen Remount ausgelöst.

**Entscheidung.** Der Modus ist ein `Signal` im Handle und Teil von `PartialEq`. `use_grid`
vergleicht Modus und Seitengröße bei jedem Rendern mit dem zuletzt gesehenen Wert und schreibt nur
bei echter Änderung — dasselbe Muster, das `use_reactive` intern verwendet
(`dioxus-hooks-0.7.10/src/use_reactive.rs`). Ein Moduswechsel leert die Auswahl, weil eine
Mehrfachauswahl im Single- oder None-Modus ungültig wäre. `GridState::set_page_size` setzt bei
geänderter Größe auf Seite 1 zurück.

**Alternative.** *Remount per `key` beim Aufrufer* — verworfen: verlagert einen Fehler der
Bibliothek auf jeden Nutzer und hat im Test ohnehin nicht funktioniert.

---

## ADR-0013 — Spaltenfilter außerhalb von `role="grid"`

**Kontext.** Der Plan fordert Filtern in Playwright; die Komponente hatte nur die globale Suche.
Die naheliegende Filterzeile unter den Spaltenköpfen läge innerhalb des Grids.

**Entscheidung.** `column_filters: true` rendert die Eingaben über dem Grid, nicht darin. Eine
Filterzeile im Grid würde bei `aria-rowcount` mitzählen und jedes `aria-rowindex` verschieben, und
Eingabefelder in `gridcell`s kollidieren mit der Pfeiltastennavigation, die die Pfeiltasten für
sich beansprucht. Jede Eingabe trägt ein `aria-label` („Filter Name").

---

## ADR-0014 — Kontrast und Accessible Name aus axe und Playwright

**Kontext.** `@axe-core/playwright` meldete genau eine Regel: `color-contrast`. Gedämpfter Text in
`--secondary-color-5` erreicht auf dem hellen Hintergrund 3,61:1 statt der geforderten 4,5:1.
Unabhängig davon fanden die Playwright-Tests die Header nach dem Sortieren nicht mehr über ihren
Namen: der Sortierpfeil aus `::after` wurde Teil des Accessible Name („Name▲").

**Entscheidung.** Gedämpfter Text nutzt `--secondary-color-3`. Der Pfeil verwendet
`content: "▲" / ""`, dessen leerer Alternativtext ihn aus dem Accessible Name nimmt;
`aria-sort` trägt die Information. Ausgewählte Zeilen bekommen zusätzlich einen Akzentbalken in
`--focused-border-color`, weil sich der Hintergrund allein im hellen Theme kaum abhebt — sichtbar
geworden erst im README-Screenshot.

---

## ADR-0015 — Virtualisierung: `GridRoot` als Scroll-Container, Bereich abgeleitet statt gespeichert

**Kontext.** Phase 4 fordert `VirtualGridBody` mit fester Zeilenhöhe, Overscan, korrektem
`aria-rowindex` und erhaltenem Fokus beim Scrollen — plattformneutral, also über `onscroll`,
`onresize` und `MountedData` (ADR-0001), ohne `web-sys` und ohne `document::eval`.

**Entscheidung.**

- **`GridRoot` ist der Scroll-Container** und meldet `scroll_top`, `scroll_left` und die
  Viewport-Höhe an den Handle; `GridHeader` meldet seine Höhe. Die Registry-Komponente scrollt
  deshalb nicht mehr über einen Wrapper (`.dg-scroll`), sondern über `.dg` selbst — die
  Scroll-Events eines Wrappers sähe das Grid nie.
- **Der gerenderte Bereich wird bei jeder Abfrage aus Scroll-Position, Zeilenhöhe und Overscan
  berechnet**, nicht nach dem Rendern gespeichert. Die erste Fassung schrieb ihn in einem Effect
  zurück. Das hinkte einen Render hinterher, und in dieser Lücke galt eine eben per `Ctrl+End`
  angesprungene Zeile als „nicht gerendert" — worauf das Root den Fokus parkte und der Zelle
  wieder wegnahm. Playwright fand das: kleine Schritte gingen, große Sprünge nicht.
- **Die Scroll-Mathematik liegt im Core** (`reveal_scroll_top`, `rows_per_viewport`), mit
  Property-Test, dass eine so gescrollte Zeile immer ganz sichtbar und gerendert ist.
- **Scrollen ist `Instant`, nicht `Smooth`** (Dioxus-Default). Bei gehaltener Pfeiltaste liefe ein
  weiches Scrollen dem Renderbereich hinterher.
- **`scroll_left` wird mitgeführt**, weil `MountedData::scroll` beide Achsen zugleich setzt und
  horizontales Scrollen sonst bei jeder Tastennavigation zurückspränge.
- **Fokus-Stellvertretung am Root**, beschrieben in `docs/ACCESSIBILITY.md`. Ein
  `focus_pending`-Flag ersetzt die reine Nonce-Logik: eine Zelle übernimmt den Fokus auch dann,
  wenn sie erst nach der Bewegung eingehängt wird, eine beim normalen Scrollen neu eingehängte Zelle
  dagegen nicht.

**Annahmen, dokumentiert an `VirtualGridBody`.** Alle Zeilen exakt `row_height` hoch; der Header
ist sticky oder fehlt. Variable Zeilenhöhen bleiben Nicht-Ziel (`PLAN.md` Abschnitt 1).

**Alternative.** *Die fokussierte Zeile immer zusätzlich rendern, auch außerhalb des Fensters.*
Verworfen: bräche das Akzeptanzkriterium „nie mehr als sichtbare Zeilen + 2 × Overscan" und
bräuchte absolute Positionierung außerhalb des Subgrid-Layouts.

---

## ADR-0016 — Layout-abhängiges Verhalten nur mit echtem Rendering prüfen

**Kontext.** Beim Debuggen lieferten `onscroll` und `onresize` im In-App-Browser keinerlei Events
— auch an einem Minimal-Spike ohne jede Bibliothekslogik. Der Browser-Bereich war ausgeblendet, und
ein nicht gezeichnetes Fenster führt keinen Rendering-Schritt aus; genau dort feuern Scroll-Events
und ResizeObserver-Callbacks. Derselbe Spike in Playwright: 1 Scroll, 2 Resizes, wie erwartet.

**Entscheidung.** Alles, was von Layout, Scrollen, Größenänderung oder Fokus abhängt, wird mit
Playwright gegen echtes Rendering geprüft, nicht im In-App-Browser. Nebenbefund für
`docs/VERIFICATION.md`: dort war `onscroll` in Phase 0 per `dispatchEvent` ausgelöst worden — belegt
waren also die Werte, nicht dass das Event bei echtem Scrollen feuert. Das ist erst jetzt belegt.

---

## ADR-0017 — Overscan 20 in der Registry-Komponente, 5 im Primitive

**Kontext.** Der manuelle Test auf dem Android-Emulator (Pixel 10 Pro, `dx serve --android
--release`) zeigte beim schnellen Wischen leere Zeilen, bevor der nächste Render ankam. Auf Mobile
läuft jedes Scroll-Event über die Brücke zwischen WebView und Rust und die Änderungen zurück; ein
Fling scrollt schneller, als diese Runde dauert. Mit dem Standard-Overscan von 5 Zeilen (200 px bei
40 px Zeilenhöhe) ist der Puffer schnell aufgebraucht.

Gemessen über eine Overscan-Auswahl im Playground, nach Augenmaß:

| Overscan | Ergebnis |
|---|---|
| 5 | deutlich leere Zeilen beim Wischen |
| 20 | nur kurz leere Zeilen |
| 40 | Scrollen spürbar zäh |

**Entscheidung.** Die Registry-Komponente bekommt einen Prop `overscan` mit Standard **20**. Das
Primitive `VirtualGridBody` behält seinen Standard 5.

- Die Komponente ist das, was die meisten nutzen, und sie läuft auch auf Mobile — ihr Standard soll
  dort funktionieren. 20 Zeilen oben und unten sind auf Web und Desktop unkritisch: bei 480 px Höhe
  rund 50 statt 22 gerenderte Zeilen.
- Den Primitive-Standard zu ändern hieße, das Verhalten eines veröffentlichten Crates zu ändern,
  ohne dass es dort einen Fehler gibt. Wer Primitives direkt verwendet, setzt `overscan` selbst.
- Das Akzeptanzkriterium „nie mehr als sichtbare Zeilen + 2 × Overscan" gilt unverändert, nur mit 20.

**Grenze.** Ganz verschwinden die leeren Zeilen auch mit 20 nicht. Ein größerer Overscan verschiebt
nur, wie schnell man wischen muss; ab 40 kostet das Rendern selbst mehr, als der Puffer bringt.
Weiter käme man nur, wenn weniger über die Brücke geht (z. B. Zellen, die bei gleichbleibender Zeile
nicht neu diffen) — ein Thema für später, falls es jemand braucht.

---

## ADR-0018 — Spaltenbreite: Overlay beim Ziehen, Tastatur über die Kopfzelle

**Kontext.** Phase 5 setzt das Ziehen der Spaltenbreite nach ADR-0002 um: Der Griff startet, das
Grid-Root verfolgt den Zeiger. Playwright zeigte zwei Lücken:

1. **Die letzte Spalte ließ sich nicht verbreitern.** Ihr Griff sitzt am rechten Rand des Grids,
   schon die erste Bewegung nach rechts verlässt das Root, und ohne Pointer Capture kommt keine
   `pointermove` mehr an. Die Breite blieb bei 88 px.
2. **Doppelklick zum Zurücksetzen funktionierte in WebKit nicht.** WebKit löst weder `click` noch
   `dblclick` aus, wenn Drücken und Loslassen auf verschiedenen Elementen passieren — Chromium
   schon. Belegt mit einem Event-Protokoll in beiden Engines.

**Entscheidung.**

- **Solange eine Spaltenbreite gezogen wird, rendert `GridRoot` ein transparentes Overlay**
  (`position: fixed; inset: 0`) als eigenes Kind. Es fängt den Zeiger im ganzen Fenster, seine
  Events bubbeln zu den Handlern am Root. Plattformneutral, ohne `eval` und ohne `web-sys`. Das
  Overlay erscheint schon beim Drücken, nicht erst bei Bewegung: bei der letzten Spalte verlässt
  bereits die erste Bewegung das Grid.
- **Zurücksetzen ohne `dblclick`:** Zweimal auf denselben Griff drücken, ohne zu ziehen und ohne
  etwas dazwischen, stellt die definierte Breite wieder her. Funktioniert in allen Engines und als
  Doppeltipp.
- **Ein Klick, der ein Ziehen beendet, sortiert nicht.** Endet das Ziehen über einer Kopfzelle,
  kann dort ein `click` ankommen — sicher dann, wenn das Overlay auf einer langsamen Brücke wie
  Android noch nicht gerendert ist. `GridHandle` merkt sich das Ende eines echten Ziehens, und die
  Kopfzelle ignoriert genau diesen Klick.
- **Tastatur über die Kopfzelle, nicht über ein `separator`-Element.** Die erste Fassung von
  `ACCESSIBILITY.md` sah einen fokussierbaren `separator` mit `aria-valuenow` vor. Im
  Roving-Tabindex des Gitters hat ein zusätzliches fokussierbares Element keine Koordinate; es
  wäre entweder ein zweiter Tab-Stopp oder per Pfeiltaste unerreichbar. Stattdessen verändert
  `Alt+←`/`Alt+→` auf der fokussierten Kopfzelle die Breite, angekündigt per
  `aria-keyshortcuts`; der Griff ist `aria-hidden`. `Alt+←` ist in Browsern „Zurück", das
  `preventDefault` im `keydown` verhindert es; im Playwright-Test bleibt die Seite stehen.
- **Messung statt Annahme:** Jede Kopfzelle meldet ihre gerenderte Breite per `onresize`. Ein
  Ziehen startet von dort, weil die Breite einer `Auto`- oder `Fraction`-Spalte nur das Layout
  kennt. Jede Bewegung rechnet vom Startpunkt aus, nicht schrittweise, damit verlorene Events sich
  nicht aufsummieren.
- **Die letzte sichtbare Spalte lässt sich nicht ausblenden**, und die Fokus-Koordinate wird beim
  Lesen auf vorhandene Zellen begrenzt. Ohne beides hätte das Gitter ohne Spalten oder nach dem
  Verschwinden der fokussierten Spalte keinen Tab-Stopp mehr.

**Grenze.** Außerhalb des Browserfensters gibt es keine Events. Wird dort losgelassen, bemerkt das
Grid es bei der nächsten Bewegung im Fenster ohne gedrückte Taste und beendet das Ziehen.

**Alternativen.**
- *`document::eval` mit `setPointerCapture`.* Verworfen aus demselben Grund wie in ADR-0002.
- *Overlay erst bei der ersten Bewegung.* Ausprobiert; die letzte Spalte bekam die erste Bewegung
  nie zu sehen.

**Persistenz.** `GridState` ist die Einheit, die eine App speichert; die Bibliothek speichert
nichts selbst (`PLAN.md` Phase 5). Das Beispiel steht im Playground: `localStorage` über
`document::eval` im Anwendungscode. Eine gespeicherte Breite wird beim Anwenden auf die
Mindestbreite begrenzt, und `set_column_width` ignoriert nicht-endliche Werte, die JSON nicht
darstellen kann — sonst würde der Zustand den Round-Trip nicht überstehen.

---

## ADR-0019 — Release 0.3.0 nach Phase 5, Phase 6 wird 0.4.0

**Kontext.** `PLAN.md` sieht 0.3.0 erst nach Phase 6 vor, mit Phase 5 als „Teil 1". Die
Registry-Komponente auf dem Default-Branch hängt aber an der veröffentlichten Crate-Version
(`component.json` → `dioxus-datagrid = "0.3"`), und `dx components add --git` installiert genau
diesen Stand. Nutzt die Komponente Crate-APIs, die noch nicht veröffentlicht sind, kompiliert die
Installation nicht — das ist bei 0.2.0 schon einmal aufgefallen, und der CI-Job gegen crates.io
belegt es für Phase 5 (`is_column_visible`, `column_width`, `GridHeader::resizable` fehlen in 0.2.0).

**Entscheidung.** Phase 5 wird als 0.3.0 veröffentlicht, bevor sie gepusht wird. Phase 6
(serverseitige Daten) wird 0.4.0. Mit dem Nutzer abgestimmt.

**Folge für künftige Phasen.** Eine Phase, die die Registry-Komponente an neue Crate-APIs bindet,
endet mit einem Release. Reihenfolge wie bei 0.2.0: Versionen anheben, veröffentlichen, mit
`SMOKE_PUBLISHED=1` gegen crates.io prüfen, dann pushen und taggen.

**Alternativen.**
- *Commits bis nach Phase 6 lokal halten.* Verworfen: eine ganze Phase ungesichert und ohne CI.
- *Ohne Release pushen.* Verworfen: die dokumentierte Installation wäre bis dahin kaputt.

---

## ADR-0020 — Serverseitige Daten: derselbe Handle, Abbruch plus Tracker, Timer per `cfg`

**Kontext.** Phase 6 verlangt `use_grid_remote` mit Debounce für Suche und Filter, Lade- und
Fehlerzustand in den Primitives und das Verwerfen veralteter Antworten.

**Entscheidungen.**

- **`use_grid_remote` liefert denselben `GridHandle`** wie `use_grid`. Beide Hooks teilen eine
  Basis, die alle Signals außer der View anlegt; nur die View unterscheidet sich. Lokal rechnet
  `compute_view` sie aus, remote ist sie die empfangene Seite mit der Server-Gesamtzahl als
  `filtered_len`. Dadurch funktionieren alle Primitives, Tastaturnavigation, Selektion,
  `aria-rowindex` und Spaltenbreiten unverändert.
- **Veraltete Antworten, doppelt abgesichert.** Eine neue Anfrage bricht den laufenden Task ab
  (`Task::cancel`), was den Future verwirft und bei einer passenden Datenquelle auch die
  Netzwerkanfrage. Zusätzlich vergibt ein `RequestTracker` aus dem Core fortlaufende IDs, und nur
  die Antwort auf die jüngste wird übernommen. Das deckt Quellen ab, deren Arbeit sich nicht
  abbrechen lässt. Mutationstest: Ohne beides schlägt der Akzeptanztest fehl; jeder der zwei
  Mechanismen allein reicht, damit er besteht.
- **Debounce nur beim Tippen.** `GridQuery::is_typing_change` unterscheidet: Ändern sich nur
  Suche oder Spaltenfilter, wartet die Anfrage 300 ms (`GridOptions::debounce`). Sortieren und
  Blättern sind einzelne, bewusste Aktionen und laden sofort.
- **Timer per `cfg`, nicht per `web-sys` oder `eval`.** Dioxus 0.7 hat keinen eigenen Timer.
  `tokio::time` (nur Feature `time`) außerhalb von WASM, `gloo-timers` auf WASM — dieselbe
  Aufteilung wie `dioxus-sdk-time`. Beide stehen ohnehin im Baum des jeweiligen Renderers
  (`dioxus-web` hängt an `gloo-timers`, Desktop, Mobile und Server laufen auf Tokio); keine
  zieht `web-sys`. Der `web-sys`-Guard der CI bleibt leer. Auf WASM steht `web-sys` über Dioxus
  selbst (`subsecond`) schon im Baum, auch vor dieser Änderung.
- **Die vorherige Seite bleibt beim Laden stehen**, statt leer zu werden; `aria-busy="true"` am
  Grid kündigt die Änderung an. `GridStatus` ist eine Live-Region, die immer im DOM steht, weil
  Screenreader nur Änderungen in bereits vorhandenen Live-Regionen ansagen.
- **Die Registry-Komponente bekommt keinen Remote-Modus.** Sie würde entweder doppelt so lang oder
  müsste ihre `data`-Prop aufgeben; beides widerspricht „dünn bleiben". Remote-Grids werden aus
  den Primitives gebaut, wie `examples/server` zeigt. Nebeneffekt: Die Komponente nutzt keine
  neuen Crate-APIs, Phase 6 erzwingt also keinen Release vor dem Push (vgl. ADR-0019).
- **Nicht kombiniert mit Virtualisierung.** Remote lädt seitenweise; ein virtualisiertes
  Remote-Grid bräuchte blockweises Nachladen beim Scrollen. Nicht Teil von `PLAN.md`.

**Tests.** `crates/dioxus-datagrid/tests/remote.rs` treibt eine echte `VirtualDom` auf Tokio mit
pausierter Zeit: simulierte Latenzen sind exakt, die Tests laufen in Millisekunden. Zusätzlich
prüft Playwright `examples/server` im Browser, einschließlich des abgebrochenen langsamen Requests.
