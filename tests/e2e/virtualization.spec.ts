import { expect, type Page, test } from "@playwright/test";

// Phase 4 acceptance: the playground's virtualized mode renders 100,000 rows at
// a fixed 40px, with the component's default overscan of 5.

const TOTAL = 100_000;
const ROW_HEIGHT = 40;
const OVERSCAN = 5;

const grid = (page: Page) => page.getByRole("grid");
const bodyRows = (page: Page) => page.locator(".dg-body [role='row']");

/** Scroll container geometry, as the grid sees it. */
async function geometry(page: Page) {
  return grid(page).evaluate((root) => {
    const header = root.querySelector(".dg-head") as HTMLElement;
    return {
      scrollTop: root.scrollTop,
      scrollHeight: root.scrollHeight,
      clientHeight: root.clientHeight,
      headerHeight: header.getBoundingClientRect().height,
    };
  });
}

/** The most rows the plan allows in the DOM: visible rows plus overscan on both sides. */
async function maxRenderedRows(page: Page) {
  const { clientHeight, headerHeight } = await geometry(page);
  const visible = Math.ceil((clientHeight - headerHeight) / ROW_HEIGHT) + 1;
  return visible + 2 * OVERSCAN;
}

/** Where DOM focus is, described through the ARIA attributes. */
async function active(page: Page) {
  return page.evaluate(() => {
    const el = document.activeElement as HTMLElement;
    return {
      role: el.getAttribute("role"),
      row: el.closest("[role='row']")?.getAttribute("aria-rowindex") ?? null,
    };
  });
}

/** Whether the focused cell sits fully inside the visible body area. */
async function focusedCellIsVisible(page: Page) {
  return page.evaluate(() => {
    const cell = document.activeElement as HTMLElement;
    const root = cell.closest("[role='grid']") as HTMLElement;
    const header = root.querySelector(".dg-head") as HTMLElement;
    const c = cell.getBoundingClientRect();
    const r = root.getBoundingClientRect();
    const top = r.top + header.getBoundingClientRect().height;
    return c.top >= top - 1 && c.bottom <= r.top + root.clientHeight + 1;
  });
}

async function scrollTo(page: Page, top: number) {
  await grid(page).evaluate((root, value) => {
    root.scrollTop = value;
  }, top);
}

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await page.getByTestId("toggle-virtualized").check();
  await expect(grid(page)).toHaveAttribute("aria-rowcount", String(TOTAL + 1));
});

test.describe("rendering", () => {
  test("the scrollbar reflects every row", async ({ page }) => {
    const { scrollHeight, headerHeight } = await geometry(page);
    expect(scrollHeight).toBeCloseTo(TOTAL * ROW_HEIGHT + headerHeight, 0);
  });

  test("renders no more than the visible rows plus overscan", async ({ page }) => {
    const limit = await maxRenderedRows(page);

    await expect(async () => {
      const count = await bodyRows(page).count();
      expect(count).toBeGreaterThan(OVERSCAN);
      expect(count).toBeLessThanOrEqual(limit);
    }).toPass();

    // And still so deep in the data.
    await scrollTo(page, 50_000 * ROW_HEIGHT);
    await expect(bodyRows(page).first()).toHaveAttribute(
      "aria-rowindex",
      String(50_000 - OVERSCAN + 2),
    );
    expect(await bodyRows(page).count()).toBeLessThanOrEqual(limit);
  });

  test("rows render at exactly the fixed height", async ({ page }) => {
    const heights = await bodyRows(page).evaluateAll((rows) =>
      rows.map((row) => row.getBoundingClientRect().height),
    );
    expect(new Set(heights)).toEqual(new Set([ROW_HEIGHT]));
  });

  test("aria-rowindex stays tied to the data while scrolling", async ({ page }) => {
    await scrollTo(page, 10_000 * ROW_HEIGHT);

    // Row index 9_999 in the view is employee 10_000, and aria-rowindex counts
    // the header, so it is announced as row 10_001.
    const row = page.locator(".dg-body [role='row'][aria-rowindex='10001']");
    await expect(row).toHaveCount(1);
    await expect(row.locator("[aria-colindex='1']")).toHaveText(/ 10000$/);
  });
});

test.describe("keyboard across the viewport edge", () => {
  test("arrowing past the bottom scrolls and keeps focus on the right cell", async ({ page }) => {
    await grid(page).locator("[tabindex='0']").focus();

    for (let i = 0; i < 30; i++) await page.keyboard.press("ArrowDown");

    // 30 presses from the header land on body row 30, aria-rowindex 31.
    await expect.poll(() => active(page)).toEqual({ role: "gridcell", row: "31" });
    expect(await focusedCellIsVisible(page)).toBe(true);
    expect((await geometry(page)).scrollTop).toBeGreaterThan(0);
    expect(await bodyRows(page).count()).toBeLessThanOrEqual(await maxRenderedRows(page));
  });

  test("PageDown moves by one viewport of rows", async ({ page }) => {
    await grid(page).locator("[tabindex='0']").focus();
    await page.keyboard.press("ArrowDown");
    await expect.poll(() => active(page)).toEqual({ role: "gridcell", row: "2" });

    const { clientHeight, headerHeight } = await geometry(page);
    const perViewport = Math.floor((clientHeight - headerHeight) / ROW_HEIGHT);

    await page.keyboard.press("PageDown");
    await expect.poll(() => active(page)).toEqual({ role: "gridcell", row: String(2 + perViewport) });
    expect(await focusedCellIsVisible(page)).toBe(true);
  });

  test("Ctrl+End reaches the last of 100,000 rows", async ({ page }) => {
    await grid(page).locator("[tabindex='0']").focus();
    await page.keyboard.press("Control+End");

    await expect.poll(() => active(page)).toEqual({ role: "gridcell", row: String(TOTAL + 1) });
    expect(await focusedCellIsVisible(page)).toBe(true);

    await page.keyboard.press("Control+Home");
    await expect.poll(() => active(page)).toEqual({ role: "columnheader", row: "1" });
  });

  test("scrolling the focused row away keeps the keyboard working", async ({ page }) => {
    await grid(page).locator("[tabindex='0']").focus();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    await expect.poll(() => active(page)).toEqual({ role: "gridcell", row: "3" });

    // Scroll far away with the mouse wheel's equivalent; the focused row leaves
    // the DOM.
    await scrollTo(page, 40_000 * ROW_HEIGHT);
    await expect(page.locator(".dg-body [role='row'][aria-rowindex='3']")).toHaveCount(0);

    // Focus is parked on the grid rather than dropped to the page, and the grid
    // is still exactly one tab stop.
    await expect.poll(() => active(page)).toEqual({ role: "grid", row: null });
    await expect(grid(page)).toHaveAttribute("tabindex", "0");
    await expect(grid(page).locator("[tabindex='0']")).toHaveCount(0);

    // The next arrow key continues from where the focus was, not from the view.
    await page.keyboard.press("ArrowDown");
    await expect.poll(() => active(page)).toEqual({ role: "gridcell", row: "4" });
    expect(await focusedCellIsVisible(page)).toBe(true);
  });

  test("tabbing back in returns to the focused row", async ({ page }) => {
    await grid(page).locator("[tabindex='0']").focus();
    for (let i = 0; i < 5; i++) await page.keyboard.press("ArrowDown");
    await expect.poll(() => active(page)).toEqual({ role: "gridcell", row: "6" });

    // Leave the grid, scroll the focused row away, then tab back in.
    await page.getByRole("textbox", { name: "Filter Department" }).focus();
    await scrollTo(page, 20_000 * ROW_HEIGHT);
    await expect(page.locator(".dg-body [role='row'][aria-rowindex='6']")).toHaveCount(0);

    await page.keyboard.press("Tab");
    await expect.poll(() => active(page)).toEqual({ role: "gridcell", row: "6" });
    expect(await focusedCellIsVisible(page)).toBe(true);
  });
});
