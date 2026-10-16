import { expect, type Page, test } from "@playwright/test";

// Phase 15 (taken ahead of its turn): a grid with many columns renders only the
// ones in view. The playground's wide setup has 200 columns, five of them the
// usual ones and the rest "Metric n" at a fixed 120px, each cell showing
// `id * (n + 2) % 997` so a test can predict any cell from its place.

const COLUMNS = 200;
const WIDTH = 120;

const grid = (page: Page) => page.getByRole("grid");
const leafHeaders = (page: Page) => page.locator(".dg-head [role='row']").last().getByRole("columnheader");
const firstRowCells = (page: Page) => page.locator(".dg-body [role='row']").first().getByRole("gridcell");

/** The `aria-colindex` of every cell in a locator, in DOM order. */
const columnIndices = (cells: ReturnType<typeof leafHeaders>) =>
  cells.evaluateAll((cells) => cells.map((cell) => Number(cell.getAttribute("aria-colindex"))));

async function open(page: Page, { virtual = true, pinned = false } = {}) {
  await page.setViewportSize({ width: 900, height: 900 });
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  // Two of the five usual columns start hidden, which leaves 4 + 195 columns.
  // Showing them is not needed; the count below is what the grid says.
  await page.getByTestId("toggle-wide").check();
  if (pinned) await page.getByTestId("toggle-pinning").check();
  if (virtual) await page.getByTestId("toggle-virtual-columns").check();
  await expect(grid(page)).toHaveAttribute("aria-colcount", /^\d+$/);
}

async function scrollLeftTo(page: Page, left: number) {
  await grid(page).evaluate((root, value) => {
    root.scrollLeft = value;
  }, left);
}

test("without virtual columns every column is in the DOM", async ({ page }) => {
  await open(page, { virtual: false });
  const total = Number(await grid(page).getAttribute("aria-colcount"));
  expect(total).toBeGreaterThan(150);
  await expect(leafHeaders(page)).toHaveCount(total);
});

test("with virtual columns only the ones in view are", async ({ page }) => {
  await open(page);
  const total = Number(await grid(page).getAttribute("aria-colcount"));
  const rendered = await leafHeaders(page).count();
  console.log(`rendered ${rendered} of ${total} column headers`);
  expect(rendered).toBeLessThan(25);
  expect(rendered).toBeGreaterThan(0);
  // The rest of the grid is still there for a reader.
  expect(total).toBeGreaterThan(150);
  // Cells follow the header: the same columns.
  expect(await columnIndices(firstRowCells(page))).toEqual(await columnIndices(leafHeaders(page)));
});

test("the scrollbar covers every column", async ({ page }) => {
  await open(page);
  const { scrollWidth, tracks } = await grid(page).evaluate((root) => ({
    scrollWidth: root.scrollWidth,
    tracks: getComputedStyle(root).gridTemplateColumns.split(" ").length,
  }));
  expect(tracks).toBe(Number(await grid(page).getAttribute("aria-colcount")));
  // The metrics alone are 195 x 120px.
  expect(scrollWidth).toBeGreaterThan(190 * WIDTH);
});

test("scrolling sideways brings other columns in, aligned under their headers", async ({ page }) => {
  await open(page);
  await scrollLeftTo(page, 100 * WIDTH);

  // Column 100 + something is rendered now; the first metric is not.
  await expect(page.getByRole("columnheader", { name: "Metric 100", exact: true })).toBeVisible();
  await expect(page.getByRole("columnheader", { name: "Metric 1", exact: true })).toHaveCount(0);

  const aligned = await grid(page).evaluate((root) => {
    const heads = [...root.querySelectorAll(".dg-head [role='row']:last-child [role='columnheader']")];
    const cells = [...root.querySelectorAll(".dg-body [role='row']:first-child [role='gridcell']")];
    return heads.map((head) => {
      const index = head.getAttribute("aria-colindex");
      const cell = cells.find((cell) => cell.getAttribute("aria-colindex") === index);
      const h = head.getBoundingClientRect();
      const c = cell?.getBoundingClientRect();
      return { index, same: !!c && Math.round(h.left) === Math.round(c.left) && Math.round(h.width) === Math.round(c.width) };
    });
  });
  expect(aligned.length).toBeGreaterThan(5);
  expect(aligned.filter((entry) => !entry.same)).toEqual([]);

  // A cell's text is what its place says: row id 1 under Metric n.
  const text = await page
    .locator(".dg-body [role='row']")
    .first()
    .locator("[aria-colindex]")
    .evaluateAll((cells) => cells.map((cell) => [cell.getAttribute("aria-colindex"), cell.textContent]));
  for (const [colindex, value] of text) {
    const column = Number(colindex);
    // Metric n is the n-th column after the four that are shown at first.
    const number = column - 4;
    if (number >= 1) expect(Number(value)).toBe((1 * (number + 2)) % 997);
  }
});

test("a column further along is where its width says", async ({ page }) => {
  await open(page);
  await scrollLeftTo(page, 100 * WIDTH);
  const left = await page.getByRole("columnheader", { name: "Metric 100", exact: true }).evaluate((head) => {
    const root = head.closest("[role='grid']") as HTMLElement;
    return Math.round(head.getBoundingClientRect().left - root.getBoundingClientRect().left + root.scrollLeft);
  });
  // The four columns before the metrics have widths of their own; every metric
  // before this one is 120px, and there are 99 of them.
  const before = await grid(page).evaluate((root) => {
    const widths = getComputedStyle(root).gridTemplateColumns.split(" ").map(parseFloat);
    return widths.slice(0, 4).reduce((sum, width) => sum + width, 0);
  });
  expect(left).toBe(Math.round(before + 99 * WIDTH));
});

test("the keyboard reaches a column far outside the window", async ({ page }) => {
  await open(page);
  await leafHeaders(page).first().click();
  await page.keyboard.press("ArrowDown");

  for (let i = 0; i < 60; i++) await page.keyboard.press("ArrowRight");

  const focus = await page.evaluate(() => {
    const cell = document.activeElement as HTMLElement;
    const root = cell.closest("[role='grid']") as HTMLElement;
    const c = cell.getBoundingClientRect();
    const r = root.getBoundingClientRect();
    return {
      role: cell.getAttribute("role"),
      column: cell.getAttribute("aria-colindex"),
      inside: c.left >= r.left - 1 && c.right <= r.left + root.clientWidth + 1,
    };
  });
  expect(focus).toEqual({ role: "gridcell", column: "61", inside: true });
});

test("End on a row reaches the last column", async ({ page }) => {
  await open(page);
  await leafHeaders(page).first().click();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("End");
  const total = await grid(page).getAttribute("aria-colcount");
  const column = await page.evaluate(() => document.activeElement?.getAttribute("aria-colindex"));
  expect(column).toBe(total);
});

test("a focused cell that scrolled away hands the tab stop to the grid", async ({ page }) => {
  await open(page);
  await leafHeaders(page).first().click();
  await page.keyboard.press("ArrowDown");
  await scrollLeftTo(page, 120 * WIDTH);
  // The focused cell is gone from the DOM, so exactly one tab stop is left: the grid.
  await expect(grid(page)).toHaveAttribute("tabindex", "0");
  await expect(page.locator(".dg [role='gridcell'][tabindex='0'], .dg [role='columnheader'][tabindex='0']")).toHaveCount(0);
  // And tabbing back in finds a cell again.
  await scrollLeftTo(page, 0);
  await expect(grid(page)).toHaveAttribute("tabindex", "-1");
  await expect(page.locator(".dg [role='gridcell'][tabindex='0']")).toHaveCount(1);
});

test("pinned columns stay in the DOM and at their edge", async ({ page }) => {
  await open(page, { pinned: true });
  await scrollLeftTo(page, 150 * WIDTH);

  const name = page.getByRole("columnheader", { name: "Name", exact: true });
  const age = page.getByRole("columnheader", { name: "Age", exact: true });
  await expect(name).toBeVisible();
  await expect(age).toBeVisible();

  const gaps = await grid(page).evaluate((root) => {
    const box = root.getBoundingClientRect();
    const first = root.querySelector("[role='columnheader'][data-pinned='start']") as HTMLElement;
    const last = root.querySelector("[role='columnheader'][data-pinned='end']") as HTMLElement;
    return {
      start: Math.round(first.getBoundingClientRect().left - box.left),
      end: Math.round(box.left + root.clientWidth - last.getBoundingClientRect().right),
    };
  });
  expect(gaps).toEqual({ start: 0, end: 0 });
  // Metric 150 is in the window between them.
  await expect(page.getByRole("columnheader", { name: "Metric 150", exact: true })).toBeVisible();
});

test("a column revealed by the keyboard clears a pinned column", async ({ page }) => {
  await open(page, { pinned: true });
  await page.getByRole("columnheader", { name: "Metric 1", exact: true }).click();
  await page.keyboard.press("ArrowDown");
  for (let i = 0; i < 12; i++) await page.keyboard.press("ArrowRight");

  const clear = await page.evaluate(() => {
    const cell = document.activeElement as HTMLElement;
    const root = cell.closest("[role='grid']") as HTMLElement;
    const pinned = [...root.querySelectorAll("[role='gridcell'][data-pinned='start']")].at(-1) as HTMLElement;
    const edge = [...root.querySelectorAll("[role='gridcell'][data-pinned='end']")][0] as HTMLElement;
    const c = cell.getBoundingClientRect();
    return {
      clearOfStart: c.left >= pinned.getBoundingClientRect().right - 1,
      clearOfEnd: c.right <= edge.getBoundingClientRect().left + 1,
    };
  });
  expect(clear).toEqual({ clearOfStart: true, clearOfEnd: true });
});

test("columns follow a resize of the window", async ({ page }) => {
  await open(page);
  const narrow = await leafHeaders(page).count();
  await page.setViewportSize({ width: 1600, height: 900 });
  await expect.poll(() => leafHeaders(page).count()).toBeGreaterThan(narrow);
});
