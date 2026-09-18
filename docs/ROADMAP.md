# Roadmap: auf Augenhöhe mit dem Syncfusion Blazor DataGrid (SfGrid)

Stand: 2026-09-18, nach Release 0.4.0. `PLAN.md` (Phasen 0–6) ist abgeschlossen. Dieses Dokument
beschreibt, was einem Vergleich mit dem SfGrid noch fehlt, und in welcher Reihenfolge wir es bauen.
Es gelten dieselben Arbeitsregeln wie bisher (`CLAUDE.md`): Phasen strikt in Reihenfolge, APIs vor
Benutzung prüfen, Abweichungen in `docs/DECISIONS.md`, Architekturgrenzen unverändert.

Quelle für den Funktionsumfang des SfGrid:
[syncfusion.com/blazor-components/blazor-datagrid](https://www.syncfusion.com/blazor-components/blazor-datagrid).

---

## 1. Maßstab

„Ebenbürtig" heißt hier: Wer heute ein SfGrid in einer Business-Anwendung einsetzt, findet die
Funktionen, die er tatsächlich benutzt, auch bei uns — typisiert, barrierefrei, auf Web, Desktop
und Mobile. Nicht gemeint ist eine Kopie der API oder jedes Randfeatures.

## 2. Stand heute (0.4.0) gegen SfGrid

| Bereich | SfGrid | wir | Lücke |
|---|---|---|---|
| **Datenbindung** | lokal, remote (REST, OData, GraphQL), DataManager | lokal, `DataSource`-Trait, Fullstack-Beispiel | Adapter für gängige Protokolle, Live-Updates |
| **Sortieren** | einfach, mehrfach, eigene Vergleiche | einfach, mehrfach, Kollation | eigener Vergleich pro Spalte |
| **Filtern** | Filterleiste, Menü mit Operatoren, Excel-/Checkbox-Filter | Filterleiste (Teilstring), globale Suche | **typisierte Operatoren, Filtermenü, Werteliste** |
| **Gruppieren** | mehrstufig, per Drag, auf-/zuklappbar | — | **fehlt ganz** |
| **Aggregate** | Summe, Mittel, Min, Max, Anzahl, eigene; pro Gruppe und gesamt | — | **fehlt ganz** |
| **Bearbeiten** | inline, Dialog, Zelle, Batch, Vorlagen, Validierung, CRUD | — (Playground-Skizze außerhalb) | **fehlt ganz** |
| **Spalten** | Breite, Reihenfolge, Formatierung, fixierte Spalten, Spaltenauswahl, mehrstufige Köpfe, Vorlagen, Spanning | Breite, Ein-/Ausblenden, Zellvorlagen | Reihenfolge, Formatierung, fixierte Spalten, mehrstufige Köpfe, Spaltenmenü, Spanning |
| **Zeilen** | Zeilen-/Detailvorlagen, Drag & Drop, Spanning, Höhe | Zeilenhöhe (virtualisiert) | Detailzeilen, Drag & Drop, Spanning |
| **Auswahl** | Zeile, Zelle, Spalte, Checkbox, bleibt über Operationen | Zeile (einfach, mehrfach, Bereich), bleibt über Sortierung/Paging | Zelle, Bereich über Zellen, Checkbox-Spalte, Auswahl bei gelöschten Zeilen bereinigen |
| **Export/Druck** | Excel, CSV, PDF, Drucken | — | **fehlt ganz** |
| **Virtualisierung** | Zeilen, Spalten, bedarfsweises Laden, Endlos-Scrollen | Zeilen (feste Höhe) | Spalten, Remote-Virtualisierung, Endlos-Scrollen |
| **Zwischenablage** | Kopieren | — | Kopieren, Einfügen |
| **Toolbar, Kontextmenü** | ja | Spaltenmenü (Komponente) | Toolbar, Kontextmenü |
| **Barrierefreiheit** | WAI-ARIA, Tastatur, RTL | WAI-ARIA-Grid, vollständige Tastatur, axe-sauber | RTL, Screenreader-Praxistest |
| **Lokalisierung** | Sprachen, Zahl-/Datums-/Währungsformat | englische Texte, teils per Prop | **Texte zentral übersetzbar, Formatierung** |
| **Responsive** | adaptives Layout, Touch | Touch-Ziele, Virtualisierung auf Android geprüft | adaptives Layout (Karten auf schmalen Bildschirmen) |
| **Zustand** | Persistenz | `GridState` mit serde | — |

## 3. Bewusst nicht Ziel

- **KI-Funktionen** (semantische Suche, Anomalieerkennung). Gehören in die App, nicht ins Grid; das
  `DataSource`-Trait reicht dafür.
- **Theme-Studio, fünf mitgelieferte Themes.** Wir stylen über die dx-components-Theme-Variablen;
  ein zweites Theme-System wäre Doppelarbeit.
- **ORM-Integrationen** (Entity Framework, Dapper). Das Fullstack-Beispiel zeigt das Muster; eine
  Bindung an ein bestimmtes ORM gehört in ein eigenes Crate der Nutzer.
- **Pivot, Tree Grid, Tabellenkalkulation mit Formeln.** Bei Syncfusion eigene Produkte, hier ebenso
  außerhalb des Umfangs.

## 4. Architekturentscheidungen vorab

Diese Punkte betreffen mehrere Phasen und werden **vor** Phase 7 entschieden und als ADR
festgehalten. Späteres Umbauen wäre teurer als alles, was sie kosten.

### A1 — Ein typisiertes Wertemodell pro Spalte

Heute hat eine Spalte getrennte Closures für Sortierung (`SortValue`) und Filter (`String`).
Operatoren („größer als", „zwischen", „vor Datum"), Aggregate, Formatierung, Export und Bearbeiten
brauchen alle **denselben typisierten Wert**. Vorschlag: eine Zugriffsfunktion `value(&T) -> CellValue`
mit den Arten Text, Ganzzahl, Kommazahl, Dezimal, Bool, Datum, Datum/Uhrzeit, Aufzählung, leer.
`sort_by_*` und `filter_by` bleiben als Kurzformen erhalten und werden intern darauf abgebildet.
Datum und Dezimal über optionale Features (`chrono` oder `time`, `rust_decimal`), damit der Core
ohne sie schlank bleibt.

### A2 — Die View wird eine Liste von Zeilenarten

`View::indices` kennt nur Datenzeilen. Gruppierung, Aggregatzeilen und Detailzeilen verlangen
`Vec<ViewRow>` mit `Data(index)`, `GroupHeader { level, key, count, expanded }`,
`GroupFooter { … }` und `Detail(index)`. Virtualisierung, Tastatur und `aria-rowindex` rechnen dann
über diese Liste. Das ist der größte Umbau im Plan; er kommt deshalb **vor** Gruppierung und
Detailzeilen, als eigener Schritt mit allen bisherigen Tests als Sicherheitsnetz.

ARIA: Mit Gruppen wird aus `role="grid"` ein `role="treegrid"` mit `aria-level`, `aria-expanded`,
`aria-setsize` und `aria-posinset` (WAI-ARIA Treegrid Pattern).

### A3 — Aus einer Registry-Komponente wird eine Familie

`data_grid` darf laut `CLAUDE.md` nicht über ~250 Zeilen wachsen, und ein SfGrid-Funktionsumfang
passt nicht in eine Datei. Vorschlag:

- `data_grid` bleibt der Kern (Tabelle, Suche, Paging).
- Zusatzkomponenten, die man bei Bedarf dazuholt: `data_grid_toolbar`, `data_grid_filter_menu`,
  `data_grid_editor` (Formular und Dialog), `data_grid_group_panel`, `data_grid_export`.
- Popover, Dialog, Kontextmenü, Checkbox und Select kommen **aus den offiziellen dx-components**
  über `componentDependencies` im Manifest, statt selbst gebaut zu werden. Muss in Phase 7 per Spike
  geprüft werden (Schema-Feld existiert laut `docs/VERIFICATION.md`, Verhalten nicht getestet).

### A4 — Schwere Abhängigkeiten in eigene, optionale Crates

Excel- und PDF-Export ziehen große Bibliotheken nach. Sie kommen in `datagrid-export` mit Features
(`csv` ohne Abhängigkeit, `xlsx` mit `rust_xlsxwriter`, `pdf` mit einer noch zu wählenden
Bibliothek). `datagrid-core` und `dioxus-datagrid` bleiben davon frei. Laut `CLAUDE.md` vor dem
Hinzufügen jeweils nachfragen.

### A5 — Alle Texte an einer Stelle

Heute stehen „Search", „Previous page", „Loading…" teils fest in den Primitives, teils als Props.
Vorschlag: eine `GridLocale`-Struktur mit allen Texten und Zahl-/Datumsformaten, per Context an die
Primitives, Englisch als Standard, Deutsch als zweite mitgelieferte Sprache zum Beweis. Bevor weitere
Texte dazukommen, also früh.

### A6 — `GridQuery` versioniert erweitern

Typisierte Filter, Gruppierung und Aggregate müssen auch zum Server. `GridQuery` bekommt neue
Felder mit `#[serde(default)]`, sodass alte Server neue Anfragen und neue Server alte Anfragen
verstehen. Das Fullstack-Beispiel wächst mit und bleibt der Nachweis der SQL-Übersetzung.

---

## 5. Phasen

Jede Phase endet mit einem Release. Seit ADR-0019 gilt: Bindet eine Phase die Registry-Komponenten
an neue Crate-APIs, wird vor dem Push veröffentlicht. Größen: **S** ≈ eine Sitzung, **M** ≈ zwei bis
drei, **L** ≈ vier und mehr.

### Phase 7: Fundament → 0.5.0 (M)

A1, A5 und der Spike zu A3.

- `CellValue` und `value(&T)` im Core, `sort_by_*`/`filter_by` darauf abgebildet, alle bestehenden
  Tests grün ohne Änderung an ihrer Erwartung.
- Formatierung pro Spalte: Zahl (Nachkommastellen, Tausendertrennung), Währung, Prozent, Datum;
  Ausrichtung (Zahlen rechtsbündig), Textumbruch oder Abschneiden mit Tooltip.
- `GridLocale` mit Englisch und Deutsch; alle Texte in Primitives und Komponente darüber.
- Spike: `componentDependencies` auf ein dx-components-Popover, Ergebnis in `VERIFICATION.md`.
- Nebenbei aus der Playground-Skizze: Auswahl wird bereinigt, wenn Zeilen aus den Daten
  verschwinden (heute bleibt „2 selected" nach dem Löschen stehen).

**Akzeptanz:** Property-Test, dass Sortierung über `CellValue` exakt der bisherigen entspricht;
Playground umschaltbar auf Deutsch; axe ohne Befund in beiden Sprachen.

### Phase 8: Filtern wie im SfGrid → 0.6.0 (M)

- Operatoren je Werteart: Text (enthält, beginnt mit, endet mit, gleich, leer), Zahl und Datum
  (=, ≠, <, ≤, >, ≥, zwischen), Bool, Aufzählung (ist eines von).
- Filtermenü am Spaltenkopf (Popover aus dx-components): Operator plus Wert, UND/ODER zweier
  Bedingungen.
- Werteliste im Stil von Excel: eindeutige Werte mit Anzahl, Suche darin, alle/keine; bei großen
  Daten begrenzt, bei Remote über eine neue `DataSource`-Methode `distinct_values`.
- Filterleiste bleibt, bekommt Operator-Kurzformen (`>100`, `=Berlin`).
- `GridQuery` um typisierte Filter erweitert (A6), SQL-Übersetzung im Fullstack-Beispiel.

**Akzeptanz:** Property-Tests für jeden Operator gegen eine naive Referenz; Fullstack-Test, dass SQL
und lokale Filterung für alle Operatoren gleich antworten.

### Phase 9: Bearbeiten → 0.7.0 (L)

Nach dem WAI-ARIA-Grid-Muster: Navigationsmodus und Bearbeitungsmodus.

- **Zelle bearbeiten:** `Enter` oder `F2` startet, `Escape` bricht ab, `Enter`/`Tab` übernimmt und
  geht weiter. Editor je Werteart (Text, Zahl, Datum, Bool, Auswahl), eigene Editoren per Vorlage.
- **Zeile bearbeiten** (inline) und **Dialog/Formular** (Registry-Komponente `data_grid_editor` mit
  dx-components-Dialog).
- **Batch:** Änderungen sammeln, geänderte Zellen markieren, gesammelt speichern oder verwerfen.
- **Anlegen und Löschen**, mit Bestätigung und bereinigter Auswahl.
- **Validierung** pro Spalte und pro Zeile, Fehler an der Zelle und per `aria-invalid` und
  `aria-describedby`.
- Speichern über Callbacks (`on_save`, `on_create`, `on_delete`) — lokal und remote gleich; für
  remote optimistische Anzeige mit Rücknahme bei Fehler.

**Akzeptanz:** Playwright für jede Bearbeitungsart nur mit Tastatur; axe im Bearbeitungsmodus;
Test, dass ein fehlgeschlagener Remote-Speicherversuch den Ursprungswert wiederherstellt.

### Phase 10: Gruppieren und Aggregate → 0.8.0 (L)

Zuerst A2 als eigener Schritt, dann darauf aufbauend:

- Gruppieren nach einer oder mehreren Spalten, per Spaltenmenü und per Drag in eine Gruppenleiste
  (`data_grid_group_panel`); Gruppen auf-/zuklappen, alle auf einmal.
- Aggregate: Summe, Mittelwert, Min, Max, Anzahl, eigene Funktion; als Fußzeile des Grids, als
  Gruppenfuß und im Gruppenkopf.
- Funktioniert mit Paging (Gruppen über Seitengrenzen), Virtualisierung und Remote
  (`GridQuery` mit Gruppierung; Server liefert Gruppenzeilen und Aggregate).
- ARIA: `treegrid`, siehe A2.

**Akzeptanz:** Property-Test, dass Gruppen die gefilterten Zeilen vollständig und überlappungsfrei
abdecken und Aggregate einer naiven Berechnung entsprechen; Tastatur klappt Gruppen mit `←`/`→` wie
im Treegrid-Muster; 100.000 Zeilen gruppiert und virtualisiert flüssig.

### Phase 11: Spalten-Layout → 0.9.0 (M)

- Reihenfolge per Drag im Kopf und per Tastatur (`Alt+Shift+←/→`), gespeichert in `GridState`.
- Fixierte Spalten links und rechts (`position: sticky` im Subgrid; Spike zur Verträglichkeit mit
  Virtualisierung und Resize).
- Mehrstufige Spaltenköpfe (Spaltengruppen) mit korrekten `aria-colindex`/`aria-colspan`.
- Spaltenmenü am Kopf: sortieren, filtern, gruppieren, fixieren, ausblenden, automatische Breite.
- Automatische Breite nach Inhalt (Messung über `onresize`/`get_client_rect`, keine `web-sys`).
- Zellen über Spalten verbinden (Column Spanning), soweit ARIA es sauber erlaubt.

### Phase 12: Zeilen, Auswahl, Zwischenablage → 0.10.0 (M)

- Detailzeilen (Master-Detail) mit beliebigem Inhalt, auch verschachtelten Grids; ARIA über
  `aria-expanded` und `aria-controls`.
- Zeilen per Drag umsortieren (Callback, die Daten gehören der App).
- Checkbox-Spalte mit „alle auswählen" (dreistufig).
- Zellauswahl und rechteckige Bereiche (`Shift+Pfeil` im Zellmodus).
- Kopieren als TSV, das Excel versteht; Einfügen in bearbeitbare Zellen. Zugriff auf die
  Zwischenablage zuerst per Spike prüfen (Dioxus bietet keine eigene API; `document::eval` ist die
  Alternative, braucht dafür eine ADR).

### Phase 13: Export und Druck → 0.11.0 (M)

- `datagrid-export` (A4): CSV ohne Abhängigkeit; Excel mit Formatierung, Gruppen und Aggregaten;
  PDF.
- Export von „aktueller Ansicht" oder „alle gefilterten Zeilen", remote über die `DataSource`.
- Drucken: eigenes Druck-Stylesheet, ganze Ansicht oder aktuelle Seite.
- Download ohne `web-sys`: Web über eine Daten-URL oder `document::eval`, Desktop über einen
  Speicherdialog — Spike, ADR.

### Phase 14: Bedienoberfläche rundherum → 0.12.0 (M)

- Toolbar-Komponente mit Standardaktionen (Hinzufügen, Bearbeiten, Löschen, Export, Suche,
  Spaltenauswahl) und eigenen Einträgen.
- Kontextmenü auf Zeilen und Köpfen (dx-components `context_menu`), auch per Tastatur (Umschalt+F10).
- Adaptives Layout: auf schmalen Bildschirmen Zeilen als Karten, Filter und Bearbeiten als Sheet.
- RTL: Layout gespiegelt, Pfeiltasten gespiegelt, getestet.

### Phase 15: Große Datenmengen, Live-Daten → 0.13.0 (L)

- Remote-Virtualisierung: Blöcke nachladen beim Scrollen, statt zu blättern; Endlos-Scrollen.
- Spaltenvirtualisierung für breite Tabellen (Spike: Verträglichkeit mit CSS-Subgrid).
- Variable Zeilenhöhen (heute Nicht-Ziel): gemessen, mit geschätzter Höhe für nicht gerenderte
  Zeilen.
- Live-Updates: Zeilen einfügen, ändern, entfernen ohne Neuladen; Fokus und Auswahl bleiben stabil.

### Danach: 1.0

API-Durchsicht, Deprecations aus den 0.x-Versionen entfernen, Screenreader-Praxistest mit NVDA,
JAWS und VoiceOver, Doku-Seite mit einer Demo pro Funktion, Benchmarks gegen die Zielwerte.

---

## 6. Reihenfolge und Abhängigkeiten

```
Phase 7 Fundament ─┬─> 8 Filtern ──────────────┐
  (A1 Werte,       ├─> 9 Bearbeiten ───────────┼─> 13 Export ─> 14 Oberfläche ─> 15 Große Daten ─> 1.0
   A5 Texte,       ├─> 10 Gruppen (A2 zuerst) ─┤
   A3 Spike)       └─> 11 Spalten ─> 12 Zeilen ┘
```

- Phase 7 zuerst, weil Operatoren, Aggregate, Formatierung, Editoren und Export alle das
  Wertemodell brauchen, und weil jeder später hinzugefügte Text sonst zweimal angefasst wird.
- Bearbeiten (9) vor Gruppieren (10): in Business-Anwendungen das meistgefragte fehlende Feature,
  und es braucht A2 nicht.
- Export (13) nach Gruppen und Aggregaten, damit er sie gleich mit exportiert.
- Große Daten (15) zuletzt: baut auf der erweiterten View (A2) und auf Live-Updates aus dem
  Bearbeiten auf.

## 7. Risiken

- **A2 (View als Zeilenarten)** berührt Virtualisierung, Tastatur, ARIA und Remote gleichzeitig.
  Mitigation: eigener Schritt ohne neue Funktion, alle Tests grün vor der ersten Gruppe.
- **Registry-Familie (A3):** ob `componentDependencies` verlässlich funktioniert, ist ungeprüft.
  Fällt der Spike negativ aus, bauen wir Popover und Dialog als eigene, schlichte Primitives.
- **Zwischenablage und Download** haben in Dioxus 0.7 keine plattformneutrale API. Möglicher
  Ausweg ist `document::eval`, was die Architekturregeln bisher ausschließen; das braucht eine
  bewusste, begründete Ausnahme.
- **Dioxus 0.8** steht an (0.8.0-alpha existiert). Ein Umstieg mitten in diesem Plan kostet
  vermutlich eine Phase. Vorschlag: nach Phase 9 prüfen, ob 0.8 stabil ist.
- **Umfang:** grob 20–30 Sitzungen bis 1.0. Die Phasen sind so geschnitten, dass jede für sich
  veröffentlichbar und nützlich ist; man kann nach jeder anhalten.

## 8. Entscheidungen

Mit dem Nutzer abgestimmt am 2026-09-18:

1. **Reihenfolge wie in Abschnitt 5:** Bearbeiten (9) vor Gruppieren (10).
2. **Datum über `chrono`** (optionales Feature im Core). Dezimal über `rust_decimal` bleibt Vorschlag
   für Phase 7.
3. **Registry-Familie (A3):** wie in Abschnitt 9 — `data_grid` bleibt der Einstieg und wird über
   Context und `children` erweiterbar.
4. **`document::eval`** ist erlaubt für Zwischenablage und Download, **wenn es keinen anderen Weg
   gibt** — nachgewiesen per Spike und jeweils als ADR begründet.
5. **Deutsch** ist die zweite mitgelieferte Sprache.

## 9. Was aus `data_grid` wird (A3)

`data_grid` bleibt der Einstieg: ein `dx components add data_grid`, und man hat eine vollständige
Tabelle mit allem, was sie heute kann. Neu ist, dass sie **erweiterbar** wird, statt selbst zu wachsen:

- `DataGrid` legt ihren `GridHandle` in den Context und nimmt `children` an.
- Zusatzkomponenten werden als Kinder eingesetzt und holen sich den Handle von dort:

  ```rust
  DataGrid { data: users, columns,
      DataGridToolbar { on_add: add_user }
      DataGridFilterMenu {}
      DataGridEditor { on_save: save_user }
  }
  ```

- Wo eine Erweiterung *in* die Tabelle muss — ein Filter-Knopf in jeder Kopfzelle, eine
  Editor-Zelle —, registriert sie sich über den Context an einem Erweiterungspunkt, den `data_grid`
  anbietet. `data_grid` muss die Zusatzkomponenten dafür nicht kennen; wer sie nicht installiert,
  kompiliert nichts davon mit.
- Jede Zusatzkomponente nennt `data_grid` in `componentDependencies`, sodass
  `dx components add data_grid_editor` den Kern mitinstalliert.
- Für volle Kontrolle bleibt der Weg über `use_grid` und die Primitives, wie in `examples/`.

Die Logik aller Zusatzkomponenten liegt weiterhin in den Crates; die Registry-Dateien bleiben dünn.
