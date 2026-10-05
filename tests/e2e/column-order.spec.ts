import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 11: the column order. The playground declares Name, Email, Department
// and Age, in that order, and a switch makes the headers reorderable.

const grid = (page: Page) => page.getByRole("grid");
const header = (page: Page, name: string) => page.getByRole("columnheader", { name, exact: true });

/** The column labels in the order they are rendered. */
const headerOrder = (page: Page) =>
  grid(page).getByRole("columnheader").evaluateAll((cells) =>
    cells.map((cell) => cell.textContent?.trim().replace(/[▲▼]$/, "").trim()),
  );

/** The cells of the first body row, in the order they are rendered. */
const firstRowOrder = (page: Page) =>
  grid(page)
    .locator(".dg-body [role='row']")
    .first()
    .getByRole("gridcell")
    .evaluateAll((cells) => cells.map((cell) => cell.textContent?.trim()));

async function open(page: Page, { reorderable = true } = {}) {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.locator(".dg-count")).toHaveText("12 rows");
  if (reorderable) await page.getByTestId("toggle-reorder").check();
}

/** Focuses a header cell without sorting by it. */
async function focusHeader(page: Page, name: string) {
  await header(page, name).evaluate((cell: HTMLElement) => cell.focus());
  await expect(header(page, name)).toBeFocused();
}

test("without the switch a header is neither draggable nor movable", async ({ page }) => {
  await open(page, { reorderable: false });

  await expect(header(page, "Name")).not.toHaveAttribute("draggable", "true");
  const shortcuts = await header(page, "Name").getAttribute("aria-keyshortcuts");
  expect(shortcuts ?? "").not.toContain("Alt+Shift");

  await focusHeader(page, "Name");
  await page.keyboard.press("Alt+Shift+ArrowRight");
  expect(await headerOrder(page)).toEqual(["Name", "Email", "Department", "Age"]);
});

test("a header dragged onto another one takes its place", async ({ page, browserName }) => {
  // WebKit in Playwright starts no HTML drag from a mouse drag.
  test.skip(browserName === "webkit", "no HTML drag and drop under WebKit automation");
  await open(page);

  await expect(header(page, "Age")).toHaveAttribute("draggable", "true");
  await header(page, "Age").dragTo(header(page, "Name"));

  expect(await headerOrder(page)).toEqual(["Age", "Name", "Email", "Department"]);
  // Dragging the other way puts a column after its target, so it lands where
  // the pointer let go rather than one place short of it.
  await header(page, "Age").dragTo(header(page, "Email"));
  expect(await headerOrder(page)).toEqual(["Name", "Email", "Age", "Department"]);
});

test.describe("keyboard", () => {
  test("Alt+Shift+Arrow moves the column and keeps the focus on it", async ({ page }) => {
    await open(page);
    await focusHeader(page, "Name");

    await page.keyboard.press("Alt+Shift+ArrowRight");
    expect(await headerOrder(page)).toEqual(["Email", "Name", "Department", "Age"]);
    await expect(header(page, "Name")).toBeFocused();
    await expect(header(page, "Name")).toHaveAttribute("aria-colindex", "2");

    await page.keyboard.press("Alt+Shift+ArrowLeft");
    expect(await headerOrder(page)).toEqual(["Name", "Email", "Department", "Age"]);
    await expect(header(page, "Name")).toHaveAttribute("aria-colindex", "1");
  });

  test("a column at the end stays there", async ({ page }) => {
    await open(page);
    await focusHeader(page, "Name");

    await page.keyboard.press("Alt+Shift+ArrowLeft");
    expect(await headerOrder(page)).toEqual(["Name", "Email", "Department", "Age"]);
    await expect(header(page, "Name")).toBeFocused();
  });

  test("Alt without Shift still resizes", async ({ page }) => {
    await open(page);
    const width = await header(page, "Name").evaluate((el) => el.getBoundingClientRect().width);

    await focusHeader(page, "Name");
    await page.keyboard.press("Alt+ArrowRight");

    expect(await headerOrder(page)).toEqual(["Name", "Email", "Department", "Age"]);
    await expect
      .poll(() => header(page, "Name").evaluate((el) => el.getBoundingClientRect().width))
      .toBeGreaterThan(width);
  });
});

test("the cells follow their headers", async ({ page }) => {
  await open(page);
  const before = await firstRowOrder(page);

  await focusHeader(page, "Age");
  await page.keyboard.press("Alt+Shift+ArrowLeft");

  expect(await headerOrder(page)).toEqual(["Name", "Email", "Age", "Department"]);
  const after = await firstRowOrder(page);
  expect(after).toEqual([before[0], before[1], before[3], before[2]]);
});

test("a moved column survives a reload", async ({ page }) => {
  await open(page);
  await focusHeader(page, "Age");
  await page.keyboard.press("Alt+Shift+ArrowLeft");
  await page.keyboard.press("Alt+Shift+ArrowLeft");
  expect(await headerOrder(page)).toEqual(["Name", "Age", "Email", "Department"]);

  await page.reload();
  // The grid first, then its order: reading the headers is one shot without a
  // retry, and a WASM app is not on the page the instant the reload returns.
  await expect(page.locator(".dg-count")).toHaveText("12 rows");

  expect(await headerOrder(page)).toEqual(["Name", "Age", "Email", "Department"]);
});

test("hiding a column does not disturb the order around it", async ({ page }) => {
  await open(page);
  await focusHeader(page, "Age");
  await page.keyboard.press("Alt+Shift+ArrowLeft");
  expect(await headerOrder(page)).toEqual(["Name", "Email", "Age", "Department"]);

  await page.locator(".dg-columns").locator("summary").click();
  await page.locator(".dg-columns").getByRole("checkbox", { name: "Email" }).uncheck();
  expect(await headerOrder(page)).toEqual(["Name", "Age", "Department"]);

  await page.locator(".dg-columns").getByRole("checkbox", { name: "Email" }).check();
  expect(await headerOrder(page)).toEqual(["Name", "Email", "Age", "Department"]);
});

test("a reordered grid has no accessibility violations", async ({ page }) => {
  await open(page);
  await focusHeader(page, "Age");
  await page.keyboard.press("Alt+Shift+ArrowLeft");

  const results = await new AxeBuilder({ page }).include(".dg").analyze();
  expect(results.violations).toEqual([]);
});
