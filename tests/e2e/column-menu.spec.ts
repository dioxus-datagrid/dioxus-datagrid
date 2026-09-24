import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 11: the menu at a column header. The playground's columns are Name,
// Email, Department and Age.

const grid = (page: Page) => page.getByRole("grid");
const header = (page: Page, name: string) => page.getByRole("columnheader", { name, exact: true });
const trigger = (page: Page, name: string) =>
  header(page, name).locator("[data-column-menu-trigger]");
const panel = (page: Page) => page.locator("[data-column-menu-panel]");
const item = (page: Page, action: string) => page.locator(`[data-column-menu-action="${action}"]`);

/** The entries the open menu offers, by what they do. */
const actions = (page: Page) =>
  panel(page)
    .locator("[data-column-menu-item]")
    .evaluateAll((items) => items.map((el) => (el as HTMLElement).dataset.columnMenuAction));

async function open(page: Page) {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.locator(".dg-count")).toHaveText("12 rows");
  await page.getByTestId("toggle-column-menu").check();
}

async function openMenu(page: Page, column: string) {
  await trigger(page, column).click();
  await expect(panel(page)).toBeVisible();
}

/** Opens a menu the way the keyboard does: Alt+Down on the focused header. */
async function openMenuByKeyboard(page: Page, column: string) {
  await header(page, column).evaluate((el: HTMLElement) => el.focus());
  await page.keyboard.press("Alt+ArrowDown");
  await expect(panel(page)).toBeVisible();
}

test("every header has a menu, and it says what it opens", async ({ page }) => {
  await open(page);

  await expect(page.locator("[data-column-menu-trigger]")).toHaveCount(4);
  await expect(trigger(page, "Name")).toHaveAttribute("aria-label", "Column options for Name");
  await expect(trigger(page, "Name")).toHaveAttribute("aria-haspopup", "menu");
  await expect(trigger(page, "Name")).toHaveAttribute("aria-expanded", "false");

  await openMenu(page, "Name");
  await expect(trigger(page, "Name")).toHaveAttribute("aria-expanded", "true");
  await expect(panel(page)).toHaveAttribute("role", "menu");
});

test("a column offers only what would do something", async ({ page }) => {
  await open(page);
  await openMenu(page, "Name");

  // Nothing is sorted or pinned yet, so there is nothing to clear or unpin.
  expect(await actions(page)).toEqual([
    "sort-asc",
    "sort-desc",
    "group",
    "pin-start",
    "pin-end",
    "hide",
  ]);
});

test("sorting from the menu sorts the column", async ({ page }) => {
  await open(page);
  await openMenu(page, "Name");
  await item(page, "sort-desc").click();

  await expect(panel(page)).toHaveCount(0);
  await expect(header(page, "Name")).toHaveAttribute("aria-sort", "descending");

  // Now the menu says which direction holds, and offers to clear it.
  await openMenu(page, "Name");
  await expect(item(page, "sort-desc")).toHaveAttribute("aria-checked", "true");
  await expect(item(page, "sort-asc")).toHaveAttribute("aria-checked", "false");
  await item(page, "sort-clear").click();
  await expect(header(page, "Name")).toHaveAttribute("aria-sort", "none");
});

test("pinning from the menu holds the column at its edge", async ({ page }) => {
  await open(page);
  await openMenu(page, "Age");
  await item(page, "pin-end").click();

  await expect(header(page, "Age")).toHaveAttribute("data-pinned", "end");
  // Pinned to the end, it is laid out last.
  const labels = await grid(page)
    .getByRole("columnheader")
    .evaluateAll((cells) => cells.map((cell) => cell.textContent?.trim().split("\n")[0]));
  expect(labels[labels.length - 1]).toContain("Age");

  await openMenu(page, "Age");
  await expect(item(page, "pin-end")).toHaveAttribute("aria-checked", "true");
  await item(page, "unpin").click();
  await expect(header(page, "Age")).not.toHaveAttribute("data-pinned", /.*/);
});

test("hiding from the menu takes the column out", async ({ page }) => {
  await open(page);
  await expect(grid(page)).toHaveAttribute("aria-colcount", "4");

  await openMenu(page, "Email");
  await item(page, "hide").click();

  await expect(grid(page)).toHaveAttribute("aria-colcount", "3");
  await expect(header(page, "Email")).toHaveCount(0);
});

test("grouping from the menu groups the rows", async ({ page }) => {
  await open(page);
  // All twelve rows on one page, so every group's header is in the DOM.
  await page.getByTestId("toggle-paging").uncheck();
  await openMenu(page, "Department");
  await item(page, "group").click();

  // Grouped, the grid is a treegrid, so it is no longer found by role "grid".
  const root = page.locator(".dg");
  await expect(root).toHaveAttribute("role", "treegrid");
  await expect(root.locator("[data-group-row]")).toHaveCount(6);

  // The menu now offers the way back.
  await openMenu(page, "Department");
  await expect(item(page, "ungroup")).toBeVisible();
  await item(page, "ungroup").click();
  await expect(root).toHaveAttribute("role", "grid");
});

test("fitting the width is offered once the column has been resized", async ({ page }) => {
  await open(page);
  await openMenu(page, "Name");
  await expect(item(page, "fit-width")).toHaveCount(0);
  await page.keyboard.press("Escape");

  // Widen it from the keyboard, which is what sets a width in the state.
  await header(page, "Name").evaluate((el: HTMLElement) => el.focus());
  await page.keyboard.press("Alt+ArrowRight");
  const wide = await header(page, "Name").evaluate((el) => el.getBoundingClientRect().width);

  await openMenu(page, "Name");
  await item(page, "fit-width").click();
  await expect
    .poll(() => header(page, "Name").evaluate((el) => el.getBoundingClientRect().width))
    .toBeLessThan(wide);
});

test.describe("keyboard", () => {
  test("the arrows walk the entries and wrap", async ({ page }) => {
    await open(page);
    await openMenu(page, "Name");
    await item(page, "sort-asc").focus();

    await page.keyboard.press("ArrowDown");
    await expect(item(page, "sort-desc")).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(item(page, "sort-asc")).toBeFocused();
    // Up from the first wraps to the last, as a menu does.
    await page.keyboard.press("ArrowUp");
    await expect(item(page, "hide")).toBeFocused();
    await page.keyboard.press("Home");
    await expect(item(page, "sort-asc")).toBeFocused();
  });

  test("Alt+Down opens the menu and Escape gives the header its focus back", async ({ page }) => {
    await open(page);
    await openMenuByKeyboard(page, "Name");
    // Opened from the keyboard, the focus lands on the first entry.
    await expect(item(page, "sort-asc")).toBeFocused();

    await page.keyboard.press("Escape");
    await expect(panel(page)).toHaveCount(0);
    await expect(header(page, "Name")).toBeFocused();
  });

  test("the button is no second tab stop", async ({ page }) => {
    await open(page);
    // The grid is one tab stop; a focusable button in every header would be
    // four more.
    await expect(trigger(page, "Name")).toHaveAttribute("tabindex", "-1");
    await expect(grid(page).locator('[tabindex="0"]')).toHaveCount(1);
  });

  test("a click outside closes it too", async ({ page }) => {
    await open(page);
    await openMenu(page, "Name");

    await page.locator("[data-column-menu-backdrop]").click({ position: { x: 5, y: 5 } });
    await expect(panel(page)).toHaveCount(0);
  });
});

test("an open column menu has no accessibility violations", async ({ page }) => {
  await open(page);
  await openMenu(page, "Name");

  const results = await new AxeBuilder({ page }).include(".dg").analyze();
  expect(results.violations).toEqual([]);
});
