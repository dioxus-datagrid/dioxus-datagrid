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
| MSRV dieses Workspace | **1.85** (durch Edition 2024 erzwungen, liegt über 1.83) |
| Edition dieses Workspace | 2024 |

Quelle: `curl https://crates.io/api/v1/crates/dioxus/versions`. 0.8.0-alpha.1 existiert bereits,
ist aber Alpha — der Workspace pinnt bewusst auf `dioxus = "0.7.10"`.

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
