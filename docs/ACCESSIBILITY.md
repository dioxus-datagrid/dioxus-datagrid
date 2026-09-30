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

Die Kopfzeilen sind Teil des Gitters, nicht davor. Ohne Spaltengruppen ist `CellFocus::row == 0`
die Kopfzeile und `row == n` die *n*-te Zeile der aktuellen Seite; mit Gruppen kommt je Ebene eine
Kopfzeile davor (`GridHandle::header_rows`). Damit stimmt die Fokus-Koordinate direkt mit
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
| `Shift`+Pfeil (bei Zellauswahl `Range`) | das Rechteck vom Anker aus wachsen lassen — dann **nicht** die Zeilenauswahl |
| `Ctrl+C` / `Cmd+C` | die Auswahl als tabgetrennten Text in die Zwischenablage |
| `Ctrl+V` / `Cmd+V` | tabgetrennten Text in bearbeitbare Zellen einfügen, ab der Ecke der Auswahl |
| `Space` (auf der Kopfzelle einer Checkbox-Spalte) | alle Zeilen der Seite auswählen oder freigeben |
| `Enter` (auf einer Zelle der Expander-Spalte) | die Detailzeile dieser Zeile öffnen oder schließen |

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
- Die Spaltenauswahl der Registry-Komponente (`column_picker`) liegt **außerhalb** von
  `role="grid"` und besteht aus normalen Checkboxen mit Label. Nicht zu verwechseln mit dem
  Spaltenmenü am Kopf, das weiter unten steht.

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

## Filtermenü

`GridFilterMenu` ist ein Knopf mit nicht-modalem Dialog (Disclosure-Muster mit `role="dialog"`).

- **Der Knopf** hat einen sprechenden Namen aus der Locale („Filter options for Department"),
  `aria-haspopup="dialog"`, `aria-expanded` und, solange offen, `aria-controls` auf das Panel.
  Das sichtbare „▾" ist `aria-hidden`.
- **Beim Öffnen** springt der Fokus auf die erste Operator-Auswahl. **`Escape`** und ein Klick
  daneben schließen ohne Übernahme; **„Anwenden" und „Zurücksetzen"** schließen mit. In jedem Fall
  kehrt der Fokus auf den Knopf zurück.
- **Nicht modal:** `Tab` darf das Panel verlassen, der Rest der Seite bleibt bedienbar. Eine
  Fokusfalle wäre für ein kleines Filterpanel mehr Hindernis als Hilfe.
- **Bedienelemente sind native Formularelemente:** `select` für Operatoren, `input` passend zum
  Werttyp (`type="date"`, `datetime-local`, Text mit `inputmode="decimal"`), Radios für und/oder,
  Checkboxen für die Werteliste. Jedes hat einen zugänglichen Namen aus der Locale. Die
  Umschaltung „Bedingung / Werte" sind Knöpfe mit `aria-pressed`.
- **Werteliste:** eine Liste von beschrifteten Checkboxen, der Name enthält Wert und Anzahl
  („engineering (3)"). Während sie lädt, steht ein `role="status"` darin.
- **Gefilterte Spalten** tragen `data-filtered="true"` am Kopf und am Menü; die Komponente zeigt es
  zusätzlich zur Farbe mit einer Unterstreichung.

## Bearbeiten

Nach dem WAI-ARIA-Grid-Muster: **Navigationsmodus** mit Pfeiltasten wie bisher, **Bearbeitungsmodus**
in der Zelle.

- **Einstieg:** `Enter` oder `F2` auf einer Zelle, oder Doppelklick. Der Editor ist ein natives
  Formularelement und bekommt den DOM-Fokus; die Gitterposition bleibt auf seiner Zelle.
- **Im Editor** gehören alle Tasten dem Editor: Das Grid wertet während der Bearbeitung keine
  Tasten aus (Leertaste wählt keine Zeile, Pfeile bewegen den Cursor im Text). `Enter` übernimmt,
  `Escape` bricht ab, `Tab` übernimmt und bearbeitet die nächste bearbeitbare Zelle — bei
  Zeilenbearbeitung wechselt `Tab` zwischen den Editoren der Zeile und kehrt am Ende zum ersten
  zurück. Danach steht der Fokus wieder auf einer Zelle, und die Pfeiltasten gelten wieder.
- **Name:** Ein Editor in der Zelle heißt wie seine Spalte (`aria-label`). Im Formular benennt ein
  `<label for>` das Feld.
- **Werteart:** Text- und Zahlfelder (`inputmode="decimal"`), `type="date"` und
  `datetime-local`, Checkbox für Wahrheitswerte, `select` für Spalten mit Auswahlliste.
- **Fehler:** Ein abgelehnter Wert setzt `aria-invalid="true"` und `aria-describedby` auf die
  Meldung, die mit `role="alert"` sofort angesagt wird. Der Editor bleibt offen und behält den
  Fokus. Fehler, die die ganze Zeile betreffen, stehen im Formular unter den Feldern (`role="alert"`)
  oder, bei Zeilenbearbeitung, in `GridEditStatus`.
- **Nur lesbar:** In einem bearbeitbaren Grid tragen Zellen ohne Setter `aria-readonly="true"`.
- **Formular (`GridEditDialog`):** modal, `role="dialog"`, `aria-modal="true"`, benannt über seine
  Überschrift. Der Fokus startet im ersten Feld und bleibt im Dialog: Fokuswächter an beiden Enden
  schicken `Tab` und `Shift+Tab` zum jeweils anderen Ende. `Escape` schließt, danach steht der Fokus
  wieder im Grid.
- **Löschen (`GridDeleteConfirm`):** `role="alertdialog"`, modal, benannt über die Frage („Diese
  Zeile löschen?"). Der Fokus startet auf **„Behalten"**, der Wahl, die nichts verliert; `Escape`
  behält die Zeilen.
- **Status (`GridEditStatus`):** Live-Region für „Wird gespeichert …", „Gespeichert", den Grund
  eines Fehlschlags und die Zahl ungespeicherter Änderungen im Batch. Der Container steht immer im
  DOM.
- **Markierungen** (`data-changed`, `data-deleted`, `data-saving`) sind nur optisch; was sie
  bedeuten, sagt die Status-Region als Zahl an. Die Komponente zeigt geänderte Zellen mit einem
  Balken, gelöschte Zeilen durchgestrichen, speichernde kursiv — nie nur über Farbe.

## Gruppen und Aggregate

Nach dem WAI-ARIA-**Treegrid**-Muster, sobald nach einer Spalte gruppiert ist; ohne Gruppen bleibt
es `role="grid"`.

- **Gruppenkopf:** eine Zeile mit genau einer Zelle über alle Spalten (`aria-colspan`). Die Zeile
  trägt `aria-level` (1 für die äußerste Gruppe), `aria-expanded`, `aria-posinset` und
  `aria-setsize` — Position und Zahl der Geschwistergruppen, auch wenn Paging oder
  Virtualisierung nicht alle im DOM halten. Die Zelle nennt Spalte, Wert und Zeilenzahl
  („Department: sales, 2 rows"); eine zugeklappte Gruppe nennt dort auch ihre Aggregate.
- **Datenzeilen und Gruppenfüße** liegen eine Ebene tiefer (`aria-level`). `aria-rowindex` zählt
  Köpfe und Füße mit, `aria-rowcount` ebenso — über alle Seiten.
- **Tastatur auf einem Gruppenkopf:** `ArrowRight` klappt auf, `ArrowLeft` klappt zu; auf einer
  schon zugeklappten Gruppe springt `ArrowLeft` zum Kopf der umgebenden Gruppe. `Enter` und
  `Space` schalten um. `ArrowUp`/`ArrowDown` laufen über Köpfe, Zeilen und Füße gleichermaßen; die
  Spalte bleibt erhalten, sodass es unter einem Kopf in derselben Spalte weitergeht. Auf
  Datenzeilen bleiben `ArrowLeft`/`ArrowRight` Zellnavigation.
- **Summenzeile (`GridFooter`):** eine eigene Zeilengruppe unter dem Körper, letzte Zeile der
  Tastaturnavigation (`Ctrl+End`) und letzte bei `aria-rowindex`. Die erste Zelle ohne Aggregat
  heißt „Totals"/„Gesamt". Aggregate stehen als Name und Wert je Zelle; bei fester Zeilenhöhe
  trägt die Zelle den vollen Text zusätzlich im `title`.
- **Gruppenleiste:** `role="group"`, benannt „Grouping"/„Gruppierung". Ziehen ist nie der einzige
  Weg: Eine Auswahlliste gruppiert nach einer weiteren Spalte, je gruppierter Spalte gibt es Knöpfe
  zum Vorziehen und Entfernen (mit Namen wie „Stop grouping by Department"), dazu
  „Expand all"/„Collapse all".

## Spaltenreihenfolge, fixierte Spalten und mehrstufige Köpfe

- **Reihenfolge ändern:** Ein Kopf mit `reorderable` ist `draggable` und nennt in
  `aria-keyshortcuts` zusätzlich `Alt+Shift+ArrowLeft` und `Alt+Shift+ArrowRight`. Die Tasten
  schieben die Spalte um einen Platz und nehmen den Fokus mit, am Rand passiert nichts. Ziehen ist
  damit nie der einzige Weg. `Alt` ohne `Shift` bleibt die Breitenänderung.
- **Fixierte Spalten:** tragen `data-pinned="start"` bzw. `"end"` an Kopf- und Körperzelle und
  werden als Block an ihrer Kante gelegt. Für Hilfstechnologie ändert sich nichts außer der
  Reihenfolge: `aria-colindex` zählt die Spalten so, wie sie stehen, und die Pfeiltasten laufen in
  derselben Reihenfolge.
- **Mehrstufige Köpfe:** je Gruppenebene eine weitere Zeile mit `role="row"` über den
  Spaltenköpfen, von außen nach innen. Eine Gruppenzelle ist ein `columnheader` mit `aria-colspan`
  über die Spalten, die sie benennt, und `aria-colindex` der ersten davon. Über einer Spalte ohne
  Gruppe steht eine leere Zelle, damit jede Kopfzeile jede Spalte abdeckt. `aria-rowcount` und
  `aria-rowindex` zählen alle Kopfzeilen mit, die erste Datenzeile ist also `header_rows + 1`.
- **Tastatur im mehrstufigen Kopf:** `ArrowUp` von einer Spalte führt auf die Gruppe darüber,
  `ArrowDown` zurück auf die erste Spalte darunter. Eine Gruppenzelle hält den Roving-`tabindex`
  für jede ihrer Spalten und ist ein einziger Stopp: `ArrowRight` geht zur nächsten Gruppe, nicht
  zur nächsten Spalte derselben Gruppe. Der Tab-Stopp liegt anfangs auf den Spaltenköpfen, nicht
  auf einer Gruppe: eine Gruppe ist eine Beschriftung, kein Bedienelement.

## Zellauswahl

Zellen und Zeilen werden getrennt ausgewählt; ein Gitter kann beides, eines oder keines von beidem
anbieten (`CellSelectionMode::{None, Single, Range}`).

- **Jede Datenzelle sagt, wo sie steht:** `aria-selected="true"` oder `"false"`, solange Zellauswahl
  eingeschaltet ist — und gar nichts, solange sie aus ist. Zusätzlich `data-cell-selected` fürs
  Styling. Das Gitter meldet `aria-multiselectable="true"`, sobald **eine** der beiden Auswahlen
  mehr als einen Eintrag zulässt.
- **Das Rechteck hat einen Anker.** `Shift`+Pfeil und `Shift`+Klick lassen dasselbe Rechteck von
  diesem Anker aus wachsen und schrumpfen, statt ein neues zu beginnen. Ein Pfeil **ohne** `Shift`
  fängt an der erreichten Zelle neu an.
- **Wo Zellen und Zeilen dieselbe Taste wollen, gewinnen die Zellen.** Bei `Range` erweitert
  `Shift`+Pfeil das Rechteck und nicht die Zeilenauswahl; `Space` bleibt der Zeile. Ein Gitter, das
  Rechtecke auswählt, wird als Tabellenblatt gelesen.
- **Kopfzeilen sind nicht auswählbar.** Ein Kopf ist keine Zelle, die man kopieren würde; eine
  Auswahl dorthin wird abgelehnt, nicht in den Körper verschoben.
- **Das Rechteck ist ein Ort, kein Inhalt.** Beide Ecken sind Ansichtskoordinaten wie der Fokus.
  Sortieren, Filtern oder Blättern lässt es deshalb stehen, wo es ist — was hervorgehoben ist, ist
  ausgewählt, und das bleibt wahr. Zeilenauswahl dagegen hängt am Schlüssel und übersteht all das.

## Kopieren

`Ctrl+C` (auf macOS `Cmd+C`) legt die Auswahl als tabgetrennten Text ab — das Format, das
Tabellenkalkulationen als Block von Zellen lesen.

- **Was kopiert wird, in dieser Reihenfolge:** das ausgewählte Rechteck aus Zellen; sonst die
  ausgewählten Zeilen mit allen sichtbaren Spalten, in der Reihenfolge, in der sie **stehen**;
  sonst die fokussierte Zelle. Auf einer Kopfzeile passiert nichts.
- **Kopiert wird, was zu sehen ist:** der Wert der Spalte, formatiert durch die Sprache des Gitters
  — „1.234,50 €" auf Deutsch, „€1,234.50" auf Englisch. Eine Spalte ohne Wert, die nur mit `cell`
  etwas zeichnet, kopiert eine leere Zelle: gerendertes Markup ist kein Text.
- **Spaltenüberschriften sind nicht dabei.** Ein Einfügen in ein Blatt, das schon Überschriften hat,
  bekäme sonst eine Zeile geschenkt.
- **Keine Berechtigungsfrage.** Schlägt der moderne Weg fehl, nimmt das Gitter den alten; gelingt
  auch der nicht, passiert nichts. Ein Gitter, das nach Rechten fragt, um eine Zeile zu kopieren,
  wäre schlimmer als ein gelegentlich stilles Nein (ADR-0033).

## Detailzeilen

Eine Zeile kann eine zweite unter sich öffnen, die beliebigen Inhalt trägt — auch ein eigenes
Gitter. Die Knöpfe dafür stehen in einer Spalte mit `.expander()`; was zu sehen ist, sagt die
Anwendung über `set_detail_rows`.

- **Die Detailzeile ist eine Zeile.** `role="row"` mit einer Zelle über alle Spalten
  (`aria-colspan`), mit eigener `aria-rowindex`, und `aria-rowcount` zählt sie mit. So bleibt die
  Zählung stimmig, statt dass die Zeilen darunter falsche Nummern tragen.
- **`aria-expanded` sitzt am Knopf**, nicht an der Zeile: an einem `role="row"` ist es nur in einem
  `treegrid` zulässig, wo die Kinder einer Zeile wieder Zeilen sind. Solange das Detail da ist,
  zeigt `aria-controls` des Knopfes auf dessen `id`; ist es zu, zeigt es auf nichts — eine Zusage
  auf ein Element, das es nicht gibt, wäre keine.
- **Kein zusätzlicher Tab-Stopp.** Der Knopf trägt `tabindex="-1"`; die Zelle um ihn trägt die
  Tastatur, wo `Enter` öffnet und schließt. Ein Klick irgendwo in die Zelle tut dasselbe.
- **Die Pfeile gehen über die Detailzeile hinweg, nicht in ihre Spalten** — sie ist eine Zelle,
  wie die Kopfzeile einer Gruppe. Was die Anwendung hineinrendert, ist danach Teil der Seite und
  in der normalen Reihenfolge erreichbar.
- **Nur wo es etwas zu zeigen gibt.** Zeilen ohne Detail bekommen keinen Knopf und sagen nichts
  über einen Zustand, den sie nicht haben.
- **Eine Expander-Spalte ohne eigene Beschriftung zeigt „Details"** im Kopf. Ein Spaltenkopf ohne
  sichtbaren Text ist einer, den niemand einordnen kann.
- **In einem virtualisierten oder entfernten Gitter gibt es sie nicht** — dort würden die Zeilen
  verrutschen (ADR-0036). Die Spalte zeichnet dann gar keine Knöpfe.

## Checkbox-Spalte

Eine Spalte mit `.checkbox()` zeigt in jeder Zeile eine Checkbox und darüber eine, die alle
umschaltet. Gezeichnet werden sie vom Gitter, weil nur das Gitter weiß, was ausgewählt ist.

- **Keine zusätzlichen Tab-Stopps.** Beide Boxen tragen `tabindex="-1"`; das Gitter bleibt ein
  Stopp. Die Zelle um eine Box trägt die Tastatur: `Space` auf einer Zeile schaltet sie um, `Space`
  auf der Kopfzelle alle. Die Kopfzelle sortiert dann nicht — es gibt nichts zu sortieren.
- **Jede Box hat einen Namen:** „Zeile auswählen" beziehungsweise „Alle Zeilen auswählen", aus der
  Sprache des Gitters. Eine Kopfzelle ohne eigene Beschriftung leiht sich den Namen ihrer Box, damit
  kein Spaltenkopf namenlos bleibt.
- **Der dritte Zustand ist echt.** Sind einige, aber nicht alle Zeilen ausgewählt, steht die
  Kopf-Checkbox auf `indeterminate` — die Eigenschaft, die HTML-AAM als „mixed" weitergibt, nicht
  ein Attribut, das Browser ignorieren würden (ADR-0035, VERIFICATION §16). Zum Gestalten trägt sie
  zusätzlich `data-select-all="none|partial|all"`.
- **„Alle" heißt: alle, die zu sehen sind** — bei Blättern die aktuelle Seite. Eine Auswahl auf
  einer anderen Seite bleibt unberührt und wird nicht mitgezählt.
- **Ein Klick schaltet um**, `Shift`+Klick reicht vom Anker bis hierher. Wo Zeilen gar nicht
  ausgewählt werden können, zeichnet die Spalte keine Boxen; bei `Single` gibt es keine
  Kopf-Checkbox.
- **Der Zeilenzustand steht ohnehin an der Zeile.** `aria-selected` am `role="row"` bleibt die
  maßgebliche Aussage; die Box ist das Bedienelement dazu.

## Einfügen

`Ctrl+V` (auf macOS `Cmd+V`) schreibt tabgetrennten Text in bearbeitbare Zellen — die Gegenrichtung
zum Kopieren, mit denselben Regeln für das Format.

- **Wo es landet:** an der oberen linken Ecke des ausgewählten Rechtecks, sonst an der fokussierten
  Zelle; von dort nach rechts und unten. Eine einzelne eingefügte Zelle füllt das ganze ausgewählte
  Rechteck, wie in einer Tabellenkalkulation.
- **Es wächst nichts.** Was über die letzte Spalte oder die letzte Zeile hinausragt, fällt weg; ein
  Einfügen legt keine Zeilen an.
- **Eine Spalte, die nicht bearbeitet werden kann, behält ihren Wert** — hält aber ihren Platz, damit
  der Rest der Zeile unter den Spalten bleibt, aus denen er kopiert wurde.
- **Eine Zeile ganz oder nicht.** Lehnt eine Spalte oder die Anwendung einen Wert ab, bleibt die
  ganze Zeile, wie sie war; die Statuszeile sagt, wie viele Zeilen das betraf, und der eingefügte
  Block bleibt markiert, damit man nachsehen kann.
- **Ein offener Editor behält sein eigenes Einfügen.** In einem Eingabefeld fügt der Browser ein, wie
  er es überall tut; das Gitter hält sich heraus.
- **Auf einer Kopfzeile passiert nichts**, und in einem Gitter ohne Bearbeitung passiert nichts.

Einfügen kann nur aus dem echten `paste`-Ereignis kommen: die Zwischenablage zu *lesen* ist überall
verboten (ADR-0033, VERIFICATION §15). Ein Knopf „Einfügen" ist deshalb nicht möglich, und es gibt
ihn bewusst nicht.

## Zellen über mehrere Spalten

Eine Spalte kann pro Zeile sagen, über wie viele Spalten ihre Zelle läuft (`Column::span`).

- **Die Zeile bleibt so breit wie der Kopf.** Die breite Zelle trägt `aria-colspan`, die Spalten,
  die sie verdeckt, rendern keine eigene Zelle. `aria-colindex` zählt weiter in Spalten: nach
  einer Zelle über drei Spalten kommt `aria-colindex="4"`.
- **Eine Zelle ist ein Stopp.** `ArrowRight` aus einer breiten Zelle führt auf die Spalte dahinter,
  nicht in die verdeckten; `ArrowDown` auf eine verdeckte Spalte landet auf der Zelle, die sie
  abdeckt. `Home`/`End` ebenso. Der Roving-`tabindex` liegt damit immer auf genau einer Zelle.
- **Fixierte Spalten begrenzen sie.** Eine Zelle läuft nie über die Kante ihres fixierten Blocks
  hinaus, sonst müsste sie zugleich kleben und mitscrollen.

## Spaltenmenü

Eine Zusatzkomponente (`data_grid_column_menu`) setzt in jede Kopfzelle einen Knopf, der ein Menü
nach dem WAI-ARIA-**Menu**-Muster öffnet: sortieren, gruppieren, fixieren, ausblenden, Breite
vergessen.

- **Der Knopf ist kein Tab-Stopp** (`tabindex="-1"`). Das Gitter bleibt ein einziger Tab-Stopp;
  geöffnet wird mit **`Alt+↓`** auf dem fokussierten Spaltenkopf. Der Knopf meldet
  `aria-haspopup="menu"`, `aria-expanded` und über `aria-controls` das Panel.
- **Das Panel** ist `role="menu"`, benannt wie der Knopf. Einträge sind `menuitem`; wo sie eine
  Auswahl zwischen Zuständen sind — die Sortierrichtung, die Kante — sind es `menuitemradio` mit
  `aria-checked`, sodass Hilfstechnologie den geltenden Zustand nennt.
- **Tastatur im Menü:** `↓` und `↑` gehen weiter und laufen um, `Home`/`End` an die Enden, `Enter`
  und `Space` wählen, `Escape` schließt. Nach dem Öffnen liegt der Fokus auf dem ersten Eintrag,
  nach dem Schließen wieder auf dem **Spaltenkopf** — nicht auf dem Knopf, der keiner
  Tab-Reihenfolge angehört.
- **Die Kopfzelle benennt sich selbst** (`aria-label` aus dem Spaltenlabel), damit der Knopftext
  nicht in den Namen der Spalte gerät.
- **Angeboten wird nur, was etwas tut:** keine Sortierung ohne Wert, kein „Sortierung aufheben"
  ohne Sortierung, kein „Fixierung aufheben" ohne Fixierung, kein Ausblenden der letzten Spalte.

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
- **Filtermenü:** `tests/e2e/filter.spec.ts` — Fokus beim Öffnen und nach dem Schließen,
  `Escape`, Klick daneben, Werteliste, Deutsch, und axe bei geöffnetem Menü in beiden Ansichten.
- **Bearbeiten:** `tests/e2e/editing.spec.ts` — jede Bearbeitungsart nur mit der Tastatur (Zelle,
  Zeile, Formular, Batch, Anlegen, Löschen), Fokus nach Übernehmen und Abbrechen, Fokusfalle im
  Formular, und axe mit offenem Editor, mit Fehlermeldung, mit offenem Formular und offener
  Löschabfrage. Markup in `crates/dioxus-datagrid/tests/editing.rs`.
- **Gruppen:** `tests/e2e/grouping.spec.ts` — Gruppieren über die Liste und per Ziehen,
  `ArrowLeft`/`ArrowRight`/`Enter`/`Space` auf Köpfen, Gruppen über Seitengrenzen, Füße und
  Summenzeile per `Ctrl+End`, axe gruppiert mit Summen im hellen und dunklen Theme, 100.000 Zeilen
  gruppiert und virtualisiert. Markup in `crates/dioxus-datagrid/tests/grouping.rs`.
- **Spaltenreihenfolge:** `tests/e2e/column-order.spec.ts` — Ziehen eines Kopfes auf einen
  anderen, `Alt+Shift`+Pfeiltasten mit dem Fokus, Verhalten am Rand, Zellen folgen den Köpfen,
  Reihenfolge übersteht Neuladen und Ausblenden, axe. Markup in
  `crates/dioxus-datagrid/tests/column_order.rs`, Kern in
  `crates/datagrid-core/tests/column_order.rs`.
- **Fixierte Spalten:** `tests/e2e/pinned.spec.ts` — Kante und Abstand, Lage im Block, Kopf und
  Zelle bleiben pixelgenau an der Kante, während die Nachbarspalte dahinter durchläuft,
  virtualisierte Zeilen, axe. Markup in `crates/dioxus-datagrid/tests/pinned.rs`.
- **Mehrstufige Köpfe:** `tests/e2e/header-groups.spec.ts` — Gruppen über den Spalten, die sie
  benennen, Zeilennummern über beide Kopfzeilen, Ausblenden und Umsortieren ordnen den Kopf neu,
  `ArrowUp`/`ArrowDown` zwischen Gruppe und Spalte, eine Gruppe als ein Stopp, ein Tab-Stopp, axe.
  Markup in `crates/dioxus-datagrid/tests/header_groups.rs`, Aufteilung im Kern in
  `crates/datagrid-core/tests/header.rs`.
- **Kopieren:** `tests/e2e/copy.spec.ts` — Rechteck, einzelne Zelle, ausgewählte Zeilen, fokussierte
  Zelle, Kopf kopiert nichts, und der alte Weg trägt, wenn der moderne abgelehnt wird. Beobachtet
  wird, was das Gitter der Zwischenablage übergibt; sie zurückzulesen ist überall verboten
  (VERIFICATION §15). Textbildung in `crates/dioxus-datagrid/tests/copy.rs`, das Format in
  `crates/datagrid-core/tests/tsv.rs`.
- **Detailzeilen:** `tests/e2e/detail-rows.spec.ts` — öffnen und schließen, die Detailzeile steht
  unter ihrer Zeile und trägt die nächste Zeilennummer, der Knopf zeigt auf sie, eine Zeile ohne
  Detail hat keinen Knopf, `Enter` auf der Zelle, die Pfeile gehen über sie hinweg, der Inhalt ist
  Teil der Seite, offen bleibt offen beim Sortieren, eine Seite trägt sie mit, virtualisiert gibt
  es sie nicht, axe. Markup in `crates/dioxus-datagrid/tests/detail.rs`, die Ansicht selbst in
  `crates/datagrid-core/tests/detail.rs`.
- **Checkbox-Spalte:** `tests/e2e/checkbox.spec.ts` — Klick schaltet um, `Shift`+Klick reicht,
  Kopf-Checkbox in allen drei Zuständen (der dritte als DOM-Eigenschaft geprüft), `Space` auf Kopf-
  und Zeilenzelle, Blättern lässt die andere Seite in Ruhe, `Single` ohne Kopf-Checkbox, `None` ohne
  Boxen, Kopieren ohne die Spalte, axe. Markup in `crates/dioxus-datagrid/tests/checkbox.rs`, die
  Spalte selbst in `crates/datagrid-core/tests/checkbox.rs`, der Anteil der Auswahl in
  `crates/datagrid-core/tests/selection.rs`.
- **Einfügen:** `tests/e2e/paste.spec.ts` — Block ab der fokussierten Zelle, eine Zelle füllt das
  Rechteck, der eingefügte Block bleibt markiert, ein abgelehnter Wert lässt seine Zeile stehen,
  Überhang fällt weg, keine neuen Zeilen, offener Editor, Gitter ohne Bearbeitung, Stapelmodus, Kopf.
  Die Tests verschicken das `paste`-Ereignis selbst; ein echtes `Ctrl+V` ist nicht synthetisierbar
  (VERIFICATION §15). Die Regeln in `crates/dioxus-datagrid/tests/paste.rs`, das Zurücklesen des
  Formats in `crates/datagrid-core/tests/tsv.rs`.
- **Zellauswahl:** `tests/e2e/cell-selection.spec.ts` — Klick, `Shift`+Klick, `Shift`+Pfeil in alle
  Richtungen, Pfeil ohne `Shift` beginnt neu, `Single` bleibt bei einer Zelle, Kopf nicht
  auswählbar, Zeilen- und Zellauswahl nebeneinander, axe mit Rechteck. Markup in
  `crates/dioxus-datagrid/tests/cell_selection.rs`, das Rechteck selbst in
  `crates/datagrid-core/tests/cell_range.rs`.
- **Zellen über mehrere Spalten:** `tests/e2e/column-span.spec.ts` — `aria-colspan` und die
  fortlaufenden `aria-colindex`, Breite der Zelle gegen die Spalten darüber, Pfeiltasten über und
  auf eine breite Zelle, `Home`/`End`, ein Tab-Stopp, axe. Markup in
  `crates/dioxus-datagrid/tests/span.rs`, Kern in `crates/datagrid-core/tests/span.rs`.
- **Spaltenmenü:** `tests/e2e/column-menu.spec.ts` — Sortieren, Gruppieren, Fixieren, Ausblenden
  und Breite über das Menü, `Alt+↓` zum Öffnen, Fokus auf dem ersten Eintrag und zurück auf dem
  Kopf, Umlaufen der Pfeiltasten, kein zweiter Tab-Stopp, Klick daneben, axe bei offenem Menü.
  Die Einträge selbst in `crates/dioxus-datagrid/tests/column_menu.rs`.
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
