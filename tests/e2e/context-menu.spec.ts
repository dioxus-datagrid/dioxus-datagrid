import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 14: the context menu, a right click or Shift+F10 on a cell or a header.
// The playground mounts data_grid_context_menu with one entry of its own on a
// cell, "Write an email", which logs the row it was chosen on.

const grid = (page: Page) => page.getByRole("grid");
const menu = (page: Page) => page.getByRole("menu", { name: "Context menu" });
const item = (page: Page, name: string | RegExp) => menu(page).getByRole("menuitem", { name, exact: true });
const cell = (page: Page, row: number, column: number) =>
  page.locator(`.dg-body [role='row'] [aria-colindex='${column}']`).nth(row);
const log = (page: Page) => page.getByTestId("context-log");

async function open(page: Page, extra: (page: Page) => Promise<void> = async () => {}) {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.getByTestId("toggle-context-menu").check();
  await extra(page);
  await expect(grid(page)).toBeVisible();
}

/** The names of the entries of the open menu, in order. */
const entries = (page: Page) => menu(page).locator("[data-context-menu-item]").allTextContents();

test("a right click on a cell opens the menu at the pointer", async ({ page }) => {
  await open(page);
  const target = cell(page, 1, 2);
  const box = (await target.boundingBox())!;
  await target.click({ button: "right", position: { x: 20, y: 10 } });

  await expect(menu(page)).toBeVisible();
  const panel = (await menu(page).boundingBox())!;
  // At the pointer: the click was 20px in and 10px down.
  expect(Math.abs(panel.x - (box.x + 20))).toBeLessThan(3);
  expect(Math.abs(panel.y - (box.y + 10))).toBeLessThan(3);

  // What the grid offers on a cell, then what the application added.
  expect(await entries(page)).toEqual(["Copy", "Copy row", "Select row", "Write an email"]);
  // The first entry has the focus, as in any menu.
  await expect(item(page, "Copy")).toBeFocused();
});

test("it opens nothing, and leaves the browser's menu, where it is not mounted", async ({ page }) => {
  await page.goto("/");
  await cell(page, 0, 1).click({ button: "right" });
  await expect(page.getByRole("menu")).toHaveCount(0);
});

test("Escape closes it and puts the focus back on the cell", async ({ page }) => {
  await open(page);
  await cell(page, 2, 1).click({ button: "right" });
  await expect(menu(page)).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(menu(page)).toHaveCount(0);
  await expect(cell(page, 2, 1)).toBeFocused();
});

test("a click outside closes it", async ({ page }) => {
  await open(page);
  await cell(page, 0, 1).click({ button: "right" });
  await expect(menu(page)).toBeVisible();
  await page.mouse.click(5, 5);
  await expect(menu(page)).toHaveCount(0);
});

test("the arrow keys move through the entries and wrap", async ({ page }) => {
  await open(page);
  await cell(page, 0, 1).click({ button: "right" });
  await expect(item(page, "Copy")).toBeFocused();
  await page.keyboard.press("ArrowUp");
  await expect(item(page, "Write an email")).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(item(page, "Copy")).toBeFocused();
  await page.keyboard.press("End");
  await expect(item(page, "Write an email")).toBeFocused();
  await page.keyboard.press("Home");
  await expect(item(page, "Copy")).toBeFocused();
});

test("Shift+F10 opens it at the focused cell, and Enter chooses an entry", async ({ page }) => {
  // Tall, so that there is room below the cell and the menu is not moved back up.
  await page.setViewportSize({ width: 1280, height: 1200 });
  await open(page);
  await cell(page, 2, 1).click();
  await page.keyboard.press("Shift+F10");
  await expect(menu(page)).toBeVisible();

  // Opened at the cell: just below its start.
  const box = (await cell(page, 2, 1).boundingBox())!;
  const panel = (await menu(page).boundingBox())!;
  expect(Math.abs(panel.x - box.x)).toBeLessThan(3);
  expect(Math.abs(panel.y - (box.y + box.height))).toBeLessThan(3);

  await page.keyboard.press("End");
  await page.keyboard.press("Enter");
  await expect(menu(page)).toHaveCount(0);
  // The entry of the application's was called with the row it was chosen on.
  await expect(log(page)).toHaveText("context: [2]");
  await expect(cell(page, 2, 1)).toBeFocused();
});

test("the menu key opens it too", async ({ page }) => {
  await open(page);
  await cell(page, 0, 1).click();
  await page.keyboard.press("ContextMenu");
  await expect(menu(page)).toBeVisible();
});

test("Select row selects the row it was opened on, and offers to undo it", async ({ page }) => {
  await open(page);
  await cell(page, 1, 1).click({ button: "right" });
  await item(page, "Select row").click();
  await expect(menu(page)).toHaveCount(0);
  await expect(page.locator(".dg-body [role='row']").nth(1)).toHaveAttribute("aria-selected", "true");

  await cell(page, 1, 1).click({ button: "right" });
  await expect(item(page, "Deselect row")).toBeVisible();
});

test("a header offers what the column menu offers", async ({ page }) => {
  await open(page);
  await page.getByRole("columnheader", { name: "Age", exact: true }).click({ button: "right" });
  await expect(menu(page)).toBeVisible();
  const names = await entries(page);
  expect(names).toEqual(expect.arrayContaining(["Sort ascending", "Sort descending"]));

  // The sort directions are a choice, so they are radio items, as in the column menu.
  await menu(page).getByRole("menuitemradio", { name: "Sort ascending", exact: true }).click();
  await expect(page.getByRole("columnheader", { name: "Age", exact: true })).toHaveAttribute("aria-sort", "ascending");
});

test("Shift+F10 on a header opens the same menu", async ({ page }) => {
  await open(page);
  await page.getByRole("columnheader", { name: "Name", exact: true }).click();
  await page.keyboard.press("Shift+F10");
  await expect(menu(page)).toBeVisible();
  expect(await entries(page)).toContain("Sort ascending");
});

test("with editing on, a cell also offers Edit and Delete", async ({ page }) => {
  await open(page, async (page) => {
    await page.getByRole("radio", { name: "Cell", exact: true }).first().check();
  });
  await cell(page, 0, 1).click({ button: "right" });
  const names = await entries(page);
  expect(names).toEqual(expect.arrayContaining(["Edit", "Delete", "Write an email"]));
  // The grid's own entries first, the application's last.
  expect(names.at(-1)).toBe("Write an email");
});

test("it stays inside the window near the bottom right corner", async ({ page }) => {
  await open(page);
  const size = page.viewportSize()!;
  // Open it as near the corner as a click can land on the grid.
  const target = cell(page, 4, 3);
  const box = (await target.boundingBox())!;
  await page.setViewportSize({ width: Math.round(box.x + box.width / 2 + 40), height: Math.round(box.y + box.height / 2 + 40) });
  await target.scrollIntoViewIfNeeded();
  await target.click({ button: "right" });
  await expect(menu(page)).toBeVisible();
  // Let the panel be measured and moved back.
  await page.waitForTimeout(400);
  const panel = (await menu(page).boundingBox())!;
  const view = page.viewportSize()!;
  expect(panel.x).toBeGreaterThanOrEqual(0);
  expect(panel.y).toBeGreaterThanOrEqual(0);
  expect(panel.x + panel.width).toBeLessThanOrEqual(view.width + 1);
  expect(panel.y + panel.height).toBeLessThanOrEqual(view.height + 1);
  expect(size.width).toBeGreaterThan(0);
});

test("right to left: the same menu, inside the window", async ({ page }) => {
  await open(page, async (page) => {
    await page.getByTestId("toggle-rtl").check();
  });
  await cell(page, 1, 2).click({ button: "right" });
  await expect(menu(page)).toBeVisible();
  const panel = (await menu(page).boundingBox())!;
  const view = page.viewportSize()!;
  expect(panel.x).toBeGreaterThanOrEqual(0);
  expect(panel.x + panel.width).toBeLessThanOrEqual(view.width + 1);
  expect(await entries(page)).toContain("Copy row");
});

test("it has no axe violations while open", async ({ page }) => {
  await open(page);
  await cell(page, 0, 1).click({ button: "right" });
  await expect(menu(page)).toBeVisible();
  const results = await new AxeBuilder({ page }).include("[data-context-menu]").analyze();
  expect(results.violations).toEqual([]);
});
