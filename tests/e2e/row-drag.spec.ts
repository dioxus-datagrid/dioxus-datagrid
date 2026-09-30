import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 12: dragging rows into another order. The playground puts a handle
// column first when `toggle-row-drag` is on.
//
// The gesture is built from pointer events, not HTML drag and drop, which
// WebKit does not deliver (docs/DECISIONS.md ADR-0037) — so the mouse is driven
// here step by step, which is also the only way Playwright can drive a real
// drag in both engines.

const grid = (page: Page) => page.getByRole("grid");
const bodyRows = (page: Page) => grid(page).locator('[role="row"]:has([role="gridcell"])');
const handle = (page: Page, row: number) => bodyRows(page).nth(row).locator("[data-row-handle]");

async function open(page: Page, rows: "none" | "single" | "multi" = "multi") {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.locator(".dg-count")).toHaveText("12 rows");
  await page.getByTestId(`selection-${rows}`).check();
  await page.getByTestId("toggle-row-drag").check();
  await expect(handle(page, 0)).toBeVisible();
}

/** The names in the Name column, in the order they are shown. */
const names = (page: Page) =>
  bodyRows(page).evaluateAll((rows) =>
    rows.map((row) => row.querySelectorAll('[role="gridcell"]')[1]?.textContent ?? ""),
  );

/** Drags the handle of one row onto another, in steps a browser believes. */
async function drag(page: Page, from: number, to: number) {
  const start = await handle(page, from).boundingBox();
  const target = await bodyRows(page).nth(to).boundingBox();
  if (!start || !target) {
    throw new Error("no box to drag from or to");
  }
  await page.mouse.move(start.x + start.width / 2, start.y + start.height / 2);
  await page.mouse.down();
  // Two moves: the first leaves the handle, the second arrives — a single jump
  // is a gesture no pointer ever makes.
  await page.mouse.move(start.x + start.width / 2, start.y + start.height / 2 + 4);
  await page.mouse.move(target.x + target.width / 2, target.y + target.height / 2, { steps: 4 });
}

test("a row can be dragged onto another", async ({ page }) => {
  await open(page);
  const before = await names(page);

  await drag(page, 0, 2);
  // While the pointer is down, both rows say what is happening.
  await expect(bodyRows(page).nth(0)).toHaveAttribute("data-dragging", "true");
  await expect(bodyRows(page).nth(2)).toHaveAttribute("data-drop", "after");
  await page.mouse.up();

  const after = await names(page);
  expect(after.slice(0, 3)).toEqual([before[1], before[2], before[0]]);
  await expect(bodyRows(page).nth(0)).not.toHaveAttribute("data-dragging", /.*/);
});

test("a row dragged upwards lands above the row it is over", async ({ page }) => {
  await open(page);
  const before = await names(page);

  await drag(page, 3, 1);
  await expect(bodyRows(page).nth(1)).toHaveAttribute("data-drop", "before");
  await page.mouse.up();

  const after = await names(page);
  expect(after.slice(0, 4)).toEqual([before[0], before[3], before[1], before[2]]);
});

test("letting go where it started moves nothing", async ({ page }) => {
  await open(page);
  const before = await names(page);

  await drag(page, 1, 1);
  await page.mouse.up();

  expect(await names(page)).toEqual(before);
});

test("Escape during a drag puts the row back", async ({ page }) => {
  await open(page);
  const before = await names(page);

  await drag(page, 0, 3);
  await expect(bodyRows(page).nth(3)).toHaveAttribute("data-drop", "after");
  await page.keyboard.press("Escape");
  await page.mouse.up();

  expect(await names(page)).toEqual(before);
  await expect(bodyRows(page).nth(0)).not.toHaveAttribute("data-dragging", /.*/);
});

test("Alt+Shift and an arrow move the focused row", async ({ page }) => {
  await open(page);
  const before = await names(page);

  await bodyRows(page).nth(1).getByRole("gridcell").nth(1).click();
  await page.keyboard.press("Alt+Shift+ArrowDown");

  const after = await names(page);
  expect(after.slice(0, 3)).toEqual([before[0], before[2], before[1]]);
  // The focus went with the row, so pressing again moves the same one.
  await expect(bodyRows(page).nth(2).getByRole("gridcell").nth(1)).toBeFocused();
  await page.keyboard.press("Alt+Shift+ArrowUp");
  expect(await names(page)).toEqual(before);
});

test("the last row on the page cannot be moved further down", async ({ page }) => {
  await open(page);
  const before = await names(page);
  const last = before.length - 1;

  await bodyRows(page).nth(last).getByRole("gridcell").nth(1).click();
  await page.keyboard.press("Alt+Shift+ArrowDown");

  // A move is counted in rows the user can see, and there is no row below this
  // one to trade places with.
  expect(await names(page)).toEqual(before);
});

test("a sorted grid offers no handles", async ({ page }) => {
  await open(page);
  await grid(page).getByRole("columnheader", { name: "Name" }).click();

  // The rows are in an order of the grid's making; there is nothing to drag.
  await expect(grid(page).locator("[data-row-handle]")).toHaveCount(0);

  // And the keyboard does not move them either.
  const sorted = await names(page);
  await bodyRows(page).nth(0).getByRole("gridcell").nth(1).click();
  await page.keyboard.press("Alt+Shift+ArrowDown");
  expect(await names(page)).toEqual(sorted);
});

test("dragging a row does not select it", async ({ page }) => {
  await open(page);

  await drag(page, 0, 2);
  await page.mouse.up();

  await expect(page.getByTestId("selected-keys")).toHaveText("selected: []");
});

test("the handle column has no axe violations", async ({ page }) => {
  await open(page);

  const results = await new AxeBuilder({ page }).include(".dg").analyze();
  expect(results.violations).toEqual([]);
});
