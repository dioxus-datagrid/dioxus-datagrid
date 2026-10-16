import { expect, type Page, test } from "@playwright/test";

// The layout facts right-to-left text stands on, one framework-free page per case.
// See docs/VERIFICATION.md §23.
//
// The grid lays every row out as a subgrid of one track list, scrolls on both
// axes inside its root, and holds pinned columns with `position: sticky` and
// logical offsets. Column virtualization counts the distance from the start edge
// as the absolute value of `scrollLeft`. Each of those is asked here under
// `dir="rtl"`, and under `dir="ltr"` beside it, so a difference is a measurement
// rather than a memory.

const COLUMNS = 300;

const page_ = (dir: "ltr" | "rtl", cells: (column: number) => boolean, extra = "") => `
<!doctype html>
<html dir="${dir}">
<meta charset="utf-8">
<style>
  body { margin: 0; }
  .dg { display: grid; overflow: auto; align-content: start; width: 800px; height: 200px;
        grid-template-columns: repeat(${COLUMNS}, 100px); }
  .dg [role="row"] { display: grid; grid-column: 1 / -1; grid-template-columns: subgrid; }
  .dg [role="gridcell"] { height: 24px; box-sizing: border-box; padding: 0 8px; overflow: hidden; }
  .dg [data-pinned] { position: sticky; z-index: 1; background: #eee; }
  .dg [data-pinned="start"] { inset-inline-start: 0; }
  .dg [data-pinned="end"] { inset-inline-end: 0; }
  ${extra}
</style>
<div class="dg" role="grid">
  ${[0, 1, 2]
    .map(
      (row) =>
        `<div role="row">` +
        Array.from({ length: COLUMNS }, (_, c) => c)
          .filter(cells)
          .map((c) => {
            const pin = c === 0 ? ' data-pinned="start"' : c === COLUMNS - 1 ? ' data-pinned="end"' : "";
            return `<div role="gridcell" data-col="${c}"${pin} style="grid-column:${c + 1}">r${row}c${c}</div>`;
          })
          .join("") +
        `</div>`,
    )
    .join("")}
</div>`;

const open = (page: Page, dir: "ltr" | "rtl", cells: (c: number) => boolean = () => true, extra = "") =>
  page.setContent(page_(dir, cells, extra));

/** The root's scroll position after scrolling it by `to` (a signed distance). */
const scrollTo = (page: Page, to: "start" | "end" | number) =>
  page.evaluate((to) => {
    const root = document.querySelector(".dg") as HTMLElement;
    const rtl = getComputedStyle(root).direction === "rtl";
    // Towards the end is a negative distance in right-to-left text.
    root.scrollLeft = to === "start" ? 0 : to === "end" ? (rtl ? -root.scrollWidth : root.scrollWidth) : (to as number);
    return { scrollLeft: root.scrollLeft, max: root.scrollWidth - root.clientWidth };
  }, to);

/** Where a cell's near edge is, from the root's start edge, in the reading direction. */
const fromStart = (page: Page, column: number) =>
  page.evaluate((column) => {
    const root = document.querySelector(".dg") as HTMLElement;
    const cell = root.querySelector(`[role="row"] [data-col="${column}"]`) as HTMLElement | null;
    if (!cell) return null;
    const box = root.getBoundingClientRect();
    const c = cell.getBoundingClientRect();
    const rtl = getComputedStyle(root).direction === "rtl";
    // Distance of the cell's start edge from the start edge of the content, whatever is scrolled.
    return Math.round((rtl ? box.right - c.right : c.left - box.left) + Math.abs(root.scrollLeft));
  }, column);

test("scrollLeft: zero at the start edge, and its sign when scrolled", async ({ page }) => {
  await open(page, "ltr");
  const ltr = await scrollTo(page, "end");
  await open(page, "rtl");
  const atStart = await page.evaluate(() => (document.querySelector(".dg") as HTMLElement).scrollLeft);
  const rtl = await scrollTo(page, "end");
  const rtlBack = await scrollTo(page, -3000);
  console.log("scrollLeft ltr end:", JSON.stringify(ltr), "| rtl start:", atStart, "| rtl end:", JSON.stringify(rtl), "| rtl -3000:", JSON.stringify(rtlBack));

  expect(ltr.scrollLeft).toBe(ltr.max);
  // At rest the scroll container sits at the start edge, which is zero in both directions.
  expect(Object.is(atStart, -0) || atStart === 0).toBe(true);
  // Scrolled towards the end, RTL reports the distance as a negative number.
  expect(rtl.scrollLeft).toBeLessThan(0);
  expect(Math.abs(rtl.scrollLeft)).toBe(rtl.max);
  // Assigning a negative value scrolls the same way (a positive one would not).
  expect(rtlBack.scrollLeft).toBe(-3000);
});

test("the distance from the start edge is the absolute value of scrollLeft", async ({ page }) => {
  for (const dir of ["ltr", "rtl"] as const) {
    await open(page, dir);
    // Column 150 sits 15000px from the start edge whatever is scrolled.
    for (const to of [0, 4000]) {
      await scrollTo(page, dir === "rtl" ? -to : to);
      expect(await fromStart(page, 150), `${dir} scrolled ${to}`).toBe(15000);
    }
  }
});

test("explicit grid-column places the first column at the start edge", async ({ page }) => {
  await open(page, "rtl", (c) => c >= 140 && c < 160);
  const m = await page.evaluate(() => {
    const root = document.querySelector(".dg") as HTMLElement;
    return { scrollWidth: root.scrollWidth, rowWidth: root.querySelector("[role='row']")!.getBoundingClientRect().width };
  });
  console.log("rtl windowed:", JSON.stringify(m));
  // The tracks are all there, so the scrollbar covers every column even with cells missing.
  expect(m.scrollWidth).toBe(COLUMNS * 100);
  expect(await fromStart(page, 150)).toBe(15000);
});

test("pinned cells stick to the logical edge, and the other one to the opposite", async ({ page }) => {
  for (const dir of ["ltr", "rtl"] as const) {
    await open(page, dir);
    await scrollTo(page, dir === "rtl" ? -14000 : 14000);
    const gaps = await page.evaluate(() => {
      const root = document.querySelector(".dg") as HTMLElement;
      const box = root.getBoundingClientRect();
      const start = root.querySelector('[data-pinned="start"]')!.getBoundingClientRect();
      const end = root.querySelector('[data-pinned="end"]')!.getBoundingClientRect();
      const rtl = getComputedStyle(root).direction === "rtl";
      const right = box.left + root.clientWidth;
      return {
        // From the edge the cell is pinned to, in the reading direction.
        start: Math.round(rtl ? right - start.right : start.left - box.left),
        end: Math.round(rtl ? end.left - box.left : right - end.right),
      };
    });
    console.log(`${dir} pinned gaps at 14000:`, JSON.stringify(gaps));
    expect(gaps, dir).toEqual({ start: 0, end: 0 });
  }
});

test("a pinned-start cell is on the right in RTL, and stays there while scrolling", async ({ page }) => {
  await open(page, "rtl");
  const side = async () =>
    page.evaluate(() => {
      const root = document.querySelector(".dg") as HTMLElement;
      const box = root.getBoundingClientRect();
      const cell = root.querySelector('[data-pinned="start"]')!.getBoundingClientRect();
      return Math.round(box.left + root.clientWidth - cell.right);
    });
  expect(await side()).toBe(0);
  await scrollTo(page, -9000);
  expect(await side()).toBe(0);
});

test("a physical box-shadow does not mirror, a logical border does", async ({ page }) => {
  // The grid draws a line at the edge of the pinned block with a box-shadow. This
  // measures which side a `1px 0 0` shadow lands on: it is the left in both.
  const shadowSide = async (dir: "ltr" | "rtl") => {
    await open(page, dir, () => true, `.dg [data-pinned="start"] { box-shadow: 1px 0 0 red; border-inline-end: 1px solid blue; }`);
    return page.evaluate(() => {
      const cell = document.querySelector('[data-pinned="start"]') as HTMLElement;
      const style = getComputedStyle(cell);
      return {
        shadow: style.boxShadow,
        borderRight: style.borderRightWidth,
        borderLeft: style.borderLeftWidth,
      };
    });
  };
  const ltr = await shadowSide("ltr");
  const rtl = await shadowSide("rtl");
  console.log("ltr:", JSON.stringify(ltr), "| rtl:", JSON.stringify(rtl));
  // The shadow's offset is the same string; only the logical border moved.
  expect(rtl.shadow).toBe(ltr.shadow);
  expect(ltr.borderRight).toBe("1px");
  expect(rtl.borderLeft).toBe("1px");
});

test("a positive scrollLeft in RTL is clamped to the start edge", async ({ page }) => {
  // What a program has to know before it scrolls a right-to-left container: the
  // sign is not a detail. A positive value does not scroll the other way, it
  // scrolls nowhere, so scrolling by a distance needs the direction.
  await open(page, "rtl");
  const clamped = await scrollTo(page, 3000);
  const negative = await scrollTo(page, -3000);
  console.log("rtl +3000:", clamped.scrollLeft, "| rtl -3000:", negative.scrollLeft);
  expect(clamped.scrollLeft).toBe(0);
  expect(negative.scrollLeft).toBe(-3000);
});
