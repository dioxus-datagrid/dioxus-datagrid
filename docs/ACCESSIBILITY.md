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

## Automatisiert geprüft

- **Markup:** SSR-Tests in `crates/dioxus-datagrid/tests/aria.rs`.
- **Verhalten im Browser:** Playwright in `tests/e2e/data-grid.spec.ts` gegen den Playground —
  Sortieren per Klick und Tastatur, Filter, Suche, Paging, alle drei Selection-Modi,
  Tastaturnavigation, und dass der DOM-Fokus tatsächlich der Navigation folgt.
- **`axe`:** `@axe-core/playwright` über die ganze Komponente, im hellen **und** im dunklen Theme,
  ohne Befund. Der erste Lauf fand zu schwachen Kontrast bei gedämpftem Text (3,61:1); behoben,
  siehe ADR-0014.

## Was noch fehlt

- **Virtualisierung (Phase 4).** `aria-rowindex` ist bereits so ausgelegt, dass ein virtuelles
  Fenster korrekt zählt; getestet ist das erst mit Phase 4.
- **Spalten-Resizing (Phase 5).** Das Resize-Handle braucht eine Tastaturalternative
  (`aria-valuenow` an einem `separator`-Element, Pfeiltasten zum Verbreitern).
- **Screenreader-Praxistest.** Markup, Verhalten und axe sind automatisiert geprüft, ein
  Durchlauf mit NVDA und VoiceOver steht aber aus. axe findet keine Probleme der Ansage-Qualität.
