# Phase 0 — Verifikation

Stand: 2026-09-16. Werkzeuge: `rustc 1.96.1`, `cargo 1.96.1`, `dx 0.7.10 (57d6794)`,
`git 2.47.1`, `node 22.23.1`.

Jeder Punkt nennt Befehl oder Quelle. Der Spike unter `playground/src/spike.rs` ist live gegen
Chromium gelaufen; die gemessenen Werte stehen jeweils dabei — siehe aber die Korrektur zu
`onscroll` in Abschnitt 3. In Phase 3 wurde er aus dem Playground entfernt, der seitdem die
Registry-Komponente rendert; der Code liegt im Commit `e13db57`.

---

## 1. Dioxus-Version und MSRV

| | |
|---|---|
| Letzte stabile 0.7 | **0.7.10** (2026-07-30) |
| MSRV von `dioxus` 0.7.10 | 1.83.0 |
| MSRV dieses Workspace | **1.88** (Edition 2024 verlangt 1.85, `time` 0.3.47+ dann 1.88 — ADR-0032) |
| Edition dieses Workspace | 2024 |

Quelle: `curl https://crates.io/api/v1/crates/dioxus/versions`. 0.8.0-alpha.1 existiert bereits,
ist aber Alpha — der Workspace pinnt bewusst auf `dioxus = "0.7.10"`.

**MSRV gegen einen echten Compiler geprüft (2026-09-26).** `rustup toolchain install 1.88.0`, dann
`RUSTFLAGS="-D warnings" cargo +1.88.0 check -p datagrid-core -p dioxus-datagrid --all-features`
→ `Finished`, Exit 0, mit `rustc 1.88.0 (6b00bc388 2025-06-23)`. Genau das tut der CI-Job
`MSRV (1.88)`. Zwei Dinge deckt er bewusst nicht ab: den `wasm32`-Ziel-Build (die MSRV-Frage sind
Compiler-Features, nicht Ziele — dafür gibt es den eigenen `wasm`-Job) und `cargo test` (die
Dev-Dependencies gehören nicht zur Zusage, ein Nutzer baut sie nicht).

Der Workspace kompiliert leer: `cargo check --workspace` → `Finished`.
`cargo fmt --all --check` und `cargo clippy --workspace --all-targets --all-features -- -D warnings`
sind ohne Befund.

## 2. Read-only-Signal-Prop-Typ

**`ReadSignal<T>`.** `ReadOnlySignal` ist der 0.6-Name und in 0.7 nicht mehr die Konvention.

Beleg: `dioxus-components/primitives/src` enthält 354 Treffer für `ReadSignal` und **0** für
`ReadOnlySignal`. Zusätzlich im Spike (`ReadSignalPanel`) kompiliert und gerendert — ein
`&'static str` am Aufrufort wird automatisch in `ReadSignal<String>` überführt.

## 3. Event-APIs auf Web und Desktop

Alle vier benötigten APIs existieren in Dioxus 0.7 und liefern brauchbare Daten. Die Zahlen unten
sind die tatsächlich gemessenen Werte aus dem Spike, jeweils gegen `getBoundingClientRect` bzw.
die DOM-Properties gegengeprüft.

### `onscroll` — tauglich, ohne Einschränkung

`ScrollData` bietet `scroll_top()`, `scroll_left()`, `scroll_width()`, `scroll_height()`,
`client_width()`, `client_height()` (`dioxus-html-0.7.10/src/events/scroll.rs`).

Gemessen auf einem **verschachtelten** Scroll-Container:

```
gemeldet: scroll_top=450  client_height=128  scroll_height=6000
DOM:      scrollTop=450   clientHeight=128   scrollHeight=6000
```

Wichtig: `scroll` bubbelt im DOM nicht. Dioxus weiß das — `dioxus-core-types-0.7.10/src/bubbles.rs`
führt `"scroll" => false`, und der Interpreter hängt nicht-bubbelnde Listener direkt an das Element
(`createListener` in `dioxus-interpreter-js-0.7.10/src/ts/core.ts`). Ein `onscroll` an einem
inneren Container funktioniert daher korrekt.

> **Korrektur (Phase 4).** Im Spike wurde das Scroll-Event per `dispatchEvent(new Event("scroll"))`
> ausgelöst. Belegt war damit, dass `ScrollData` die richtigen Werte liefert, nicht dass das Event
> bei echtem Scrollen feuert. Das ist seit Phase 4 mit Playwright belegt; siehe ADR-0016.

**Konsequenz:** Die Virtualisierung in Phase 4 braucht kein `document::eval`. Das offizielle
`VirtualList` benutzt zwar eines, aber nur wegen variabler Zeilenhöhen (Scroll-Korrektur,
Scroll-End-Debounce). Bei fester Zeilenhöhe entfällt beides.

### `onmounted` + `get_client_rect()` — korrekt, aber **nicht zum Mount-Zeitpunkt**

Das ist der einzige echte Fallstrick, den der Spike gefunden hat.

```
im onmounted-Handler gemessen: [x=8.0   y=8539.3  w=0.0   h=36.0]
später on demand gemessen:     [x=49.0  y=1561.9  w=32.0  h=74.0]
getBoundingClientRect:         {x:49,   y:1561.875, w:32,  h:74}
```

Die On-demand-Messung trifft den DOM exakt. Die Mount-Messung trifft ihn nicht einmal annähernd:
`x=8` ist der Default-Body-Margin, `y=8539` die Position im ungelayouteten Dokumentfluss. Der
`onmounted`-Handler läuft also, **bevor das Stylesheet angewendet und das Layout stabil ist.**

**Konsequenz für Phase 2/4:** Viewport-Maße niemals im `onmounted`-Handler festschreiben. Das
`MountedData` im Signal behalten und die Messung entweder über `onresize` beziehen (feuert per
ResizeObserver mit korrekten Werten, siehe unten) oder bei Bedarf erneut anstoßen.

### `onresize` — tauglich

`ResizeData::get_content_box_size()` / `get_border_box_size()`
(`dioxus-html-0.7.10/src/events/resize.rs`). Der Interpreter setzt dafür einen `ResizeObserver`
auf (`createResizeObserver` in `core.ts`), das Event feuert also auch initial nach dem Layout.

Gemessen: `content_box width=192.0 height=64.0`, passend zu den CSS-Maßen `12rem × 4rem`.

### Pointer-Events — tauglich, aber **ohne Pointer Capture**

`PointerData` liefert `pointer_id()`, `pointer_type()`, `is_primary()`, Druck/Tilt und über
`InteractionLocation` die `client_coordinates()`. Alles vorhanden.

**Dioxus 0.7 hat keine `setPointerCapture`-API.** Gemessen mit Listenern direkt am Drag-Handle:

| Drag | Ergebnis |
|---|---|
| innerhalb des Handles | `delta_x=181 moves=2` — korrekt, entspricht exakt der Drag-Distanz |
| über den Handle-Rand hinaus | `delta_x=0 moves=0 left_handle=true` — **die Moves hören am Rand auf** |

Für Phase 5 (Spaltenbreite per Drag) wäre das fatal: ein Resize-Handle ist wenige Pixel breit, der
Zeiger verlässt ihn sofort.

Der Spike hat deshalb direkt die Alternative gegengetestet — Handle startet nur den Drag,
`onpointermove`/`onpointerup` hängen an einem großen Vorfahren:

```
Start auf einem 24 px breiten Handle → delta_x=605 max_delta_x=605 moves=2
```

Das trägt. Siehe [DECISIONS.md](DECISIONS.md) → ADR-0002.

### Desktop

Desktop und Mobile laufen über WebView und benutzen denselben Interpreter (`dioxus-interpreter-js`)
wie Web; die DOM/CSS-Parität aus `PLAN.md` Abschnitt 1 gilt damit auch für die Events oben.
`cargo check -p playground --no-default-features --features desktop` ist grün (Exit 0).

~~Noch offen: Ausführung auf einem echten Desktop-Fenster und auf Android/iOS.~~ Geprüft in Phase 4
mit dem Playground auf Windows-Desktop und dem Android-Emulator, siehe Abschnitt 8.

## 4. Component-Schema

`dx components schema` → gespeichert unter [`docs/component.schema.json`](component.schema.json).

Felder: `name` (Pflicht), `description`, `authors`, `members`, `exclude`, `globalAssets`,
`cargoDependencies`, `componentDependencies`. Eine `cargoDependency` ist entweder ein String oder
ein Objekt mit `name` / `version` / `git` / `rev` / `features` / `default_features`.

## 5. Konventionen aus `DioxusLabs/dioxus-components`

Analysiert am Klon von `main`.

- **Root-Manifest** `component.json` listet unter `members` Pfade zu den Komponenten. Die liegen
  dort in der Preview-App (`preview/src/components/<name>`), nicht in einem eigenen Registry-Ordner.
  Wir weichen ab und legen sie unter `registry/<name>` — siehe ADR-0003.
- **Komponenten-Ordner:** `component.json`, `component.rs`, `mod.rs` (`mod component; pub use
  component::*;`), `style.css`, `docs.md`, `variants/`. `docs.md`, `component.json` und `variants`
  stehen im `exclude`.
- **Theme:** die Variablen leben in `preview/assets/dx-components-theme.css` — nicht
  `dx-components.css`, wie `PLAN.md` Abschnitt 4.3 annahm. Komponenten referenzieren sie über
  `globalAssets`.
  Verfügbare Variablen: `--primary-color` … `--primary-color-7`, `--secondary-color` …
  `--secondary-color-6`, `--focused-border-color`, sowie Success/Warning/Error/Info-Paare.
  Light/Dark läuft über den Trick `var(--dark, X) var(--light, Y)`, gesteuert per
  `html[data-theme=...]` und `prefers-color-scheme`.
- **Styling:** neuere Komponenten benutzen das CSS-Modul-Makro
  `#[css_module("/src/components/<name>/style.css")]` aus `dioxus-code` und referenzieren Klassen
  typisiert als `Styles::dx_pagination`. Ältere schreiben Klassennamen als String.
- **Attribut-Weitergabe:** `#[props(extends = GlobalAttributes)] attributes: Vec<Attribute>` und
  `..attributes` im rsx; für zusammengesetzte Fälle `merge_attributes` + `attributes!`.
- **Playwright:** `playwright/` mit `playwright.config.ts`, Tests gegen `dx run --web --release`
  auf Port 8080, Projekte chromium/firefox/webkit, `@axe-core/playwright` für a11y.
- **Lizenz:** MIT OR Apache-2.0.

## 6. Eigene Registry referenzieren

Zwei Wege, beide verifiziert.

**Über `Dioxus.toml`** (Keys bestätigt in `dioxus-cli-0.7.10/src/config/component.rs`:
`ComponentConfig { registry, components_dir }`):

```toml
[components]
registry = { path = "../../../registry" }   # oder { git = "...", rev = "..." }
components_dir = "src/components"
```

**Über CLI-Flags:** `--path <PATH>` bzw. `--git <URL> --rev <REV>` an `add` / `list` / `remove`.
CLI-Flags schlagen die Config; ohne beides fällt der CLI auf
`https://github.com/dioxuslabs/components` zurück (`resolve_or_default`).

**Wichtig:** `dx components` muss aus einem Binary-Crate heraus laufen, sonst bricht es mit
„Failed to find binary package to build" ab.

Der komplette Weg ist als [`scripts/registry-smoke.sh`](../scripts/registry-smoke.sh)
automatisiert und läuft grün gegen `tests/fixtures/consumer-app`:

```
dx components add data_grid --path <registry>
  → src/components/data_grid/{mod.rs,component.rs}   kopiert
  → src/components/mod.rs                            angelegt, "pub mod data_grid;" eingetragen
  → assets/dx-datagrid-theme.css                     globalAsset kopiert
  → cargo add dioxus                                 cargoDependency ausgeführt
  → component.json und docs.md                       korrekt ausgelassen (exclude)
cargo check                                          grün
```

Der CLI weist danach selbst darauf hin, dass `mod components;` von Hand in `main.rs` gehört.

> **Korrektur (Phase 3).** Geprüft war hier nur die `--path`-Form. Die `--git`-Form verhält sich
> anders: der CLI klont das Repo und liest `component.json` **im Repo-Root**. Mit dem Manifest
> allein unter `registry/` schlug `dx components list --git
> https://github.com/dioxus-datagrid/dioxus-datagrid` mit „Failed to open component manifest"
> fehl. Behoben durch ein Root-Manifest, das `registry` als Member führt — der CLI löst Members
> rekursiv auf. Der Smoke-Test läuft seitdem gegen das Repo-Root, also genau den Einstiegspunkt,
> den `--git` benutzt. Siehe ADR-0009.

Nebenbefund: `globalAssets` werden nach Dateiname ins Zielverzeichnis kopiert und müssen innerhalb
des Registry-Roots liegen. Zwei Komponenten mit gleichnamigem Asset überschreiben sich also
gegenseitig — relevant für Phase 3, wenn wir das offizielle Theme mitliefern wollen.

## 7. crates.io-Namen

`cargo search` liefert für **`dioxus-datagrid`** und **`datagrid-core`** jeweils null Treffer.
Beide Namen sind frei; die Namen aus `PLAN.md` bleiben. Reserviert ist bislang nichts —
veröffentlicht wird erst nach expliziter Freigabe.

---

## Offene Punkte

- ~~**Mobile-Verifikation steht aus.**~~ Android-Emulator geprüft in Phase 4, siehe Abschnitt 8.
  Ursprünglich: Der Spike lief auf Chromium und ist für Desktop nur
  Compile-geprüft. Android-Emulator / iOS-Simulator hängen an der offenen Entscheidung in
  `PLAN.md` Abschnitt 9.
- ~~`registry/data_grid` ist ein Phase-0-Stub.~~ Erledigt in Phase 3.
- ~~`cargoDependencies` des Stubs zeigt auf `dioxus`.~~ Erledigt in Phase 3: die Komponente hängt
  per `git` an `dioxus-datagrid`; nach einer Veröffentlichung auf crates.io wird daraus die
  `version`-Form.
- ~~CI ist noch nie remote gelaufen.~~ Läuft seit dem ersten Push auf
  `github.com/dioxus-datagrid/dioxus-datagrid`.

---

## 8. Phase 4: manueller Test auf dem Android-Emulator

Geprüft am 2026-09-17 mit dem Playground (`dx serve --android --release --package playground
--no-default-features --features mobile`) auf einem Pixel-10-Pro-Emulator, Host Windows 11.

**Ergebnis.**

- Virtualisierung funktioniert: 100.000 Zeilen, nach dem Scrollen erscheinen die richtigen Zeilen.
- **Fehler gefunden: die Kopfzeile scrollte weg.** `position: sticky` saß auf den Kopfzellen statt
  auf der Kopfzeilen-Gruppe und betraf alle Plattformen. Behoben, mit Playwright-Test.
- Beim schnellen Wischen leere Zeilen mit Overscan 5, kurz mit 20, zäh ab 40. Daraus ADR-0017.
- Kein sichtbarer Scrollbalken: Android blendet Overlay-Scrollbalken in scrollbaren Elementen nur
  während der Bewegung ein. Plattformverhalten, kein Fehler des Grids.

**Hürden beim Bauen auf einem Windows-Host.**

- **`dx` 0.7.10 linkt für Android auf Windows nicht.** `dx` schreibt die Linker-Argumente in eine
  Antwortdatei mit Windows-Pfaden in Anführungszeichen; der NDK-Clang liest sie für ein
  Nicht-MSVC-Ziel nach GNU-Regeln, in denen `\` ein Escape ist — aus `C:\dev\…` wird `C:dev…`.
  Nachgestellt mit einer Antwortdatei direkt gegen `clang.exe`. Umgehung: in
  `ndk\<version>\toolchains\llvm\prebuilt\windows-x86_64\bin\x86_64-linux-android28-clang.cmd`
  (und `aarch64-…`) `--rsp-quoting=windows` vor `%*` ergänzen. Gemeldet als
  [DioxusLabs/dioxus#5847](https://github.com/DioxusLabs/dioxus/issues/5847).
- `JAVA_HOME` muss auf das Verzeichnis zeigen (`…\Android Studio\jbr`), nicht auf `java.exe`.

**Desktop.** Geprüft am 2026-09-17 unter Windows 11 (`dx serve --desktop --release --package
playground --no-default-features --features desktop`, WebView2): Scrollen flüssig, Kopfzeile bleibt
stehen.

**Web.** Geprüft am 2026-09-17 (`dx serve --web --release --package playground`): Scrollen flüssig.
Das Verhalten ist zusätzlich durch Playwright auf
Chromium, WebKit und (in CI) Firefox abgedeckt.

## 9. Phase 7: `componentDependencies` (Spike zu A3)

Geprüft am 2026-09-18 mit dx 0.7.10, Quelle `packages/cli/src/cli/component.rs` auf `v0.7.10` und
`main`, dazu eine Spike-Komponente in einem frischen Projekt.

- **Ein String verweist immer auf das offizielle Registry.** `ComponentDependency::Builtin(name)`
  löst über `ComponentRegistry::default()` auf, also `https://github.com/dioxuslabs/components` —
  nicht auf das Registry, aus dem die abhängige Komponente kommt. Eine Zusatzkomponente, die
  `"data_grid"` als String nennt, bekäme `data_grid` aus dem offiziellen Registry, wo es keins gibt.
  Eigene Komponenten müssen als `{ "name", "git", "rev" }` angegeben werden.
- **Abhängigkeiten werden rekursiv aufgelöst**, schon installierte still übersprungen.
- **Ohne `rev` wird der Klon nie aktualisiert.** `resolve()` lädt nur, wenn das Verzeichnis unter
  `~/.dx/components/` fehlt. Im Spike lag dort ein Klon dieses Repos vom 16.09. (`2dcda2c`, noch
  ohne `component.json` im Wurzelverzeichnis); dx scheiterte daran, statt neu zu holen. Das trifft
  auch `dx components add data_grid --git …` direkt: Nutzer bekommen den Stand ihres ersten Aufrufs.
  Abhilfe: `dx components update --git <url>` (steht jetzt im README) oder ein festes `rev`.
- **Blocker: `globalAssets` einer Abhängigkeit scheitern.** `copy_global_assets` vergleicht jeden
  Asset-Pfad mit der Wurzel des *aufrufenden* Registrys, die einmal zu Beginn von `add` bestimmt
  wird. Die Assets einer Abhängigkeit liegen in deren eigenem Klon, also immer „außerhalb":
  `Cannot copy global asset '…/preview/assets/dx-components-theme.css' for component 'popover'
  because it is outside of the component registry '…'`. Die Dateien der Komponente sind dann schon
  kopiert, das Theme nicht, und der Befehl endet mit Fehler. Reproduziert mit dem offiziellen
  `popover` wie mit unserem `data_grid` (mit `rev`). Auf `main` unverändert.

**Folge:** Solange dx das nicht behebt, nennen unsere Zusatzkomponenten keine
`componentDependencies`. Siehe ADR-0025.

## 10. Phase 9: Callbacks, die asynchron speichern

Geprüft am 2026-09-19 an `dioxus-core`, `dioxus-hooks` und `dioxus-signals` 0.7.10 (Quelle im
Cargo-Register), dann per SSR-Test (`crates/dioxus-datagrid/tests/editing.rs`) und Playwright.

- **`EventHandler<T>` nimmt synchrone Closures und `async`-Blöcke.** `SpawnIfAsync` (in
  `events.rs`) startet einen zurückgegebenen `Future<Output = ()>` selbst per `spawn`. Ein
  `on_save: move |save| async move { … }` braucht also keinen eigenen Rückgabetyp.
- **`Callback::call` läuft im Scope, in dem der Callback erzeugt wurde** (`origin`, über
  `with_scope_on_stack`), nicht im Scope des Aufrufers. Das `spawn` eines asynchronen Handlers hängt
  damit an der Komponente der App, nicht an der Editor-Zelle, die beim Übernehmen verschwindet.
  Ein Umweg über `Runtime::in_scope` ist nicht nötig.
- **`Callback`s vergleichen ihre Identität** (`ptr_eq` plus `origin`). Callback-Props behalten sie
  über Renders hinweg, weil Dioxus beim Diffing nur die innere Funktion austauscht.
  `GridHandle::set_editing` kann daher bei jedem Render aufgerufen werden und schreibt nur bei
  echter Änderung. Ein `EventHandler::new(…)` im Render-Körper erzeugte dagegen jedes Mal einen
  neuen — in Tests und eigenen Komponenten deshalb in `use_hook` oder `use_callback`.
- **`use_callback`** (dioxus-hooks) hält einen `Callback` über Renders stabil und tauscht nur die
  Closure. Die Editoren reichen so `set_text`, `onkeydown` und `onmounted` an eigene Editoren
  weiter, ohne bei jedem Render neue Callbacks anzulegen.
- **`Writable::try_write`** liefert einen Fehler statt zu paniken, wenn das Signal schon verworfen
  ist. Ein Speichervorgang, der erst endet, nachdem die Seite verlassen wurde, schreibt so nichts.
- **`VirtualDom::in_scope(ScopeId::ROOT, …)`** führt Code im Runtime aus; so treiben die Tests das
  Handle Schritt für Schritt, ohne Browser-Events.

## 11. Nach Phase 9: Stand von Dioxus 0.8

Geprüft am 2026-09-19 (ROADMAP §7: „nach Phase 9 prüfen, ob 0.8 stabil ist").

- **Nicht stabil.** Neuestes Release ist `0.8.0-alpha.1` (31.07.2026), zeitgleich mit `0.7.10`; davor
  `0.8.0-alpha.0` (19.05.2026). Der Meilenstein „0.8.0" auf GitHub hat 41 offene und 13
  geschlossene Issues, sein Termin (30.06.2025) ist verstrichen. Die Release Notes der Alpha
  nennen neue Events (Auswahl, `beforeinput`, Zwischenablage-Daten) und Fehlerbehebungen, keine
  Brüche in Signals, Props, Callbacks oder den Events, die wir nutzen.
- **Spike:** Workspace in einem Worktree auf `dioxus = "=0.8.0-alpha.1"` gestellt.
  `dioxus-datagrid` und der Playground kompilieren ohne Änderung und ohne Clippy-Befund, alle
  Tests von Core und Crate bestehen (SSR-Tests, Remote, Bearbeiten), `web-sys` bleibt draußen.
- **Folge:** Wir bleiben auf 0.7. Ein Umstieg sieht nach heutigem Stand billig aus; er wird fällig,
  wenn 0.8 stabil erscheint. Die Zwischenablage-Events der Alpha sind für Phase 12 interessant:
  Sie könnten das `document::eval` aus Entscheidung 4 (ROADMAP §8) für das Einfügen ersparen.

## 12. Phase 10: Ziehen auf eine Gruppenleiste

Geprüft gegen `dioxus-html` 0.7.10 (`src/events/drag.rs`, `src/data_transfer.rs`).

- **HTML-Drag-and-Drop ist da:** `ondragstart`, `ondragover`, `ondragleave`, `ondrop`, `ondragend`
  mit `DragData`. `DragData::data_transfer()` liefert ein `DataTransfer` mit `set_data`/`get_data`.
  Das Attribut `draggable` setzt `rsx!` wie jedes andere.
- **`prevent_default` in `ondragover` nimmt den Drop an**, wie im Browser; ohne ihn feuert `ondrop`
  nicht. `prevent_default` in `ondragstart` bricht das Ziehen ab — so verhindert der
  Resize-Griff, dass ein Spaltenkopf mitgezogen wird.
- **Firefox beginnt kein Ziehen ohne Daten.** Der Kopf legt deshalb seine Spalten-Id per
  `set_data("text/plain", …)` in den `DataTransfer`. Gelesen wird sie von dort nicht: Welche Spalte
  gezogen wird, merkt sich das Handle (`dragged_column`), das funktioniert auch dort, wo ein
  Renderer den `DataTransfer` nicht durchreicht.
- **Warum nicht Pointer-Events:** Bei Touch fängt der Browser den Zeiger implizit am Element, auf
  dem er aufsetzt; ein `pointerup` über der Leiste käme dort nie an. Lösen ließe sich das nur mit
  `releasePointerCapture`, also `web-sys`. HTML-Drag-and-Drop braucht das nicht.
- **Playwright:** `dragTo` löst unter Chromium und Firefox echte HTML-Drags aus, unter WebKit
  nicht. Der Drag-Test läuft deshalb ohne WebKit; der Tastaturweg (Liste in der Leiste) läuft
  überall.
- **Generische Komponenten ohne typisierte Props:** `DataGridGroupPanel::<User> {}` — der Turbofish
  funktioniert in `rsx!`. Nötig, weil nichts sonst den Zeilentyp verrät, unter dem `DataGrid` sein
  Handle in den Context legt.

## 13. Phase 11: Fixierte Spalten im Subgrid

Geprüft am 2026-09-22 im Playground (Chromium, Browser-Pane), Grid auf 370 px Breite mit
198 px Überhang, `Name` an den Anfang und `Alter` ans Ende fixiert.

- **`position: sticky` funktioniert für Zellen im CSS-Subgrid.** Die Zeile ist so breit wie der
  gesamte Inhalt und der Scroll-Container ist `.dg`, also kann eine Zelle darin die volle
  Scrollstrecke wandern. Ganz nach rechts gescrollt: die fixierte `Name`-Spalte steht weiter bei
  `left = 25`, also exakt an der linken Kante des Grids, `Alter` bei `right = 395` an der rechten;
  die ungebundene `E-Mail`-Spalte war auf `left = -77` gewandert.
- **Der Abstand zur Kante braucht kein `web-sys`.** Er ist die Summe der Breiten der fixierten
  Spalten davor (bzw. dahinter), und die kennt das Handle schon: feste Breiten aus der Spalte,
  gemessene aus `onresize` (`record_column_width`). Sie stehen als `--dg-pin-offset` an der Zelle.
- **Virtualisierung: `overflow: hidden` an der Zeile bricht das Fixieren.** Es macht die Zeile zu
  einem eigenen Scroll-Container, und die Zelle klebt dann an der Zeile statt am Grid — im Test
  wanderte sie auf `left = -173`. Mit `overflow: clip` bleibt die Zeile ein bloßer Clip und die
  Zelle steht wieder bei `left = 25`. `clip` gibt es ab Chrome 90, Firefox 81 und Safari 16.
- **Kopfzeilen kleben auf beiden Achsen.** Die Kopfgruppe ist vertikal sticky, die fixierte
  Kopfzelle zusätzlich horizontal; sie braucht deshalb einen höheren `z-index` als beide.
- **Hintergrund ist Pflicht.** Körperzellen haben von sich aus keinen, die scrollenden Zellen
  schienen sonst durch. Die Regeln für Hover und Auswahl sind spezifischer und gewinnen weiterhin,
  damit eine fixierte Zelle ihrer Zeile folgt.

## 14. Phase 11: `onresize` erreicht nur ein Element pro Grid

Geprüft am 2026-09-23 im Playground (Chromium, Browser-Pane), Grid mit vier Spaltenköpfen, die
alle einen `onresize`-Handler tragen.

**Der Befund.** Ändert sich die Breite des Grids, verschickt Dioxus sein `resize`-CustomEvent
**nur auf `.dg`** — dem Element, auf dem `GridRoot` seinen Handler hat. Mit einem Listener in der
Capture-Phase am `document` mitgeschnitten:

```
Breite geändert → 2 Ereignisse, beide mit target = div.dg
Kopfzellen im DOM: 4, davon beobachtet: 0
```

Ein **eigener** `ResizeObserver` auf derselben Kopfzelle meldet dagegen korrekt: 96 px, nach einer
Änderung 174 px. Am Browser und am Element liegt es also nicht.

**Die Folge war still und falsch.** Die Handler der Kopfzellen liefen trotzdem — mit dem Eintrag
des Grid-Wurzelelements. `record_column_width` hat deshalb für jede Auto-Spalte dieselbe
Seitenbreite gespeichert (684 px statt 96 px). Das erklärt auch die vorher hier notierten
404 px und 464 px: dieselbe Messung, nur zu einem anderen Zeitpunkt.

**Die Lösung.** Messen an einer Stelle statt an jeder Zelle:

- Die Kopfzelle **misst nicht mehr selbst**. Sie reicht beim Einhängen ihr `MountedData` an das
  Handle weiter (`register_column_element`) — messen wäre dort ohnehin zu früh, siehe §3.
- `GridHandle::measure_columns` misst alle registrierten Köpfe per `get_client_rect()`.
- `GridRoot` ruft das aus einem `use_effect` auf, also **nachdem** der DOM steht, und zusätzlich
  aus seinem eigenen `onresize` — dem einen, das feuert.

Nachgemessen: Zwei an den Anfang fixierte Spalten, die erste automatisch breit. `--dg-pin-offset`
der zweiten ist 96 px, exakt die gemessene Breite der ersten; ganz nach rechts gescrollt stehen
sie nebeneinander an der Kante (`left = 25` und `left = 121`) statt übereinander.

**Nicht generell kaputt.** Das Panel des Filtermenüs bekommt sein `onresize` sehr wohl — die
Einpassung in den Viewport hängt daran und ist per E2E-Test abgedeckt. Die Regel ist also enger
als „ein Element pro App"; welche Elemente erfasst werden und welche nicht, ist von außen nicht
zu klären.

**Offen für Dioxus.** Ein Bugreport mit dem Capture-Mitschnitt oben wäre der nächste Schritt.
Für uns ist der Weg über eine Stelle ohnehin der bessere: eine Messung statt einer pro Zelle.

**Nachtrag 2026-09-27: der offensichtliche Verdacht ist unschuldig.** In die Quelle geschaut, statt
weiter von außen zu raten:

- `dioxus-core-types/src/bubbles.rs` führt `"resize" => false`. `createListener` nimmt damit den
  Nicht-Bubbling-Zweig und hängt den Handler **an das jeweilige Element**, nicht an die Wurzel.
- `createResizeObserver` beobachtet **jedes** übergebene Element; der eine gemeinsame
  `ResizeObserver` verschickt sein CustomEvent an `entry.target`.
- Dieser Code ist auf `main` von DioxusLabs/dioxus heute zeichengleich mit 0.7.10.

Es ist also nicht „ein Observer pro App", und die Ursache unseres Befunds ist damit weiterhin
unidentifiziert — am wahrscheinlichsten auf der Rust-Seite, beim Zurückrouten des synthetischen
Events an den Handler des Ziels. Eine Suche in den Issues von DioxusLabs/dioxus findet **keinen**
Report zu `onresize`-Abdeckung; offen und für uns einschlägig ist dagegen
[#2293](https://github.com/DioxusLabs/dioxus/issues/2293) „Incorrect values returned by
`MountedData::get_client_rect()`" (seit 2024-04-11) — genau die API, auf der unsere Messung steht.

**Zurückgestellt.** Ein Fix bräuchte eine Minimalreproduktion und Debugging in einem
dioxus-Checkout, und er landete auf `main` = 0.8-Alpha, die wir nicht benutzen. Wir sind nicht
blockiert; der Ausflug wird fällig, wenn Dioxus 0.8 stabil wird (§11).
