# Barrierefreiheit

Umgesetzt nach den [WAI-ARIA Authoring Practices, Data Grid Pattern][apg]. Dieses Dokument
beschreibt, was die Primitives aus `dioxus-datagrid` tatsächlich rendern — nicht, was sie rendern
sollten. Jede Zeile hier ist durch einen SSR-Test in
[`crates/dioxus-datagrid/tests/aria.rs`](../crates/dioxus-datagrid/tests/aria.rs) abgedeckt.

[apg]: https://www.w3.org/WAI/ARIA/apg/patterns/grid/

---

## Rollen

| Element | Rolle | Primitive |
|---|---|---|
| Container | `grid` | `GridRoot` |
| Kopf- und Körpergruppe | `rowgroup` | `GridHeader`, `GridBody` |
| Zeile | `row` | `GridHeader` (Kopfzeile), `GridRow` |
| Spaltenkopf | `columnheader` | `GridHeaderCell` |
| Zelle | `gridcell` | `GridCell` |

Die Primitives rendern `div`-Elemente mit expliziten Rollen statt einer `table`. Grund: eine
`table` mit `role="grid"` verliert ohnehin ihre Tabellensemantik, und `display: grid` mit `subgrid`
ist die praktikable Grundlage für gemeinsame Spaltenspuren und späteres Spalten-Resizing (Phase 5).

## Attribute

### Zählung — der Grund, warum Paging korrekt bleibt

`aria-rowcount` und `aria-rowindex` beziehen sich auf den **gesamten gefilterten Datenbestand**,
nicht auf die gerenderte Seite. Ohne das könnte Hilfstechnologie bei Paging und Virtualisierung
nicht ansagen, wo im Datenbestand man sich befindet.

- `aria-rowcount` am Root = Anzahl gefilterter Zeilen **+ 1 für die Kopfzeile**.
- `aria-rowindex` ist 1-basiert und zählt die Kopfzeile mit: die Kopfzeile ist `1`, die erste
  Datenzeile der ersten Seite ist `2`.
- Auf Seite *p* bei Seitengröße *s* trägt die Zeile *r* (0-basiert) den Index `p·s + r + 2`.
  Seite 2 einer 25er-Seite beginnt also bei `27`.
- `aria-colcount` am Root = Anzahl **sichtbarer** Spalten; `aria-colindex` an Kopfzellen und Zellen
  ist 1-basiert über die sichtbaren Spalten.

Wird gefiltert, sinkt `aria-rowcount` entsprechend — die Ansage bezieht sich auf das, was der
Nutzer gerade durchwandern kann.

### Sortierung

`aria-sort` steht **nur an sortierbaren** Spaltenköpfen:

| Zustand | `aria-sort` |
|---|---|
| sortierbar, aktuell aufsteigend | `ascending` |
| sortierbar, aktuell absteigend | `descending` |
| sortierbar, nicht sortiert | `none` |
| nicht sortierbar | *kein Attribut* |

`none` ist bewusst gesetzt statt weggelassen: es ist das Signal, dass diese Spalte überhaupt
sortiert werden kann.

Zusätzlich für Styling, ohne a11y-Bedeutung: `data-sortable`, `data-sorted` und
`data-sort-priority` (Position innerhalb einer Mehrspaltensortierung, `0` = primär).

Die Registry-Komponente zeigt die Richtung zusätzlich als Pfeil per CSS-`::after`. Der steht als
`content: "▲" / ""` mit leerem Alternativtext: ohne ihn würde der Pfeil Teil des Accessible Name,
und ein Screenreader läse „Name, black up-pointing triangle". Aufgefallen ist das, weil Playwright
den Header nach dem Sortieren nicht mehr über seinen Namen fand.

### Selektion

- `aria-multiselectable="true"` am Root **nur** bei `SelectionMode::Multi`.
- `aria-selected` an Zeilen **nur**, wenn Selektion überhaupt aktiviert ist. Ist sie aus, fehlt das
  Attribut ganz — eine Zeile soll nicht behaupten, auswählbar zu sein, wenn sie es nicht ist.
- Ist Selektion aktiv, trägt **jede** Zeile `aria-selected` mit `true` oder `false`, nicht nur die
  ausgewählten. Das ist die Vorgabe des Patterns.

## Tastatur

Die Kopfzeile ist Teil des Gitters, nicht davor. `CellFocus::row == 0` ist die Kopfzeile,
`row == n` die *n*-te Zeile der aktuellen Seite. Damit stimmt die Fokus-Koordinate direkt mit
`aria-rowindex` überein.

| Taste | Wirkung |
|---|---|
| `↑` `↓` `←` `→` | eine Zelle in die jeweilige Richtung, **begrenzt an den Rändern** (kein Umbruch) |
| `Home` / `End` | erste / letzte Spalte **derselben Zeile** |
| `Ctrl+Home` / `Ctrl+End` | erste / letzte Zelle des Gitters |
| `PageUp` / `PageDown` | eine Seitenhöhe nach oben / unten |
| `Enter` (auf Kopfzelle) | sortieren |
| `Shift+Enter` (auf Kopfzelle) | additiv sortieren (Spalte tritt der Mehrspaltensortierung bei) |
| `Space` (auf Kopfzelle) | sortieren, wie `Enter` |
| `Space` (auf Datenzeile) | Zeilenauswahl umschalten |
| `Shift+Space` | Auswahl vom Anker bis hierher erweitern |
| `Shift+↑` / `Shift+↓` | Fokus bewegen **und** die Auswahl mitziehen (nur bei `Multi`) |

Auf macOS gilt `Cmd` gleichwertig zu `Ctrl` für `Home`/`End`.

Bewegung begrenzt an den Rändern statt umzubrechen: die Authoring Practices behandeln ein Gitter
als Fläche, und ein Umbruch würde die Pfeiltasten unvorhersehbar über Zeilen springen lassen.

### Roving `tabindex`

Genau **ein** Element im Gitter trägt `tabindex="0"`, alle übrigen `tabindex="-1"`. Damit ist das
Gitter ein einziger Tab-Stopp; innerhalb wird mit den Pfeiltasten navigiert.

Der Roving-`tabindex` allein bestimmt nur, was `Tab` erreicht. Nach einem Pfeiltastendruck liegt
der DOM-Fokus noch auf der vorherigen Zelle, also holt ihn die neu fokussierte Zelle aktiv zu sich
(`MountedData::set_focus`). Damit das nicht bei jedem Neurendern passiert und den Fokus aus einem
Suchfeld reißt, zählt `GridHandle` einen Nonce mit, der nur bei *absichtlicher* Fokusbewegung
hochgezählt wird.

## Virtualisierung

Mit `VirtualGridBody` liegen nur die sichtbaren Zeilen plus Overscan im DOM. Für Hilfstechnologie
ändert sich dadurch nichts an der Zählung:

- `aria-rowcount` beschreibt weiterhin alle gefilterten Zeilen, `aria-rowindex` jeder gerenderten
  Zeile zählt vom Anfang der Daten. Zeile 50.000 wird als 50.001 angesagt, egal wie wenige Zeilen im
  DOM stehen.
- `PageUp` / `PageDown` bewegen um die Zeilen, die ganz in den Viewport passen, statt um die ganze
  Datenmenge.
- Tastaturnavigation scrollt die fokussierte Zeile um das nötige Minimum in den Sichtbereich; eine
  Zeile unter dem Rand wird zur untersten sichtbaren, keine springt nach oben.

### Fokus, wenn die fokussierte Zeile verschwindet

Scrollt man per Maus oder Touch von der fokussierten Zeile weg, verlässt ihre Zelle das DOM. Ein
Browser, dessen fokussiertes Element entfernt wird, setzt den Fokus auf die Seite zurück — die
nächste Pfeiltaste ginge dann ins Leere. Deshalb:

1. **Das Grid-Root übernimmt den Fokus** (`tabindex="-1"`, programmatisch), solange die fokussierte
   Zeile nicht gerendert ist. Das Root trägt die Tastenbelegung, die nächste Pfeiltaste setzt also
   an der fokussierten Zeile fort und scrollt zurück.
2. **Das Root wird zum Tab-Stopp** (`tabindex="0"`), solange keine Zelle den Roving-`tabindex`
   tragen kann. Es bleibt bei genau einem Tab-Stopp.
3. **Tabbt man zurück ins Grid**, scrollt das Root zur fokussierten Zeile und gibt den Fokus an
   deren Zelle weiter.

Die Fokusübergabe während eines Sprungs (etwa `Ctrl+End` über 100.000 Zeilen) ist abgesichert:
Solange eine Fokusbewegung aussteht, parkt das Root den Fokus nicht, sonst würde es ihn der gerade
eingerückten Zelle wieder wegnehmen.

## Spaltenbreite und Spaltenauswahl

Mit `GridHeader { resizable: true }` trägt jede in der Breite veränderbare Kopfzelle einen
`ColumnResizeHandle`.

- **Tastatur:** Auf einer fokussierten Kopfzelle verbreitert `Alt+→` die Spalte um 16 px, `Alt+←`
  verschmälert sie. Die Kopfzelle kündigt das mit `aria-keyshortcuts="Alt+ArrowLeft Alt+ArrowRight"`
  an. Der Fokus bleibt dabei stehen; ohne `Alt` navigieren die Pfeiltasten wie gewohnt.
- **Der Ziehgriff selbst trägt `aria-hidden="true"`** und ist nicht fokussierbar. Er ist eine
  reine Zeiger-Hilfe; ein fokussierbarer `separator` im Gitter hätte keine Position im
  Roving-Tabindex und wäre ein zweiter Tab-Stopp. Siehe ADR-0018.
- **Mindestbreite:** Keine Spalte wird schmaler als ihr `min_width` (ohne Angabe 48 px), auch nicht
  über wiederhergestellten Zustand.
- **Spalten ausblenden:** `aria-colcount` sinkt, `aria-colindex` der übrigen Spalten bleibt
  lückenlos. Die letzte sichtbare Spalte lässt sich nicht ausblenden. Liegt der Fokus auf einer
  Spalte, die verschwindet, rückt er auf die nächste vorhandene Zelle — das Gitter bleibt genau ein
  Tab-Stopp. Dasselbe gilt, wenn ein Filter die fokussierte Zeile entfernt.
- Das Spaltenmenü der Registry-Komponente liegt **außerhalb** von `role="grid"` und besteht aus
  normalen Checkboxen mit Label.

## Serverseitige Daten

Bei `use_grid_remote` kommen die Zeilen seitenweise vom Server.

- `aria-rowcount` und `aria-rowindex` beziehen sich auf die **Gesamtzahl des Servers**, nicht auf
  die empfangene Seite — genau wie beim lokalen Paging.
- **Während eine Anfrage läuft, trägt das Grid `aria-busy="true"`** und zeigt weiter die vorherige
  Seite. Es wird nicht leer; Fokus und Tab-Stopp bleiben erhalten.
- **`GridStatus` ist eine Live-Region** (`role="status"`, `aria-live="polite"`). Der Container
  steht immer im DOM, denn Screenreader sagen nur Änderungen in bereits vorhandenen Live-Regionen
  an. Er zeigt „Loading…" während einer Anfrage oder den Fehler mit einem „Retry"-Button.
  Beide Texte sind per Prop übersetzbar.

## Automatisiert geprüft

- **Markup:** SSR-Tests in `crates/dioxus-datagrid/tests/aria.rs`.
- **Verhalten im Browser:** Playwright in `tests/e2e/data-grid.spec.ts` gegen den Playground —
  Sortieren per Klick und Tastatur, Filter, Suche, Paging, alle drei Selection-Modi,
  Tastaturnavigation, und dass der DOM-Fokus tatsächlich der Navigation folgt.
- **Virtualisierung:** `tests/e2e/virtualization.spec.ts` mit 100.000 Zeilen — DOM-Obergrenze,
  `aria-rowindex` tief in den Daten, Pfeiltasten und `PageDown` über den Viewport-Rand,
  `Ctrl+End` bis Zeile 100.001, Fokus nach Wegscrollen und beim Zurücktabben.
- **Spalten:** `tests/e2e/columns.spec.ts` — Ziehen, Mindestbreite, `Alt`+Pfeiltasten,
  Zurücksetzen, Ausblenden bis zur letzten Spalte, Tab-Stopp nach Ausblenden der fokussierten
  Spalte.
- **Serverseitige Daten:** `crates/dioxus-datagrid/tests/remote.rs` (`aria-busy`, Statusregion,
  Zählung über die Server-Gesamtzahl) und `tests/e2e/server.spec.ts` im Browser.
- **`axe`:** `@axe-core/playwright` über die ganze Komponente, im hellen **und** im dunklen Theme,
  ohne Befund. Der erste Lauf fand zu schwachen Kontrast bei gedämpftem Text (3,61:1); behoben,
  siehe ADR-0014.

## Was noch fehlt

- **Ansage der Spaltenbreite.** Ändert sich die Breite per Tastatur, sagt ein Screenreader den
  neuen Wert nicht an; sichtbar ist die Änderung sofort.
- **Screenreader-Praxistest.** Markup, Verhalten und axe sind automatisiert geprüft, ein
  Durchlauf mit NVDA und VoiceOver steht aber aus. axe findet keine Probleme der Ansage-Qualität.
