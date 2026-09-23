import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 11: columns held at an edge while the grid scrolls sideways. The
// playground pins Name to the start and Age to the end, both outermost at
// their edge, which is the case that needs no measured width.

const grid = (page: Page) => page.getByRole("grid");
const header = (page: Page, name: string) => page.getByRole("columnheader", { name, exact: true });

/** Left and right of an element, rounded, in viewport coordinates. */
async function edges(page: Page, selector: string) {
  return page.locator(selector).first().evaluate((el) => {
    const box = el.getBoundingClientRect();
    return { left: Math.round(box.left), right: Math.round(box.right) };
  });
}

/** Scrolls the grid as far right as it goes, and says how far that was. */
async function scrollToEnd(page: Page) {
  return grid(page).evaluate((el) => {
    el.scrollLeft = el.scrollWidth;
    return el.scrollWidth - el.clientWidth;
  });
}

/**
 * Opens the playground with every column shown and the viewport narrow enough
 * that the grid has to scroll sideways.
 */
async function open(page: Page, { virtualized = false } = {}) {
  await page.setViewportSize({ width: 560, height: 900 });
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.getByTestId("toggle-pinning").check();
  if (virtualized) await page.getByTestId("toggle-virtualized").check();

  // The two formatted columns start hidden; showing them forces the overflow.
  await page.locator(".dg-columns").locator("summary").click();
  for (const name of ["Salary", "Since"]) {
    await page.locator(".dg-columns").getByRole("checkbox", { name }).check();
  }
  await page.locator(".dg-columns").locator("summary").click();
  await expect(grid(page)).toHaveAttribute("aria-colcount", "6");
}

/** The `--dg-pin-offset` a header carries, in pixels. */
const offsetOf = (page: Page, name: string) =>
  header(page, name).evaluate((el) =>
    Number.parseFloat(getComputedStyle(el).getPropertyValue("--dg-pin-offset")),
  );

test("a pinned column says which edge it is held at", async ({ page }) => {
  await open(page);

  await expect(header(page, "Name")).toHaveAttribute("data-pinned", "start");
  await expect(header(page, "Email")).toHaveAttribute("data-pinned", "start");
  await expect(header(page, "Age")).toHaveAttribute("data-pinned", "end");
  await expect(header(page, "Department")).not.toHaveAttribute("data-pinned", /.*/);

  // Outermost at their edge, so against it.
  expect(await offsetOf(page, "Name")).toBe(0);
  expect(await offsetOf(page, "Age")).toBe(0);
});

test("a second pinned column clears the first", async ({ page }) => {
  await open(page);

  // Name is auto-sized, so this only works if the grid measured it — which is
  // what docs/VERIFICATION.md 13 is about.
  const width = await header(page, "Name").evaluate((el) => el.getBoundingClientRect().width);
  await expect.poll(() => offsetOf(page, "Email")).toBeCloseTo(width, 0);

  await scrollToEnd(page);
  const name = await edges(page, '[role="columnheader"][data-pinned="start"]');
  const email = await edges(page, '[role="columnheader"][data-pinned="start"] ~ [data-pinned="start"]');
  // Side by side at the edge, not stacked on top of each other.
  expect(email.left).toBe(name.right);
});

test("a pinned column moves to its edge, whatever the column order", async ({ page }) => {
  await open(page);

  // Age is declared fourth of six and pinned to the end, so it is shown last.
  const labels = await grid(page)
    .getByRole("columnheader")
    .evaluateAll((cells) => cells.map((cell) => cell.textContent?.trim().replace(/[▲▼]$/, "").trim()));
  expect(labels[0]).toBe("Name");
  expect(labels[labels.length - 1]).toBe("Age");
});

test("the pinned columns stay at the edges while the rest scrolls away", async ({ page }) => {
  await open(page);

  const before = await edges(page, '[role="columnheader"][data-pinned="start"]');
  const scrolling = '[role="columnheader"]:not([data-pinned])';
  const email = await edges(page, scrolling);
  const overflow = await scrollToEnd(page);
  expect(overflow).toBeGreaterThan(50);

  const gridBox = await edges(page, ".dg");
  const start = await edges(page, '[role="columnheader"][data-pinned="start"]');
  const end = await edges(page, '[role="columnheader"][data-pinned="end"]');
  const emailAfter = await edges(page, scrolling);

  // The pinned ones are still against their edges, to the pixel.
  expect(start.left).toBe(gridBox.left);
  expect(start.left).toBe(before.left);
  expect(end.right).toBe(gridBox.right);
  // The one beside them scrolled left, far enough to pass behind the block of
  // pinned columns rather than being pushed along by it.
  const blockEnd = await page
    .locator('.dg [role="columnheader"][data-pinned="start"]')
    .last()
    .evaluate((el) => Math.round(el.getBoundingClientRect().right));
  expect(emailAfter.left).toBeLessThan(email.left);
  expect(emailAfter.left).toBeLessThan(blockEnd);
});

test("the cells are held along with their header", async ({ page }) => {
  await open(page);
  await scrollToEnd(page);

  const gridBox = await edges(page, ".dg");
  const cell = await edges(page, '.dg-body [role="gridcell"][data-pinned="start"]');
  const header = await edges(page, '[role="columnheader"][data-pinned="start"]');

  // A column that only stuck in its header would tear as the grid scrolls.
  expect(cell.left).toBe(gridBox.left);
  expect(cell.left).toBe(header.left);
});

test("a virtualized row does not break the hold", async ({ page }) => {
  await open(page, { virtualized: true });

  // The regression this guards: `overflow: hidden` on a virtualized row makes
  // the row its own scroll container, and the cell sticks to the row instead
  // of to the grid. See docs/VERIFICATION.md 13.
  await expect(grid(page).locator('.dg-body [role="row"]').first()).toHaveCSS("overflow", "clip");

  await grid(page).evaluate((el) => {
    el.scrollLeft = el.scrollWidth;
    el.scrollTop = 4000;
  });
  await expect
    .poll(async () => (await edges(page, '.dg-body [role="gridcell"][data-pinned="start"]')).left)
    .toBe((await edges(page, ".dg")).left);
});

test("a pinned grid has no accessibility violations", async ({ page }) => {
  await open(page);
  await scrollToEnd(page);

  const results = await new AxeBuilder({ page }).include(".dg").analyze();
  expect(results.violations).toEqual([]);
});
