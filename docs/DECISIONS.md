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

**Nachtrag.** Die MSRV steht seit ADR-0032 auf **1.88**; die Entscheidung für Edition 2024 bleibt.

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

---

## ADR-0021 — Fullstack-Beispiel mit SQLite

**Kontext.** `examples/server` simuliert den Server im Prozess. Offen blieb, wie ein echter Server
eine `GridQuery` beantwortet — und was dabei sicherheitsrelevant ist.

**Entscheidung.** `examples/fullstack`: eine Dioxus-Server-Funktion (`#[post("/api/employees")]`)
fragt eine SQLite-Datenbank im Speicher ab.

- **`rusqlite` mit `bundled`, nur im Beispiel und nur hinter dem `server`-Feature.** Die Crates
  bekommen keine Datenbank-Abhängigkeit, und der WASM-Client-Build sieht `rusqlite` nie. Mit dem
  Nutzer abgestimmt. `bundled` kompiliert SQLite mit, damit das Beispiel ohne installierte
  Bibliothek auf allen drei CI-Betriebssystemen baut.
- **Eine feste Spaltenliste ist die Sicherheitsgrenze.** Spalten-IDs kommen vom Client und werden
  nur nachgeschlagen, nie ins SQL eingesetzt; unbekannte IDs werden ignoriert, wie lokal im Grid.
  Filter- und Suchtext gehen ausschließlich als gebundene Parameter, `%`, `_` und `\\` darin
  werden für `LIKE` maskiert. Getestet mit feindseligen IDs und Texten.
- **Das SQL muss exakt wie das Grid antworten.** Ein Test vergleicht Zeilen und Gesamtzahl der
  SQL-Abfrage mit `compute_view` über dieselben Daten, für Sortierung, Suche, Filter und Paging.
  `COLLATE NOCASE` entspricht der Standard-Sortierung ohne Groß-/Kleinschreibung, `id` als letzter
  Sortierschlüssel der stabilen Sortierung — und verhindert, dass Zeilen zwischen Seiten springen.
- **Ein `Mutex` um eine Verbindung** statt Pool und `spawn_blocking`: genug für ein Beispiel, im
  Code als Vereinfachung benannt.

**Folge für die CI.** Das Beispiel ist Workspace-Mitglied; `--all-features` baut es mit `server`
und vereinigt die Dioxus-Features workspace-weit. Clippy und Tests laufen damit ohne Befund.
Playwright startet das Beispiel mit `dx run` als dritten Server.

## ADR-0022 — Ein typisierter Zellwert für Sortieren, Formatieren und alles Weitere

**Kontext.** Bis 0.4.0 hatte eine Spalte eine Closure fürs Sortieren (`SortValue`) und eine fürs
Filtern (`String`). Formatierung, typisierte Filter, Aggregate, Export und Bearbeiten brauchen alle
denselben typisierten Wert (`docs/ROADMAP.md`, A1).

**Entscheidung.**

- **`CellValue<'a>` ist der bisherige `SortValue`, erweitert** um `Date` und `DateTime` (hinter dem
  Feature `chrono`). `SortValue` bleibt als Typ-Alias, die Varianten heißen gleich, bestehender Code
  kompiliert unverändert. `Text` borgt weiter aus der Zeile — der Grund dafür (keine Allokation pro
  Zeile beim Sortieren) gilt unverändert.
- **`ColumnSpec::value` ersetzt `sort_key`**, dazu `sortable: bool` (Standard `true`). Eine Spalte
  mit Wert ist damit sortierbar, außer man schaltet es ab. `sort_by`, `sort_by_text` und
  `sort_by_value` bleiben als Kurzformen von `value`, `value_text` und `value_of`.
- **`filter_by` bleibt getrennt.** Suchen über den Wert statt über einen eigenen Text würde das
  Verhalten bestehender Spalten ändern (eine Zahlenspalte wäre plötzlich durchsuchbar). Wie Filter
  und Wert zusammenkommen, entscheidet Phase 8.
- **Ordnung:** `Bool` < Zahl < Datum/Zeit < `Text` < `None`. Ein Datum zählt als Mitternacht. Für
  die bisherigen Varianten ist die Ordnung dieselbe; ein Property-Test vergleicht die Sortierung
  gegen eine unabhängig ausgeschriebene Referenz der 0.4.0-Ordnung.
- **`chrono`, nicht `time`,** mit dem Nutzer abgestimmt: `default-features = false` mit `alloc`,
  optional, ohne `wasmbind` — kein `js-sys`/`web-sys` im Baum (per `cargo tree` geprüft).
  `rust_decimal` kommt erst, wenn ein Anwendungsfall es verlangt.

**Folge.** `ColumnSpec::sort_key` gibt es nicht mehr; wer das Feld direkt las, liest `value`. Das
ist der einzige Bruch, und er gehört in eine 0.x-Minor-Version.

## ADR-0023 — Formate und Texte in `GridLocale`, getragen vom Grid-Handle

**Kontext.** Texte wie „Previous page" standen fest in den Primitives, andere als Props in der
Komponente. Zahlen- und Datumsformate gab es nicht (`docs/ROADMAP.md`, A5).

**Entscheidung.**

- **`GridLocale` liegt im Core**, als reine Daten (`Cow<'static, str>`, `char`, `bool`). Der Core
  braucht die Zahlen- und Datumsformate selbst (Export in Phase 13), und Texte sind nur Strings.
  Englisch ist Standard, Deutsch wird mitgeliefert. Texte mit Platzhaltern (`{count}`) sind Vorlagen,
  gleichnamige Methoden füllen sie. Mit `serde` ist die Struktur (de)serialisierbar, fehlende Felder
  fallen auf Englisch zurück — eine Übersetzung kann aus einer Datei kommen.
- **Keine Pluralregeln-Bibliothek.** `row_count_one` und `row_count` decken Englisch und Deutsch ab;
  Sprachen mit mehr Pluralformen bleiben offen, bis jemand sie braucht.
- **Das Grid-Handle trägt die Locale** (`GridOptions::locale`, `GridHandle::locale`,
  `set_locale`), nicht ein eigener Dioxus-Context. Jedes Primitive bekommt das Handle ohnehin; so
  können zwei Grids auf einer Seite verschiedene Sprachen sprechen, und eine geänderte Option schaltet
  um wie `page_size` und `selection` (Vergleich mit dem zuletzt gesehenen Wert).
- **Text-Props werden `Option<String>`** (`placeholder`, `loading_label`, `retry_label`,
  `search_placeholder`, `empty_message`, `column_picker_label`). Gesetzt gewinnen sie, sonst gilt die
  Locale. Dioxus nimmt für `Option`-Props weiter den blanken Wert an, bestehende Aufrufe kompilieren.
- **Formatierung:** `CellFormat` (Plain, Number, Currency, Percent, Date, DateTime, DatePattern) sagt
  die Art, die Locale die Zeichen. Ein Format, das nicht zum Wert passt, fällt auf Plain zurück.
  Rundung über `format!("{:.*}")`; `-0,00` wird `0,00`. Ein ungültiges `chrono`-Muster würde bei
  `to_string` panicken — geschrieben wird deshalb über `write!`, im Fehlerfall ISO 8601.
- **Darstellung:** Eine Spalte mit Wert und ohne `cell` zeigt den formatierten Wert. Kopf- und
  Datenzellen tragen `data-align` (numerische Formate `end`) und `data-overflow`; die Komponente
  setzt das per CSS mit logischen Werten um (`text-align: end` folgt der Schreibrichtung).
  `TruncateWithTooltip` setzt `title` — nur mit Wert, denn die Ausgabe eines eigenen Renderers ist
  kein Text, den das Grid lesen kann. Standard bleibt `Truncate` ohne Tooltip wie bisher.

## ADR-0024 — Auswahl folgt den Zeilen aus den Daten

**Kontext.** Wurde eine ausgewählte Zeile aus den Daten entfernt, blieb ihr Schlüssel ausgewählt:
unsichtbar, nicht abwählbar, und mitgezählt („2 selected").

**Entscheidung.** `use_grid` entfernt per Effekt Schlüssel, deren Zeile nicht mehr in den Daten ist
(`Selection::retain_existing`, bisher ungenutzt). Nur lokal: Ein Remote-Grid hält eine Seite, und
Zeilen anderer Seiten existieren weiter. Der Effekt schreibt nur bei tatsächlich veralteten
Schlüsseln, damit er keine Render-Schleife auslöst.

## ADR-0025 — Zusatzkomponenten ohne `componentDependencies`, eigene schlichte Popups

**Kontext.** A3 in `docs/ROADMAP.md` sah vor, dass Zusatzkomponenten `data_grid` und offizielle
dx-components (Popover, Dialog) über `componentDependencies` mitinstallieren. Der Spike
(`docs/VERIFICATION.md` §9) zeigt: Jede Abhängigkeit mit `globalAssets` — das offizielle `popover`
wie unser `data_grid` — lässt `dx components add` mit Fehler enden, weil dx deren Theme-Datei gegen
das falsche Registry prüft. Ein String-Verweis zeigt zudem immer aufs offizielle Registry.

**Entscheidung.**

- **Keine `componentDependencies` in unseren Komponenten**, bis dx den Fehler behebt. Die Doku nennt
  den Installationsbefehl mit allen Teilen, etwa `dx components add data_grid data_grid_editor`;
  beide kommen dann aus demselben Registry und teilen dessen Theme.
- **Popover, Dialog und Menü bauen wir als schlichte, eigene Bausteine** in `dioxus-datagrid`
  (Fallback aus `docs/ROADMAP.md` §7): unstyled Primitives mit Tastatur und ARIA, gestylt in der
  Registry-Komponente über die Theme-Variablen. `dioxus-primitives` direkt als Crate zu nutzen wäre
  die Alternative, ist auf crates.io aber nur ein Platzhalter (0.0.0) und sonst nur per Git zu
  haben — für eine veröffentlichte Crate keine tragfähige Abhängigkeit.
- **Der Fehler geht upstream** (Bericht vorbereitet, der Nutzer reicht ihn ein). Behebt dx ihn, wird
  das hier neu entschieden.

**Folge für A3.** Die Architektur bleibt: `data_grid` legt den Handle in den Context und nimmt
Kinder an, Zusatzkomponenten lesen ihn dort. Nur die automatische Mitinstallation entfällt vorerst.

## ADR-0026 — Typisierte Filter neben der Filterleiste

**Kontext.** Phase 8 (`docs/ROADMAP.md`) verlangt Operatoren je Werteart, ein Filtermenü mit zwei
Bedingungen, eine Werteliste wie in einer Tabellenkalkulation und dieselben Filter auf dem Server.

**Entscheidung.**

- **Modell im Core:** `ColumnFilter` = Bedingungen (`Condition` = `FilterOp` + `FilterValue`-
  Operanden), verbunden mit UND oder ODER (`any`). Die Werteliste ist `OneOf`, bei angehakten
  leeren Zellen ODER `IsEmpty`. Ein Modell für Menü und Werteliste, statt zwei Filterarten.
- **Zwei Felder im Zustand:** `GridState::column_filters` (Text der Filterleiste) bleibt, neu ist
  `GridState::filters` (typisiert, vom Menü). Eine Zeile muss beide bestehen. So bleibt gespeicherter
  Zustand und bestehender Code gültig, und `serde(default)` liest alte Zustände wie alte Anfragen.
- **Operanden sind zunächst Text** (`FilterValue::Text`) und werden erst gegen die Werteart der
  Spalte gelesen (`coerce`, einmal pro Filter über `prepared`). Filterleiste und Menü müssen die
  Werteart dafür nicht kennen, und ein Server bekommt, was der Nutzer meinte. Zahlen nehmen `,` als
  Dezimaltrennzeichen, Daten ISO, deutsches und US-Format.
- **Filterleiste:** `>100`, `<=`, `!=`, `=`, `a..b`; sonst Teilstring wie bisher — aber **nur bei
  Text**. Bei Zahl, Datum und Bool heißt schlichter Text „gleich" (`30` findet 30, nicht 130).
  `ColumnFilter::from_bar_text` ist öffentlich, damit ein Server exakt dieselbe Semantik nutzt.
- **Filterbar wird, was einen Wert oder Filtertext hat** (vorher nur Filtertext). Eine Spalte wie
  „Alter" bekommt dadurch ein Feld in der Filterleiste. Der einzige Test, der das Gegenteil
  festhielt, wurde angepasst; die Suche bleibt auf Filtertext beschränkt.
- **Werteart** (`ValueKind`): erklärt per `.kind()` oder aus dem ersten nicht-leeren Wert der Zeilen.
  Remote-Grids sollten sie erklären, weil vor der ersten Seite keine Zeilen da sind.
- **Werteliste:** `distinct_values` wertet alle Filter **außer denen der eigenen Spalte** aus — ein
  abgewählter Wert bleibt in der Liste, Werte, die andere Filter ausschließen, verschwinden.
  Höchstens `limit` Werte (Standard 1.000), `truncated` sagt, ob es mehr gab. Remote über die neue
  Methode `DataSource::distinct_values` mit Standard-Implementierung (leere Liste), also ohne Bruch.
  Das Handle hält dafür eine typlose Closure, damit es nur über den Zeilentyp generisch bleibt.
- **Das Menü ist ein eigenes, ungestyltes Primitive** (`GridFilterMenu`), kein Popover aus
  dx-components (ADR-0025): nicht-modaler Dialog, Klick daneben über eine `position: fixed`-Ebene
  (kein `web-sys` für Klicks außerhalb), Fokus zurück auf den Knopf. In `data_grid` per Prop
  `filter_menu`, in der vorhandenen Filterzeile neben dem Textfeld.
- **Warum Prop statt Zusatzkomponente (A3):** Die Filterzeile gehört schon zu `data_grid`, das Menü
  braucht keinen eigenen Zustand in der Komponente, und zwei Hilfsfunktionen der Komponente
  (`column_template`, `pickable_columns`) wanderten in die Crate — die Datei ist mit Menü kürzer
  (237 Zeilen) als vorher. Die Familie aus A3 beginnt, wo eine Erweiterung eigenen Zustand braucht:
  beim Editor in Phase 9.

**Folge für Server.** `GridQuery` hat `filters`. Das Fullstack-Beispiel übersetzt alle Operatoren
nach SQL; ein Test vergleicht für 3 Spalten × 13 Operatoren × 19 Operanden das SQL-Ergebnis mit dem
lokalen, dazu Filterleisten-Kurzformen, UND/ODER und Wertelisten. Er fand beim ersten Lauf eine
Abweichung (Zahl als Operand von „enthält").

**Gefunden nebenbei.** `dioxus-datagrid` passte `match` auf Varianten hinter dem `chrono`-Feature des
Cores an. Aktiviert eine andere Crate `chrono` nur im Core, kompilierte `dioxus-datagrid` nicht mehr.
Solche Matches liegen jetzt im Core (`FilterValue::edit_text`), und die CI prüft die Mischungen.

## ADR-0027 — Bearbeiten: Das Grid schreibt nie, es übergibt

**Kontext.** Phase 9 (`docs/ROADMAP.md`) verlangt Bearbeiten in der Zelle, in der Zeile, im Dialog
und als Batch, Anlegen und Löschen, Validierung an der Zelle und — für Remote-Daten — eine
optimistische Anzeige, die bei einem Fehler zurückgenommen wird. Die Daten eines Grids sind ein
`ReadSignal` der App; das Grid besitzt sie nicht.

**Entscheidung.**

- **Das Grid schreibt die Daten nie.** Eine Bearbeitung arbeitet auf einer Kopie der Zeile. Beim
  Übernehmen bekommt die App die bearbeitete Kopie über einen Callback (`on_save`, `on_create`,
  `on_delete`, `on_batch_save`) und speichert sie, wo sie will. Lokal und remote dieselbe API:
  lokal schreibt der Callback das Signal, remote ruft er den Server.
- **Ergebnis über ein Token.** Jeder Callback bekommt ein Token (`Save`, `Create`, `Delete`,
  `SaveBatch`) mit den Zeilen. Wird es fallen gelassen, gilt das Speichern als gelungen; `fail(msg)`
  meldet einen Fehler. Das Token darf in einen `async`-Block wandern — `EventHandler` startet ihn
  selbst, im Scope der App (VERIFICATION.md §10). So braucht ein synchroner lokaler Callback keine
  Zeile extra, und ein asynchroner meldet sich, wenn er fertig ist.
- **Optimistisch, mit Rücknahme.** Solange ein Speichern läuft, zeigt das Grid die bearbeitete
  Zeile (`data-saving`). Scheitert es, verschwindet die Kopie, die Daten zeigen wieder den alten
  Stand, und `GridEditStatus` sagt warum. Ein Remote-Grid lädt nach Erfolg seine Seite neu und zeigt
  die Kopie, bis die neue Seite da ist, damit der alte Wert nicht aufblitzt.
- **Typisierte Setter** (`.editable(|row, age: u32| …)`): Der Core liest den getippten Text als
  Werteart der Spalte (`Value::parse`, wie beim Filtern) und wandelt ihn über `FromValue` in den Typ
  des Feldes. Was nicht passt — kein Zahlwert, negativ für `u32`, `2,5` für eine Ganzzahl, leer ohne
  `Option` —, wird mit einer Meldung aus der Locale abgelehnt, bevor der Setter läuft. Dazu
  `.validate` je Spalte und `validate_row` je Zeile. Unveränderte Felder einer bestehenden Zeile
  werden nicht neu geprüft (alte Daten dürfen heutige Regeln verletzen); neue Zeilen vollständig.
- **Vier Modi:** `Cell` (speichert je Zelle, zusätzlich zu SfGrid, weil in Tabellen üblich),
  `Row`, `Dialog`, `Batch`. `Enter` in der Zelle übernimmt und geht nach unten wie in einer
  Tabellenkalkulation, `Tab` zur nächsten bearbeitbaren Zelle; in der Zeile wechselt `Tab` zwischen
  deren Editoren.
- **Die Bearbeitung folgt ihrer Zeile per Schlüssel**, nicht per Position. Sortiert oder lädt das
  Grid während einer Bearbeitung neu, wandert der Editor mit; verlässt die Zeile die Seite, wird die
  Bearbeitung verworfen, damit das Grid seine Tasten zurückbekommt.
- **Eigene Dialoge statt dx-components** (wie ADR-0025): modal mit `aria-modal`, Fokuswächtern an
  beiden Enden und einer `position: fixed`-Ebene dahinter. `<dialog>.showModal()` oder `inert`
  bräuchten `web-sys` oder `eval`.
- **Registry:** `data_grid` nimmt `children` und legt sein Handle in den Context (A3, ROADMAP §9).
  `data_grid_editor` ist die erste Zusatzkomponente: `DataGridEditor` als Kind von `DataGrid`, mit
  Werkzeugleiste, Status, Formular und Löschabfrage. Ohne `componentDependencies` (ADR-0025) — die
  Doku sagt, `data_grid` zuerst zu installieren. Die Zell-Editoren selbst rendert die Crate in
  `GridCell`, `data_grid` muss sie nicht kennen.
- **`FilterValue` heißt jetzt `Value`**, weil derselbe Typ geschrieben wird; der alte Name bleibt
  als veralteter Alias.

**Abweichung vom Plan.** Neue Zeilen werden **immer im Formular** angelegt und erscheinen im Grid
erst, wenn sie gespeichert sind — auch im Batch, wo sie nur gezählt werden. Eine neue Zeile inline
im Grid bräuchte eine Zeile, die nicht in den Daten steht, mitten in `aria-rowindex`, Tastatur und
Virtualisierung; das ist genau die View aus Zeilenarten (A2), die Phase 10 ohnehin baut. Gelöschte
Zeilen verschwinden, wenn die App sie aus den Daten nimmt; im Batch werden sie bis zum Speichern
durchgestrichen gezeigt.

## ADR-0028 — Gruppen als Zeilen der View, gerechnet lokal oder auf dem Server

**Kontext.** Phase 10 (`docs/ROADMAP.md`) verlangt Gruppieren nach einer oder mehreren Spalten,
auf- und zuklappbar, Aggregate in Gruppenkopf, Gruppenfuß und unter dem Grid, und das mit Paging,
Virtualisierung und Remote-Daten, als Treegrid bedienbar. Vorher kommt A2: die View als Liste von
Zeilenarten.

**Entscheidung.**

- **A2:** `View::rows` ist eine Liste von `ViewRow` — `Data(index)`, `GroupHeader(group)`,
  `GroupFooter(group)` —, und alles, was Zeilen per Position anspricht (Rendern, Tastatur,
  Virtualisierung, `aria-rowindex`, Bearbeiten), zählt über sie. `View::indices` bleibt als
  Liste der Datenzeilen, damit Auswahl und bestehender Code weiter funktionieren. Der Umbau kam als
  eigener Schritt mit grünen Tests vor der ersten Gruppe. `ViewRow` ist `#[non_exhaustive]`, damit
  Detailzeilen (Phase 12) dazukommen können.
- **Gruppieren ist Sortieren plus Schnitt.** Die gruppierten Spalten gehen der Sortierung voran, in
  ihrer Sortierrichtung, aufsteigend wenn unsortiert; danach gilt der Rest der Sortierung innerhalb
  der Gruppe. Gruppen sind die Läufe gleicher Werte in dieser Reihenfolge. Der Schlüssel einer
  Gruppe (`GroupKey`) ist der Pfad ihrer Werte; über ihn merkt sich `GridState`, welche Gruppen vom
  Standard (`groups_collapsed`) abweichen. Beides ist serialisierbar und überlebt Persistenz.
- **Paging zählt jede Zeile**, also auch Gruppenköpfe und -füße; eine zugeklappte Gruppe belegt
  einen Platz. So bleibt eine Seite eine Seite, auch wenn Gruppen zugeklappt sind; eine Gruppe
  kann auf der einen Seite beginnen und auf der nächsten enden. Köpfe werden auf Folgeseiten nicht
  wiederholt, sonst hätte dieselbe Zeile zwei `aria-rowindex`.
- **Aggregate** (`Sum`, `Average`, `Min`, `Max`, `Count`, eigene über die Zeilen) hängen an der
  Spalte. Leere Zellen zählen nicht, `Count` zählt Werte wie SQL `COUNT(spalte)`. Eine Summe bleibt
  ganzzahlig, solange alle Werte es sind und sie nicht überläuft. Wo sie erscheinen: im Fuß einer
  aufgeklappten Gruppe, im Kopf einer zugeklappten (deren Fuß mit ihr verschwindet) und in
  `GridFooter` über alle gefilterten Zeilen. Gruppenfüße entstehen nur, wenn eine Spalte ein
  Aggregat hat.
- **Treegrid.** Mit Gruppen wird `role="grid"` zu `treegrid`. Ein Gruppenkopf ist eine Zeile mit
  **einer** Zelle über alle Spalten (`aria-colspan`), mit `aria-level`, `aria-expanded`,
  `aria-posinset`, `aria-setsize`; Datenzeilen und Füße liegen eine Ebene tiefer. Auf dem Kopf
  klappt `ArrowRight` auf, `ArrowLeft` zu bzw. springt zur umgebenden Gruppe; `Enter` und
  `Space` schalten um. Auf Datenzeilen bleiben die Pfeile Zellnavigation — das Muster sieht
  Zeilenfokus vor, den das Grid nicht hat; eine Kopfzeile mit einer Zelle ist die Entsprechung.
  `GridFooter` gehört zur Tastaturnavigation (letzte Zeile) und meldet seine Höhe, damit ein
  klebender Fuß beim Virtualisieren keine Zeile verdeckt.
- **Gruppenleiste** (`GridGroupPanel`, Registry `data_grid_group_panel`): Spaltenköpfe werden per
  HTML-Drag-and-Drop auf die Leiste gezogen (VERIFICATION §12). Jede Zieh-Aktion hat einen
  Tastaturweg: eine Liste zum Hinzufügen, je gruppierter Spalte Knöpfe „zuerst danach
  gruppieren" und „nicht mehr danach gruppieren", dazu „alle auf-/zuklappen".
- **Remote.** `GridQuery` trägt Gruppierung, Auf-/Zuklappzustand und die gewünschten Aggregate;
  `Page` kann ein Layout aus `ViewRow`, die Gruppen der Seite, die Gesamtzeilenzahl und die Summen
  zurückgeben — alles mit `serde(default)`, alte Server und Clients verstehen sich weiter. Für
  Server, die ihre Zeilen im Speicher haben, reichen `query.state()`, `Aggregate::apply_query`,
  `compute_view` und `Page::from_view`. Für SQL rechnet `GroupedPage::plan` aus Gruppenzählungen
  (ein `GROUP BY` je Ebene) aus, welche Köpfe, Füße und Zeilenabschnitte auf die Seite fallen; nur
  diese Abschnitte werden mit `LIMIT`/`OFFSET` geholt. Ein Property-Test belegt, dass Planer plus
  Nachladen exakt die lokale Seite ergibt; das Fullstack-Beispiel belegt es gegen SQLite.
  Eigene Aggregate sind Closures des Clients und bleiben lokal.

**Abweichungen vom Plan.**

- **Gruppieren „per Spaltenmenü"** kommt mit dem Spaltenmenü in Phase 11. Bis dahin ist die Liste
  in der Gruppenleiste der Weg ohne Ziehen, auch per Tastatur.
- **`DataGridGroupPanel::<T>`** braucht den Zeilentyp als Turbofish, weil es keine typisierten
  Props hat, aus denen er folgen könnte.
- Das **Fullstack-Beispiel** zeigt Gruppen und Gruppenfüße, aber keine Summenzeile: Sie verschöbe
  alle bestehenden Zeilenzählungen seiner Tests. Das Server-Beispiel zeigt sie.

---

## ADR-0029 — Spaltenmenü als Zusatzkomponente, geöffnet mit `Alt+↓`

**Kontext.** `docs/ROADMAP.md` Phase 11 nennt ein Spaltenmenü am Kopf mit „sortieren, filtern,
gruppieren, fixieren, ausblenden, automatische Breite". Drei Fragen waren offen: wo es lebt, wie
es ohne Maus erreichbar ist, und ob das Filtern hineingehört.

**Entscheidung.**

1. **Eigene Registry-Komponente `data_grid_column_menu`**, nicht eine Prop an `data_grid`. Die
   Kernkomponente steht bei 256 Zeilen, die Regel sagt „< ~250"; Pinning und Menü hätten sie
   deutlich darüber getrieben. Das ist genau der Fall, für den A3 die Familie vorgesehen hat.
   Die Anmeldung läuft wie bei der Gruppenleiste über das Handle (`set_column_menu`), nicht über
   einen Import: `data_grid` muss die Zusatzkomponente nicht kennen, und wer sie nicht
   installiert, kompiliert nichts davon.

2. **Der Knopf ist kein Tab-Stopp.** Er sitzt in einer Gitterzelle, und „das Gitter ist ein
   einziger Tab-Stopp" ist eine geprüfte Zusage (`docs/ACCESSIBILITY.md`). Vier fokussierbare
   Knöpfe in vier Köpfen wären vier weitere. Er trägt `tabindex="-1"` und wird mit **`Alt+↓`**
   auf dem fokussierten Spaltenkopf geöffnet — die übliche Taste für ein Menü an einem
   Bedienelement, und sie reiht sich in `Alt+←/→` (Breite) und `Alt+Shift+←/→` (Reihenfolge) ein.
   Geschlossen wird auf den **Kopf** zurückfokussiert, nicht auf den Knopf: Fokus auf einem
   Element ohne Tab-Stopp ließe die Pfeiltasten des Gitters ins Leere laufen.

   *Alternative:* Knopf mit `tabindex="0"`. Verworfen — bricht die Zusage und ihre Tests.

3. **Offen/zu liegt im Handle**, nicht in der Komponente (`open_column_menu`). Sonst könnte die
   Kopfzelle das Menü in sich nicht öffnen; nebenbei ist damit höchstens ein Menü offen.

4. **Kein Filtern im Menü.** `data_grid` setzt mit `filter_menu` bereits ein Filtermenü an jede
   Spalte. Zwei Wege zum selben Panel wären zwei Stellen zum Suchen und zwei Zustände, die
   auseinanderlaufen können. Das ist eine bewusste Abweichung von der Aufzählung in der Roadmap.

5. **„Automatische Breite" ist „Breite vergessen".** Auto-Spalten wachsen ohnehin mit ihrem Inhalt;
   was der Nutzer will, ist die gezogene Breite loszuwerden. Der Eintrag erscheint deshalb nur,
   wenn es eine gibt, und ruft `reset_column_width`.

**Folgen.**

- Die Kopfzelle benennt sich jetzt selbst (`aria-label` aus dem Spaltenlabel). Ohne das wäre der
  Knopftext in den Namen der Spalte gerutscht — „Name Column options for Name".
- Klicks im Menü werden gestoppt, bevor sie die Kopfzelle erreichen: ein Klick auf den Knopf hätte
  sonst die Spalte sortiert.
- Solange das Menü offen ist, hebt die Kopfzelle ihr `overflow: hidden` auf und steigt im
  `z-index`; sonst schnitte sie das Panel an ihrer Kante ab.
- Was eine Spalte anbietet, liegt als `column_menu_entries` offen, und `ColumnAction::apply` führt
  es aus — wer ein eigenes Menü baut, braucht die Komponente nicht.

---

## ADR-0030 — Zellen über mehrere Spalten: eine Zeile entscheidet, die Tastatur folgt

**Kontext.** `docs/ROADMAP.md` Phase 11 nennt „Zellen über Spalten verbinden (Column Spanning),
soweit ARIA es sauber erlaubt". Offen war, wer die Breite bestimmt, was mit den verdeckten Spalten
passiert und wie sich die Pfeiltasten verhalten.

**Entscheidung.**

1. **Die Spalte sagt es pro Zeile**, nicht die Zeile pro Spalte: `Column::span(|row| n)`. Eine
   Zeile kennt ihre Spalten nicht — die Spalten sind es, die wissen, was sie darstellen, und die
   schon `cell`, `value` und `format` pro Zeile liefern. Ohne `span` liest die Bibliothek keine
   Zeile: `row_spans` gibt sofort `RowSpans::none` zurück.

2. **Die verdeckten Spalten rendern nichts.** `GridRow` fragt `RowSpans`, welche Zellen es gibt,
   und rendert nur die. `aria-colindex` zählt weiter in Spalten, nicht in Zellen, und die breite
   Zelle trägt `aria-colspan` — damit bleibt die Zeile für den Screenreader so breit wie der Kopf.
   Die Zeile ist ein `subgrid`, also sagt dieselbe Zelle dem Layout `grid-column: span n`.

3. **Eine Zelle verlässt ihren fixierten Block nicht.** Sie kann nicht gleichzeitig am Rand kleben
   und mitscrollen; eine Spanne, die über die Kante des Blocks liefe, wird an ihr gekappt. Ebenso
   an der letzten Spalte. Ein `span` von 0 oder 1 ist dasselbe: die Spalte allein.

4. **Die Spanne folgt der Anordnung, nicht der Deklaration.** Sie verdeckt die Spalten, die gerade
   rechts von ihr stehen — Umsortieren, Ausblenden und Fixieren ändern also, was sie verdeckt. Das
   ist die Regel, die keine zweite Wahrheit einführt; die Alternative (benannte Nachbarn) hätte
   einen zweiten Ordnungsbegriff neben `column_order` gebraucht.

5. **Die Tastatur bewegt sich in Zellen.** `RowSpans::step` setzt den Fokus nach jedem Zug auf
   eine Zelle: ein Zug nach rechts, der in derselben Zelle endete, geht über sie hinaus — eine
   Pfeiltaste, die sichtbar nichts tut, liest sich als kaputte Taste; ein Zug, der auf einer
   verdeckten Spalte landet, landet auf der Zelle darüber. Ein Zug nach unten behält die Spalte
   und ist deshalb ausdrücklich kein Zug nach rechts — die erste Fassung verwechselte beides, und
   ein E2E-Test hat es gefunden.

6. **Dieselbe Regel gilt für die mehrstufigen Köpfe.** Eine Gruppenkopfzelle ist eine Zelle über
   mehreren Spalten, also ist sie jetzt ebenfalls ein einziger Stopp: `→` springt zur nächsten
   Gruppe, statt innerhalb der Gruppe stehen zu bleiben. Das ist eine Verhaltensänderung
   gegenüber 0.8.0, und die bessere: vorher war die Taste in einer breiten Gruppe stumm.

**Folgen.**

- `GridCell` hat eine neue Prop `span` (Vorgabe 1). Wer die Primitives von Hand in einer Schleife
  zusammensetzt, fragt `grid.row_spans(row)`; `GridRow` und damit `data_grid` tun es von selbst.
- Die Spanne wird aus der **gespeicherten** Zeile gelesen, nicht aus einer laufenden Bearbeitung:
  eine Zelle, die unter dem offenen Editor die Breite wechselt, wäre schlechter als eine, die
  einen Takt zu spät nachzieht.
- `GridGroupRow` und die Fußzeilen rechneten mit „eine Kopfzeile"; sie fragen jetzt
  `header_rows()`. Mit mehrstufigen Köpfen *und* Gruppierung waren `aria-rowindex` und die
  Fokuszeile vorher um die Gruppenebenen verschoben.

---

## ADR-0031 — `cargo audit` wöchentlich, nicht bei jedem Pull Request

**Kontext.** `PLAN.md` §7 zählt die CI-Prüfungen auf; eine Sicherheitsprüfung der Abhängigkeiten
ist nicht darunter. Die Frage war, ob sie fehlt und, falls ja, wann sie laufen soll.

**Entscheidung.** Ein eigener Workflow `.github/workflows/audit.yml` mit
`rustsec/audit-check@v2.0.0`, ausgelöst **wöchentlich** (Montag 06:00 UTC), bei Änderungen an
einer `Cargo.toml` oder `Cargo.lock`, und von Hand.

- **Nicht bei jedem PR.** Ein neues Advisory entsteht nicht aus einem Commit, sondern aus dem
  Kalender. Liefe die Prüfung bei jedem Pull Request, ginge irgendwann ein völlig unbeteiligter
  PR rot, und die Prüfung würde als Rauschen gelesen und abgeschaltet.
- **Aber bei Änderungen an den Manifesten.** Das ist der eine Moment, in dem ein eigener Commit
  eine verwundbare Abhängigkeit hereinholen kann. Dort meldet die Aktion am Lauf, statt ein Issue
  anzulegen; beim Wochenlauf legt sie eines an.
- **Kein `cargo deny`** — vorerst. Es kann mehr (Lizenzen, Dubletten, verbotene Crates), braucht
  aber eine gepflegte Konfiguration und macht bei einem Baum aus 684 Paketen zuerst Lärm mit
  Versionsdubletten, die niemanden stören. Wenn Lizenzprüfung ein Thema wird, ist das der Weg.

**Folgen.** Ein Befund heißt hier „die Anforderung in `Cargo.toml` anheben", nicht „unsere Nutzer
sind betroffen": geprüft wird das committete `Cargo.lock` dieses Workspace, und wer gegen die
veröffentlichten Crates baut, löst seine Abhängigkeiten selbst auf. Für eine Bibliothek ist das
der ehrliche Wert der Prüfung — sie sagt, wann eine Anforderung zu alt geworden ist.

**Erster Lauf, lokal verprobt (2026-09-26, `cargo-audit` 0.22.2, 684 Pakete).** Ein Befund und
sechs Warnungen — und keines davon liegt im Baum von `dioxus-datagrid`; sie erscheinen erst unter
`--all-features --target all`, also im Desktop-, Server- und Beispiel-Unterbau von dioxus:

| Advisory | Paket | Woher | Art |
|---|---|---|---|
| RUSTSEC-2026-0009 | `time` 0.3.45 | `dioxus-fullstack` → `reqwest` → `cookie` | DoS, mittel |
| RUSTSEC-2025-0057 | `fxhash` | `selectors` | unmaintained |
| RUSTSEC-2024-0436 | `paste` | `pulp`, `rav1e` | unmaintained |
| RUSTSEC-2024-0370 | `proc-macro-error` | `glib-macros`, `gtk3-macros` | unmaintained |
| RUSTSEC-2024-0429 | `glib` | GTK-Stack des Linux-Desktops | unsound |
| RUSTSEC-2026-0253 | `lru` | `dioxus-server` | unsound |
| RUSTSEC-2026-0097 | `rand` 0.7.3 | `phf_generator` | unsound |

Alle sieben stehen in der `ignore`-Liste des Workflows, jede mit ihrem Grund im Kommentar. Sonst
legte der Wochenlauf sieben Issues an, die niemand schließen kann, und die Prüfung wäre binnen
eines Monats Rauschen. Was zählt, ist die **achte** Meldung.

**Der `time`-Befund ist behoben, nicht ignoriert.** Die Behebung ist `time >= 0.3.47`, und die
verlangt Rust 1.88 — über der damaligen MSRV von 1.85, weshalb Cargos MSRV-bewusster Resolver
0.3.45 festhielt. Statt den Befund auf die Ignorierliste zu setzen, ist die MSRV gestiegen
(ADR-0032); die Liste enthält seither nur noch die sechs Warnungen, für die es keine Behebung
gibt, und keine Verwundbarkeit.

---

## ADR-0032 — MSRV 1.88, um eine Verwundbarkeit tatsächlich zu beheben

**Kontext.** Der erste `cargo audit`-Lauf (ADR-0031) fand RUSTSEC-2026-0009 in `time` 0.3.45.
Behoben ist das in `time >= 0.3.47`, und die verlangt Rust 1.88. Bei `rust-version = "1.85"` hält
Cargos MSRV-bewusster Resolver die alte Version fest, und es blieben zwei Wege: den Befund
ignorieren oder die MSRV anheben.

**Entscheidung.** `rust-version = "1.88"` im Workspace (alle Crates erben es). Die drei Gründe:

1. **Eine Verwundbarkeit gehört nicht auf eine Ignorierliste.** Sonst verliert die Liste genau die
   Eigenschaft, die sie nützlich macht — dass darin nur steht, was nicht zu beheben ist.
2. **Die Reichweite kostet fast nichts.** 1.88 erschien am 2025-06-26, also über ein Jahr vor dieser
   Entscheidung. Wer Dioxus 0.7 und Edition 2024 benutzt, ist ohnehin nicht auf einer Toolchain von
   vorgestern.
3. **Der Resolver deckt die Folgen ab.** Nach dem Anheben verlangt kein Paket in der `Cargo.lock`
   mehr als 1.88 (geprüft über `cargo metadata`: 0 von 434 Paketen mit deklarierter `rust-version`
   liegen darüber, 10 liegen genau auf 1.88).

**Alternative.** *MSRV bei 1.85 lassen und RUSTSEC-2026-0009 ignorieren.* Verworfen: der Befund
betrifft zwar nur das Fullstack-Beispiel — `time` liegt nicht im Baum von `dioxus-datagrid` —, aber
eine ignorierte Verwundbarkeit ist eine, die man beim nächsten Mal auch ignoriert.

**Folgen.**

- Eine MSRV-Anhebung ist für Nutzer eine brechende Änderung im Sinne der Erwartung, auch wenn
  Cargo sie nur als „Paket verlangt neuere Toolchain" meldet. Sie geht deshalb mit dem
  **Minor-Release 0.9.0** hinaus und steht im Changelog unter *Changed*.
- **Ein CI-Job bewacht die Zusage**, sonst wäre sie nur behauptet: `MSRV (1.88)` in `main.yml`
  prüft mit `dtolnay/rust-toolchain@1.88.0` die beiden Bibliotheks-Crates mit allen Features. Nur
  die zwei — die Beispiele und das Playground ziehen den Desktop- und Server-Unterbau herein, dessen
  Minimum nicht unseres ist. Steigt die MSRV wieder, sind es zwei Stellen: `Cargo.toml` und dieser
  Job.
- **Clippy schreibt jetzt mehr vor.** Mehrere Lints sind an die MSRV gekoppelt: mit 1.88 sind
  `let`-Ketten stabil, also verlangt `collapsible_if` sie, und `is_multiple_of` ersetzt `% n == 0`.
  Bei `-D warnings` in CI ist das keine Empfehlung, sondern Pflicht — zehn Dateien wurden
  entsprechend umgeschrieben, mechanisch und bedeutungsgleich. Es ist derselbe Grund, aus dem der
  Sprung nicht in zwei Commits passt: Code mit `let`-Ketten baut mit 1.85 nicht.

---

## ADR-0033 — Zwischenablage: schreiben mit Rückfallweg, einfügen nur aus dem `paste`-Ereignis

**Kontext.** `docs/ROADMAP.md` Phase 12 verlangt „Kopieren als TSV, das Excel versteht; Einfügen in
bearbeitbare Zellen" und schreibt vor, den Zugriff zuerst per Spike zu prüfen. Der Spike liegt vor
(`docs/VERIFICATION.md` §15).

**Entscheidung.**

1. **Über `document::eval`, nicht über Events.** `dioxus-html`s `ClipboardData` ist leer — `onpaste`
   nennt den Inhalt nicht. Eine eigene API hat Dioxus nicht, `web-sys` ist uns verboten (A2). Damit
   ist `document::eval` der einzige plattformneutrale Weg, und die Grenze bleibt gewahrt:
   `dioxus-datagrid` bekommt kein `web-sys`, nur JavaScript als Text.

2. **Die Nutzlast reist über den Kanal, nie im Quelltext.** `eval.send(tsv)` und `await
   dioxus.recv()` statt `format!` in den Skripttext. TSV enthält Tabulatoren, Zeilenumbrüche und
   Anführungszeichen; eine Interpolation wäre eine Injektionsstelle in fremde Daten — genau wovor
   die Dioxus-Dokumentation zu `eval` warnt.

3. **Schreiben: `writeText`, bei Ablehnung `execCommand("copy")`.** `writeText` ist der moderne Weg
   und im echten Chromium wie in WebKit erlaubt; das kopflose Chromium der CI verweigert es ohne
   erteilte Berechtigung, und dort trägt `execCommand`. Beide Wege statt einer Berechtigungsbitte:
   ein Gitter soll nicht nach Rechten fragen, um eine Zeile zu kopieren.

4. **Einfügen kommt aus dem `paste`-Ereignis, nicht aus `readText`.** `readText` wurde in jeder
   geprüften Umgebung verweigert, auch mit gültiger Benutzeraktion. Ein Eval installiert stattdessen
   einen `paste`-Listener und schickt `event.clipboardData.getData("text/plain")` per `dioxus.send`
   zurück; Rust empfängt in einer Schleife. Folge für die API: Einfügen ist **kein** Aufruf, den die
   App auslösen kann, sondern ein Ereignis, auf das das Gitter reagiert — `on_paste` gibt es, ein
   `paste_now()` nicht.

5. **Ein Listener pro Gitter, im Handle.** Der Kanal lebt, solange das Skript wartet (ein nie
   erfülltes Promise), also gehört er in einen Hook des Gitters und nicht in jede Zelle. Das ist
   dieselbe Lehre wie bei der Breitenmessung in Phase 11: eine Stelle, nicht eine pro Zelle.

**Folgen.**

- Ein E2E-Test für das Einfügen muss ein `ClipboardEvent` mit eigenem `DataTransfer` verschicken;
  ein echtes `Ctrl+V` ist nicht synthetisierbar. Das ist eine Einschränkung der Prüfung, keine des
  Produkts, und gehört als Kommentar in den Test.
- Desktop und Mobile sind ungeprüft: derselbe Weg, anderes WebView. Vor dem Ausliefern von Phase 12
  an einem Desktop-Build nachzusehen.
- Der Spike (`playground/src/clipboard_spike.rs`, `tests/e2e/clipboard-spike.spec.ts`) bleibt,
  solange gebaut wird, und wird danach entfernt.

---

## ADR-0034 — Zellauswahl neben der Zeilenauswahl, als Ort statt als Inhalt

**Kontext.** `docs/ROADMAP.md` Phase 12 nennt „Zellauswahl und rechteckige Bereiche (`Shift+Pfeil`
im Zellmodus)". Drei Fragen waren offen: wie sie sich zur bestehenden Zeilenauswahl verhält, woran
ein Rechteck hängt, und wer `Shift`+Pfeil bekommt.

**Entscheidung.**

1. **Zwei Auswahlen, nicht ein Modus mit Varianten.** `CellSelectionMode::{None, Single, Range}`
   steht **neben** `SelectionMode`, nicht darin. Ein Gitter kann Zeilen auswählen, Zellen, beides
   oder keines. Die Alternative — `SelectionMode` um `Cell` erweitern — hätte `selection_mode()`
   zweideutig gemacht: jede Stelle, die heute „darf ich Zeilen auswählen?" fragt, hätte etwas
   anderes bedeutet.

2. **Ein Rechteck ist ein Ort, kein Inhalt.** `CellRange` hält zwei `CellFocus`, also
   Ansichtskoordinaten. Zeilenauswahl hängt am Schlüssel und übersteht Sortieren, Filtern und
   Blättern; ein Rechteck kann das nicht — es hat keinen Schlüssel. Also gilt für es dieselbe Regel
   wie für den Fokus: es wird an die Größe des Gitters **geklemmt**, nicht gelöscht. Was
   hervorgehoben ist, ist ausgewählt; das bleibt nach einer Sortierung wahr, auch wenn andere Daten
   darunterliegen. *Alternative:* bei jeder Ansichtsänderung löschen. Verworfen — es wäre eine
   zweite Regel für dasselbe Problem, und „mein Kopieren-Bereich ist weg" ist die unfreundlichere
   Überraschung.

3. **Der Anker gehört zum Rechteck.** `CellRange { anchor, focus }` statt zweier Ecken „oben links /
   unten rechts": nur so lassen `Shift`+Pfeil und `Shift`+Klick *dasselbe* Rechteck wachsen und
   schrumpfen. `top_left`/`bottom_right` sortieren beim Lesen, nicht beim Speichern.

4. **Bei `Range` gewinnen die Zellen `Shift`+Pfeil.** Die Taste erweitert dann das Rechteck und
   nicht die Zeilenauswahl; `Space` bleibt der Zeile. Ein Gitter, das Rechtecke auswählt, wird als
   Tabellenblatt gelesen, und dort ist `Shift`+Pfeil die Bereichstaste. Bei `Single` — das nur eine
   Zelle verspricht — bleibt `Shift`+Pfeil bei den Zeilen, und die Zellauswahl folgt dem Fokus.

5. **Kopfzeilen sind nicht auswählbar.** `select_cell` und `extend_cell_selection` lehnen eine
   Kopfzeile ab, statt in den Körper zu verschieben. Ein Kopf ist nichts, was man kopieren würde,
   und eine stillschweigende Verschiebung wäre eine erfundene Absicht.

**Folgen.**

- Jede Datenzelle trägt `aria-selected`, sobald Zellauswahl eingeschaltet ist — und nichts, solange
  sie aus ist. `aria-multiselectable` gilt, sobald *eine* der beiden Auswahlen mehr als einen
  Eintrag zulässt.
- Eine Zelle über mehrere Spalten zählt als **eine** Zelle: sie gilt als ausgewählt, wenn das
  Rechteck irgendeine der Spalten erreicht, die sie verdeckt.
- `GridOptions` hat ein neues öffentliches Feld (`cell_selection`), `data_grid` eine neue Prop.
- Das Rechteck ist **nicht** Teil von `GridState`: es überlebt kein Neuladen, wie die Zeilenauswahl
  auch nicht. Mehrere Rechtecke (Strg-Klick) gibt es bewusst noch nicht; sie kommen, wenn ein
  Anwendungsfall sie verlangt, und würden aus `Option<CellRange>` ein `Vec` machen.

**Nachtrag zu ADR-0033 (Kopieren gebaut, 2026-09-27).** Drei Festlegungen, die beim Bauen fällig
wurden:

- **Was `Ctrl+C` bedeutet, in dieser Reihenfolge:** das ausgewählte Rechteck; sonst die ausgewählten
  Zeilen mit allen sichtbaren Spalten; sonst die fokussierte Zelle. Ein Rechteck ist die genauere
  Aussage des Nutzers und schlägt deshalb die Zeilenauswahl; die fokussierte Zelle ist der Rest, der
  ohne jede Auswahl noch sinnvoll ist. Zeilen werden in **Ansichtsreihenfolge** kopiert, nicht in
  der Reihenfolge des Anklickens — kopiert wird ein Block, kein Verlauf.
- **Kopiert wird der formatierte Wert**, nicht die Rohzahl: „1.234,50 €". Was man sieht, klebt man
  ein. Eine Spalte ohne Wert, die nur mit `cell` etwas zeichnet, kopiert leer — ein gerendertes
  `Element` ist Markup, und daraus Text zu raten wäre schlechter als eine leere Zelle. Ein
  `Column::copy_as` könnte das später gezielt öffnen.
- **Ohne Kopfzeile.** Eine Überschriftenzeile ins Blatt zu kippen, das schon eine hat, ist der
  häufigere Ärger; wer sie will, kann sie aus `copy_text` selbst bauen.

Dazu ein Fund aus dem Property-Test des Formats: eine erste Zeile aus lauter leeren Zellen schluckte
den Zeilenumbruch danach, weil der Erzeuger „ist schon etwas geschrieben?" statt „ist das die erste
Zeile?" fragte. Aus einem 2×1-Block wurde ein 1×1-Block.

**Nachtrag zu ADR-0033 (Einfügen gebaut, 2026-09-27).** Was beim Bauen zu entscheiden war:

- **Wo der Block landet:** an der oberen linken **Ecke** des ausgewählten Rechtecks, sonst an der
  fokussierten Zelle; von dort nach rechts und unten. Eine einzelne eingefügte Zelle füllt dagegen
  das ganze Rechteck — so macht es jede Tabellenkalkulation, und ein Nutzer, der drei Zellen markiert
  und einen Wert einfügt, meint genau das.
- **Was hängen bleibt, fällt weg.** Kein Einfügen legt Zeilen an und keines legt Spalten an. Eine
  Zeile wäre eine Zeile ohne Schlüssel — `new_row` gehört der Anwendung, und ein Einfügen ist kein
  Anlass, sie hinter ihrem Rücken aufzurufen.
- **Eine nicht bearbeitbare Spalte hält ihren Platz.** Sie behält ihren Wert, verschluckt aber die
  Zelle, die über ihr lag. *Alternative:* sie überspringen und die restlichen Werte nachrücken
  lassen. Verworfen — dann landet die Abteilung im Alter, sobald irgendwo dazwischen eine Spalte
  nicht editierbar ist. Ausrichtung schlägt Vollständigkeit.
- **Eine Zeile ganz oder nicht.** Wird ein Wert abgelehnt — Zahl unlesbar, Prüfung der Spalte oder
  der Anwendung dagegen —, bleibt die **ganze** Zeile, wie sie war. Ein halb eingefügter Datensatz
  ist schlimmer als ein nicht eingefügter, und ein `validate_row` kann sonst nichts Sinnvolles
  prüfen. Gezählt wird in `PasteReport::refused`, und das Gitter sagt es über
  `GridLocale::paste_refused` in seiner Statuszeile.
- **Gespeichert wird zeilenweise, über denselben Weg wie eine bestätigte Bearbeitung.**
  `save_row` — aus `commit_edit` herausgezogen — heißt: im Stapelmodus in die Änderungen, sonst ein
  `on_save` pro Zeile. Kein neuer Weg für dieselbe Sache.
- **Der eingefügte Block bleibt ausgewählt.** Auch der abgelehnte: dort ist nachzusehen.
- **Der Listener sitzt am Dokument, gefiltert in JS.** Ein Einfügen mit Fokus auf einer Gitterzelle
  zielt nicht auf etwas, worauf das Gitter selbst hören könnte. Was Text annimmt — `input`,
  `textarea`, `select`, `contenteditable` — behält sein eigenes Einfügen; deshalb prüft das Skript
  das Ziel, nicht Rust. Ein Gitter, ein Listener, beendet über den Kanal, wenn das Gitter geht.

**Offen, und ehrlich offen:** dass ein **echtes** `Ctrl+V` das Gitter erreicht, ist nicht gemessen.
Die Automatisierung kann es nicht — über das Debug-Protokoll geschickte Tasten fügen nicht ein, nicht
einmal in ein gewöhnliches `input` (`docs/VERIFICATION.md` §15, Nachtrag). Alles ab dem Listener ist
geprüft, in Playwright wie im echten Browser. Die Frage dahinter ist, ob WebKit ein `paste` überhaupt
ausliefert, wenn der Fokus auf nichts Bearbeitbarem liegt; Chromium tut es (daran hängen die
Einfügen-Funktionen der Zeichen-Apps). Falls eine Prüfung von Hand zeigt, dass WebKit es nicht tut,
ist der bekannte Ausweg ein verstecktes `textarea`, das bei `Ctrl+V` kurz den Fokus bekommt — Aufwand
und Fokus-Turnen, die erst eine Messung rechtfertigt.

Der Spike (`playground/src/clipboard_spike.rs`) bleibt deshalb noch stehen: seine Paste-Probe ist
genau das Werkzeug für diese Prüfung von Hand. Er geht heraus, sobald sie gelaufen ist.

---

## ADR-0035 — Checkbox-Spalte: eine Spalte wie jede andere, gezeichnet vom Gitter

**Kontext.** `docs/ROADMAP.md` Phase 12 verlangt eine „Checkbox-Spalte mit ‚alle auswählen'
(dreistufig)". Drei Fragen: woher die Spalte kommt, was „alle" heißt, und wie der dritte Zustand
überhaupt darstellbar ist.

**Entscheidung.**

1. **Die Anwendung erklärt die Spalte, das Gitter zeichnet sie.**
   `Column::new("select", "").checkbox()` — eine gewöhnliche Spalte mit einem Schalter, kein
   Sonderweg am Spaltenmodell vorbei. *Alternative:* eine Option am Gitter
   (`GridOptions::checkbox_column(true)`), die eine synthetische Spalte voranstellt. Verworfen: sie
   müsste durch alles hindurch, was Spalten zählt — `aria-colindex`, Layoutbreiten, Fokusspalten,
   Fixieren, Umsortieren, Ausblenden, Spannen, Kopieren. Als echte Spalte trägt sie all das
   umsonst. Die Anwendung kann sie darum auch fixieren, umsortieren oder ausblenden.
   Gezeichnet wird sie trotzdem vom Gitter, denn nur das Gitter weiß, was ausgewählt ist; ein
   `cell`-Renderer bekommt die Zeile, nicht den Zustand.

2. **Die Spalte bringt mit, was sie sein will:** schmal, zentriert, nicht sortierbar, nicht
   gruppierbar, nicht in der Breite zu ziehen. Alles davon lässt sich danach überschreiben — der
   Schalter ist ein Vorschlag, keine Mauer.

3. **„Alle" heißt: alle, die zu sehen sind.** Die Kopf-Checkbox deckt die Zeilen der aktuellen
   Seite; eine Auswahl auf einer anderen Seite wird weder mitgezählt noch angefasst. *Alternative:*
   alle gefilterten Zeilen über alle Seiten. Verworfen — eine Checkbox, die behauptet, Zeilen zu
   halten, die niemand sieht, ist schlimmer als eine, die nur über die Seite vor einem spricht. Bei
   einem entfernten Gitter wäre sie zudem eine Behauptung über Daten, die gar nicht da sind.

4. **Der dritte Zustand ist eine DOM-Eigenschaft, also wird er als solche gesetzt.** HTML hat kein
   `indeterminate`-Attribut, und Dioxus' Interpreter schreibt nur `value`, `checked`, `selected`
   und ihre `initial_`-Formen als Eigenschaften (`docs/VERIFICATION.md` §16). Die Kopf-Checkbox
   trägt deshalb eine erzeugte `id`, und ein `document::eval` setzt `box.indeterminate` nach jeder
   Änderung — derselbe Weg wie bei der Zwischenablage (ADR-0033), aus demselben Grund: es gibt
   keinen anderen, der plattformneutral bleibt.
   *Alternative 1:* `aria-checked="mixed"` auf der nativen Checkbox. Verworfen — HTML-AAM leitet den
   Zustand einer nativen Checkbox aus ihren Eigenschaften ab; das Attribut wäre Markup, das
   Browser ignorieren, also eine Zusicherung, die nichts zusichert.
   *Alternative 2:* ein `div role="checkbox"` mit echtem `aria-checked="mixed"`. Verworfen — ohne
   CSS wäre es unsichtbar, und die Primitiven sollen ohne Stylesheet benutzbar bleiben. Die
   Filtermenüs benutzen native Checkboxen; zwei Bauformen für dieselbe Sache wären eine dritte
   Antwort auf eine beantwortete Frage.

5. **Keine zusätzlichen Tab-Stopps.** Beide Checkboxen tragen `tabindex="-1"`. Das Gitter ist ein
   Stopp; die Zelle um eine Box herum trägt die Tastatur — `Space` auf einer Zeile schaltet sie um,
   `Space` auf der Kopfzelle alle. Ein Klick auf die Box selbst wird dort behandelt und nicht
   weitergereicht, damit die Zelle ihn nicht ein zweites Mal auslegt.

6. **Ein Klick auf eine Checkbox-Zelle schaltet um, statt hinzuzufügen.** Sonst fügt ein Klick auf
   eine Zelle im `Multi`-Modus nur hinzu; in einer Checkbox-Spalte wäre das falsch herum. `Shift`
   reicht wie überall vom Anker bis hierher. Eine Zellauswahl beginnt sie nicht: ein Rechteck, das
   in der Auswahlspalte anfängt, kopierte eine leere Spalte.

7. **Kopiert wird sie nicht.** Beim Kopieren ausgewählter Zeilen bleibt eine Checkbox-Spalte
   draußen — sie ist Bedienelement, kein Wert, und würde als leeres erstes Feld in der Tabelle
   landen.

**Folgen.**

- `ColumnSpec` hat ein neues öffentliches Feld (`checkbox`), `GridLocale` zwei neue Texte
  (`select_all`, `select_row`), `GridHandle` zwei neue Methoden (`visible_selection`,
  `toggle_select_all`), und der Kern eine neue Aufzählung `SelectionExtent`.
- Eine Kopfzelle ohne eigene Beschriftung leiht sich den Namen ihrer Box, damit kein Spaltenkopf
  ohne Namen dasteht.
- Gruppenzeilen bekommen keine Checkbox. „Eine Gruppe auswählen" ist eine eigene Entscheidung und
  kommt, wenn ein Anwendungsfall sie verlangt.
