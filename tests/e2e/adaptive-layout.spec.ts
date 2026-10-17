import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 14: the adaptive layout. The playground gives data_grid compact_below: 600
// when "Cards when narrow" is on; below that width of the grid, rows are cards, the
// filters are in a sheet behind a button, and the edit form comes up from the edge.

const grid = (page: Page) => page.getByRole("grid");
const count = (page: Page) => page.locator(".dg-count");
const cell = (page: Page, row: number, column: number) =>
  page.locator(`.dg-body [role='row'] [aria-colindex='${column}']`).nth(row);
const sheet = (page: Page) => page.getByRole("dialog", { name: "Filters" });
const filtersButton = (page: Page) => page.getByRole("button", { name: "Filters", exact: true });

async function open(page: Page, width: number, extra: (page: Page) => Promise<void> = async () => {}) {
  await page.setViewportSize({ width, height: 900 });
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.getByTestId("toggle-compact").check();
  await extra(page);
  await expect(grid(page)).toBeVisible();
}

test("a narrow grid lays its rows out as cards, and says so", async ({ page }) => {
  await open(page, 400);
  await expect(grid(page)).toHaveAttribute("data-compact", "true");
  // Every cell says what it is.
  await expect(cell(page, 0, 1)).toHaveAttribute("data-label", "Name");
  await expect(cell(page, 0, 4)).toHaveAttribute("data-label", "Age");
  // The label is drawn in front of the value, from the attribute.
  const before = await cell(page, 0, 2).evaluate((el) => getComputedStyle(el, "::before").content);
  expect(before).toBe('"Email"');

  // A card is its cells one above the other.
  const [first, second] = await Promise.all([cell(page, 0, 1).boundingBox(), cell(page, 0, 2).boundingBox()]);
  expect(second!.y).toBeGreaterThan(first!.y + first!.height - 2);
  expect(Math.abs(second!.x - first!.x)).toBeLessThan(2);
});

test("a wide grid is a table, with or without the option", async ({ page }) => {
  await open(page, 1280);
  await expect(grid(page)).not.toHaveAttribute("data-compact", /.*/);
  await expect(cell(page, 0, 1)).not.toHaveAttribute("data-label", /.*/);
  const [first, second] = await Promise.all([cell(page, 0, 1).boundingBox(), cell(page, 0, 2).boundingBox()]);
  expect(Math.abs(second!.y - first!.y)).toBeLessThan(2);
  expect(second!.x).toBeGreaterThan(first!.x);
});

test("it follows the width of the window both ways", async ({ page }) => {
  await open(page, 1280);
  await expect(grid(page)).not.toHaveAttribute("data-compact", /.*/);
  await page.setViewportSize({ width: 400, height: 900 });
  await expect(grid(page)).toHaveAttribute("data-compact", "true");
  await page.setViewportSize({ width: 1280, height: 900 });
  await expect(grid(page)).not.toHaveAttribute("data-compact", /.*/);
});

test("without the option nothing changes, however narrow", async ({ page }) => {
  await page.setViewportSize({ width: 400, height: 900 });
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(grid(page)).toBeVisible();
  await expect(grid(page)).not.toHaveAttribute("data-compact", /.*/);
});

test("the header stays for a screen reader, out of sight", async ({ page }) => {
  await open(page, 400);
  const header = page.getByRole("columnheader", { name: "Name", exact: true });
  await expect(header).toHaveCount(1);
  // The group of headers is clipped to a point, not removed: `display: none`
  // would take it out of the accessibility tree.
  const box = (await page.locator(".dg-head").boundingBox())!;
  expect(box.height).toBeLessThanOrEqual(2);
  expect(box.width).toBeLessThanOrEqual(2);
});

test("the keys still move through the grid as a grid", async ({ page }) => {
  await open(page, 400);
  await cell(page, 0, 1).click();
  await page.keyboard.press("ArrowDown");
  await expect(cell(page, 1, 1)).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await expect(cell(page, 1, 2)).toBeFocused();
});

test("nothing is pinned or scrolls sideways", async ({ page }) => {
  await open(page, 400, async (page) => {
    await page.getByTestId("toggle-pinning").check();
  });
  await expect(cell(page, 0, 1)).toHaveCSS("position", "static");
  const overflow = await grid(page).evaluate((el) => el.scrollWidth - el.clientWidth);
  expect(overflow).toBeLessThanOrEqual(1);
});

test("the filters are behind a button, in a sheet", async ({ page }) => {
  await open(page, 400);
  await expect(page.locator(".dg-filters")).toHaveCount(0);
  await expect(sheet(page)).toHaveCount(0);
  await expect(filtersButton(page)).toHaveAttribute("aria-expanded", "false");

  await filtersButton(page).click();
  await expect(sheet(page)).toBeVisible();
  await expect(filtersButton(page)).toHaveAttribute("aria-expanded", "true");
  // The focus starts on the close button.
  await expect(sheet(page).getByRole("button", { name: "Close" })).toBeFocused();

  // At the bottom edge, as wide as the window.
  const box = (await sheet(page).boundingBox())!;
  const view = page.viewportSize()!;
  expect(Math.abs(box.y + box.height - view.height)).toBeLessThan(2);
  expect(Math.abs(box.width - view.width)).toBeLessThan(2);

  // A filter in it filters the grid behind.
  await expect(count(page)).toHaveText(/^12 rows/);
  await sheet(page).locator("label", { hasText: "Department" }).locator("input").fill("engineering");
  await expect(count(page)).toHaveText(/^3 rows/);
});

test("Escape closes the sheet and puts the focus back on the button", async ({ page }) => {
  await open(page, 400);
  await filtersButton(page).click();
  await expect(sheet(page)).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(sheet(page)).toHaveCount(0);
  await expect(filtersButton(page)).toBeFocused();
});

test("tapping outside closes the sheet, and so does Close", async ({ page }) => {
  await open(page, 400);
  await filtersButton(page).click();
  await page.mouse.click(200, 40);
  await expect(sheet(page)).toHaveCount(0);

  await filtersButton(page).click();
  await sheet(page).getByRole("button", { name: "Close" }).click();
  await expect(sheet(page)).toHaveCount(0);
  await expect(filtersButton(page)).toBeFocused();
});

test("a wide grid keeps its filters above it, with no button", async ({ page }) => {
  await open(page, 1280);
  await expect(page.locator(".dg-filters")).toHaveCount(1);
  await expect(filtersButton(page)).toHaveCount(0);
});

test("widening the window closes an open sheet", async ({ page }) => {
  await open(page, 400);
  await filtersButton(page).click();
  await expect(sheet(page)).toBeVisible();
  await page.setViewportSize({ width: 1280, height: 900 });
  await expect(sheet(page)).toHaveCount(0);
  await expect(page.locator(".dg-filters")).toHaveCount(1);
});

test("the edit form comes up from the bottom edge as a sheet", async ({ page }) => {
  await open(page, 400, async (page) => {
    await page.getByRole("radio", { name: "Dialog", exact: true }).first().check();
  });
  await page.getByRole("button", { name: "Add", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "New row" });
  await expect(dialog).toBeVisible();
  const box = (await dialog.boundingBox())!;
  const view = page.viewportSize()!;
  expect(Math.abs(box.y + box.height - view.height)).toBeLessThan(2);
  expect(Math.abs(box.width - view.width)).toBeLessThan(2);
});

test("on a wide grid the edit form stays in the middle", async ({ page }) => {
  await open(page, 1280, async (page) => {
    await page.getByRole("radio", { name: "Dialog", exact: true }).first().check();
  });
  await page.getByRole("button", { name: "Add", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "New row" });
  await expect(dialog).toBeVisible();
  const box = (await dialog.boundingBox())!;
  const view = page.viewportSize()!;
  expect(box.y + box.height).toBeLessThan(view.height - 20);
});

test("a grid that virtualizes its rows is never compact", async ({ page }) => {
  await open(page, 400, async (page) => {
    await page.getByTestId("toggle-virtualized").check();
  });
  await expect(grid(page)).not.toHaveAttribute("data-compact", /.*/);
});

test("a grid that virtualizes its columns is never compact", async ({ page }) => {
  await open(page, 400, async (page) => {
    await page.getByTestId("toggle-wide").check();
    await page.getByTestId("toggle-virtual-columns").check();
  });
  await expect(grid(page)).not.toHaveAttribute("data-compact", /.*/);
});

test("German names the button and the sheet in German", async ({ page }) => {
  await open(page, 400, async (page) => {
    await page.getByRole("radio", { name: "Deutsch" }).check();
  });
  await page.getByRole("button", { name: "Filter", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Filter" });
  await expect(dialog.getByRole("button", { name: "Schließen" })).toBeVisible();
});

test("it has no axe violations as cards, or with the sheet open", async ({ page }) => {
  await open(page, 400);
  let results = await new AxeBuilder({ page }).include(".dg-wrapper").analyze();
  expect(results.violations).toEqual([]);

  await filtersButton(page).click();
  await expect(sheet(page)).toBeVisible();
  results = await new AxeBuilder({ page }).include("[data-sheet]").analyze();
  expect(results.violations).toEqual([]);
});
