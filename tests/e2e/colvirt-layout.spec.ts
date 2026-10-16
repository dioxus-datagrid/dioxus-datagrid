import { expect, type Page, test } from "@playwright/test";

// The layout facts column virtualization stands on, as one framework-free page
// per case — see docs/VERIFICATION.md §20.
//
// Can columns be virtualized next to CSS subgrid?
//
// The grid lays every row out as a subgrid of one track list on the root. To
// render only some columns, the cells that are rendered have to be placed on
// their own track (`grid-column: N`), and the tracks of the cells that are not
// rendered have to keep their size without them. This page builds exactly that
// shape, with no framework, and measures it.

const COLUMNS = 300;
const ROWS = 20;

type Setup = {
  template: string;
  // Which columns carry a cell. Everything else is left out of the DOM.
  rendered: (column: number) => boolean;
  // The text of a cell, to give auto tracks something to size by.
  text?: (column: number, row: number) => string;
  pinnedStart?: number[];
  pinnedEnd?: number[];
};

const PAGE = (setup: Setup) => {
  const cells = (row: number) => {
    let html = "";
    for (let column = 0; column < COLUMNS; column++) {
      if (!setup.rendered(column)) continue;
      const pin = setup.pinnedStart?.includes(column)
        ? ' data-pinned="start" style="inset-inline-start:' + column * 100 + 'px;grid-column:' + (column + 1) + '"'
        : setup.pinnedEnd?.includes(column)
          ? ' data-pinned="end" style="inset-inline-end:' + (COLUMNS - 1 - column) * 100 + 'px;grid-column:' + (column + 1) + '"'
          : ' style="grid-column:' + (column + 1) + '"';
      const text = setup.text ? setup.text(column, row) : "c" + column;
      html += '<div role="gridcell" data-col="' + column + '"' + pin + ">" + text + "</div>";
    }
    return html;
  };
  let rows = "";
  for (let row = 0; row < ROWS; row++) rows += '<div role="row">' + cells(row) + "</div>";
  return `
<!doctype html>
<meta charset="utf-8">
<style>
  body { margin: 0; }
  .dg { display: grid; overflow: auto; align-content: start; width: 800px; height: 400px;
        grid-template-columns: ${setup.template}; }
  .dg [role="row"] { display: grid; grid-column: 1 / -1; grid-template-columns: subgrid; }
  .dg [role="gridcell"] { overflow: hidden; white-space: nowrap; padding: 0 8px; box-sizing: border-box; height: 24px; }
  .dg [data-pinned] { position: sticky; z-index: 1; background: #eee; }
</style>
<div class="dg" role="grid">${rows}</div>`;
};

const fixed = `repeat(${COLUMNS}, 100px)`;

const metrics = (page: Page) =>
  page.evaluate(() => {
    const root = document.querySelector(".dg") as HTMLElement;
    const row = root.querySelector('[role="row"]') as HTMLElement;
    return {
      scrollWidth: root.scrollWidth,
      rowWidth: row.getBoundingClientRect().width,
      cells: row.querySelectorAll('[role="gridcell"]').length,
    };
  });

const left = (page: Page, column: number) =>
  page.evaluate((column) => {
    const root = document.querySelector(".dg") as HTMLElement;
    const cell = root.querySelector('[role="row"] [data-col="' + column + '"]') as HTMLElement | null;
    if (!cell) return null;
    return Math.round(cell.getBoundingClientRect().left - root.getBoundingClientRect().left + root.scrollLeft);
  }, column);

test("fixed tracks keep their size and place without the cells next to them", async ({ page }) => {
  // Only columns 140..160 exist.
  await page.setContent(PAGE({ template: fixed, rendered: (c) => c >= 140 && c < 160 }));
  const m = await metrics(page);
  console.log("windowed, fixed tracks:", JSON.stringify(m));
  expect(m.cells).toBe(20);
  expect(m.scrollWidth).toBe(COLUMNS * 100);
  expect(m.rowWidth).toBe(COLUMNS * 100);
  // Column 150 sits where 150 tracks of 100px put it, not at the start.
  expect(await left(page, 150)).toBe(15000);
});

test("pinned cells stay at their edge while the columns around them are absent", async ({ page }) => {
  await page.setContent(
    PAGE({
      template: fixed,
      rendered: (c) => c === 0 || c === COLUMNS - 1 || (c >= 140 && c < 160),
      pinnedStart: [0],
      pinnedEnd: [COLUMNS - 1],
    }),
  );
  await page.evaluate(() => {
    (document.querySelector(".dg") as HTMLElement).scrollLeft = 14000;
  });
  const edges = await page.evaluate(() => {
    const root = document.querySelector(".dg") as HTMLElement;
    const box = root.getBoundingClientRect();
    const first = (root.querySelector('[data-col="0"]') as HTMLElement).getBoundingClientRect();
    const last = (root.querySelector('[data-col="299"]') as HTMLElement).getBoundingClientRect();
    return { startGap: Math.round(first.left - box.left), endGap: Math.round(box.right - last.right) };
  });
  console.log("pinned edges at scrollLeft 14000:", JSON.stringify(edges));
  // The scrollbar of the root eats a few pixels at the end edge; the start edge is exact.
  expect(edges.startGap).toBe(0);
  expect(edges.endGap).toBeLessThanOrEqual(16);
});

test("auto tracks with a floor sit at the floor once the grid overflows, cells or not", async ({ page }) => {
  const template = `repeat(${COLUMNS}, minmax(40px, auto))`;
  const text = (c: number) => "x".repeat(4 + (c % 7) * 6);

  await page.setContent(PAGE({ template, rendered: () => true, text }));
  const all = await metrics(page);
  await page.setContent(PAGE({ template, rendered: (c) => c >= 140 && c < 160, text }));
  const some = await metrics(page);
  console.log("auto tracks, all rendered vs windowed:", JSON.stringify({ all, some }));
  // `auto` as the maximum only grows a track into free space. A grid that needs
  // column virtualization has none, so its auto columns are as wide as their
  // floor and a window of cells cannot change that.
  expect(all.scrollWidth).toBe(COLUMNS * 40);
  expect(some.scrollWidth).toBe(all.scrollWidth);
});

test("auto tracks do follow their cells when there is free space", async ({ page }) => {
  // Few columns in a wide root: the case where the window is every column anyway.
  const template = "repeat(4, minmax(40px, auto))";
  const text = (c: number) => "x".repeat(4 + c * 20);
  const grid = (rendered: (c: number) => boolean) =>
    `<!doctype html><meta charset="utf-8"><style>
      .dg { display: grid; width: 800px; grid-template-columns: ${template}; }
      .dg [role="row"] { display: grid; grid-column: 1 / -1; grid-template-columns: subgrid; }
      .dg [role="gridcell"] { overflow: hidden; white-space: nowrap; }
    </style><div class="dg"><div role="row">${[0, 1, 2, 3]
      .filter(rendered)
      .map((c) => `<div role="gridcell" style="grid-column:${c + 1}">${text(c)}</div>`)
      .join("")}</div></div>`;
  const widthOf = async (rendered: (c: number) => boolean) => {
    await page.setContent(grid(rendered));
    return page.evaluate(() => {
      const cells = [...document.querySelectorAll('[role="gridcell"]')];
      return cells.map((cell) => Math.round(cell.getBoundingClientRect().width));
    });
  };
  const all = await widthOf(() => true);
  const some = await widthOf((c) => c !== 3);
  console.log("free space, all vs one absent:", JSON.stringify({ all, some }));
  // Dropping a cell moves the others' widths: nothing to rely on here.
  expect(some[0]).not.toBe(all[0]);
});

test("fr tracks with a floor are unaffected by absent cells", async ({ page }) => {
  const template = `repeat(${COLUMNS}, minmax(48px, 1fr))`;
  await page.setContent(PAGE({ template, rendered: () => true }));
  const all = await metrics(page);
  await page.setContent(PAGE({ template, rendered: (c) => c >= 140 && c < 160 }));
  const some = await metrics(page);
  console.log("fr tracks, all rendered vs windowed:", JSON.stringify({ all, some }));
  expect(some.scrollWidth).toBe(all.scrollWidth);
});

test("a cell over several columns is placed with `grid-column: N / span M`", async ({ page }) => {
  await page.setContent(`
<!doctype html><meta charset="utf-8">
<style>
  .dg { display: grid; overflow: auto; width: 800px; height: 100px; grid-template-columns: ${fixed}; }
  .dg [role="row"] { display: grid; grid-column: 1 / -1; grid-template-columns: subgrid; }
  .dg [role="gridcell"] { height: 24px; box-sizing: border-box; }
</style>
<div class="dg"><div role="row"><div id="wide" role="gridcell" style="grid-column: 11 / span 3">wide</div></div></div>`);
  const box = await page.locator("#wide").boundingBox();
  console.log("spanning cell:", JSON.stringify(box));
  expect(Math.round(box?.width ?? 0)).toBe(300);
});
