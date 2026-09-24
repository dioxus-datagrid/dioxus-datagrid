import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 11: one cell over several columns. The playground lets the Name column
// run across Email and Department for anyone under 26 — two of the twelve
// people, "adam Fischer" (25) and "Ava Richter" (23).

const grid = (page: Page) => page.getByRole("grid");
const header = (page: Page, name: string) => page.getByRole("columnheader", { name, exact: true });

/** The row of one person, by the name its first cell shows. */
const rowOf = (page: Page, name: string) =>
  grid(page).getByRole("row").filter({ hasText: name }).first();

/** Opens the playground with every row on one page and the spans switched on. */
async function open(page: Page, { spanning = true } = {}) {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.locator(".dg-count")).toHaveText("12 rows");
  await page.getByTestId("toggle-paging").uncheck();
  if (spanning) await page.getByTestId("toggle-span").check();
}

/** The cells of a row as `[aria-colindex, aria-colspan]`. */
const cellsOf = (page: Page, name: string) =>
  rowOf(page, name)
    .getByRole("gridcell")
    .evaluateAll((cells) =>
      cells.map((cell) => [
        cell.getAttribute("aria-colindex"),
        cell.getAttribute("aria-colspan"),
      ]),
    );

test("a row that spans nothing keeps a cell per column", async ({ page }) => {
  await open(page);

  expect(await cellsOf(page, "Zoe Bauer")).toEqual([
    ["1", null],
    ["2", null],
    ["3", null],
    ["4", null],
  ]);
});

test("a wide cell replaces the columns it covers", async ({ page }) => {
  await open(page);

  // Three columns in one cell, then Age on its own.
  expect(await cellsOf(page, "adam Fischer")).toEqual([
    ["1", "3"],
    ["4", null],
  ]);
  await expect(rowOf(page, "adam Fischer")).toContainText("still in onboarding");
});

test("the wide cell is as wide as the columns it covers", async ({ page }) => {
  await open(page);

  const box = async (locator: ReturnType<typeof rowOf>, index: number) =>
    locator.getByRole("gridcell").nth(index).evaluate((el) => {
      const rect = el.getBoundingClientRect();
      return { left: Math.round(rect.left), right: Math.round(rect.right) };
    });

  // It starts where Name starts and ends where Department ends.
  const wide = await box(rowOf(page, "adam Fischer"), 0);
  const plain = rowOf(page, "Zoe Bauer");
  const name = await box(plain, 0);
  const department = await box(plain, 2);

  expect(wide.left).toBe(name.left);
  expect(wide.right).toBe(department.right);
});

test("switching the spans off gives the columns back", async ({ page }) => {
  await open(page, { spanning: false });

  expect(await cellsOf(page, "adam Fischer")).toHaveLength(4);
  await page.getByTestId("toggle-span").check();
  expect(await cellsOf(page, "adam Fischer")).toHaveLength(2);
});

test("a wide cell does not change the header or the row count", async ({ page }) => {
  await open(page);

  await expect(grid(page)).toHaveAttribute("aria-colcount", "4");
  await expect(grid(page)).toHaveAttribute("aria-rowcount", "13");
  await expect(header(page, "Department")).toBeVisible();
});

test.describe("keyboard", () => {
  /** What the focused cell holds, and how far it reaches. */
  const focused = (page: Page) =>
    page.evaluate(() => {
      const cell = document.activeElement as HTMLElement | null;
      return {
        text: cell?.textContent?.trim() ?? "",
        colindex: cell?.getAttribute("aria-colindex"),
        colspan: cell?.getAttribute("aria-colspan"),
      };
    });

  test("the arrows treat a wide cell as one stop", async ({ page }) => {
    await open(page);
    // Sorted by name, "adam Fischer" is the first row.
    await header(page, "Name").click();
    await rowOf(page, "adam Fischer").getByRole("gridcell").first().click();
    expect(await focused(page)).toMatchObject({ colindex: "1", colspan: "3" });

    // One press to the right leaves the cell rather than walking the columns
    // it covers.
    await page.keyboard.press("ArrowRight");
    expect(await focused(page)).toMatchObject({ colindex: "4", colspan: null });

    await page.keyboard.press("ArrowLeft");
    expect(await focused(page)).toMatchObject({ colindex: "1", colspan: "3" });
  });

  test("moving down onto a covered column lands on the wide cell", async ({ page }) => {
    await open(page);

    // Unsorted, "adam Fischer" is the second row; the Department cell above it
    // is the third column, which their wide cell covers.
    await rowOf(page, "Zoe Bauer").getByRole("gridcell").nth(2).click();
    expect(await focused(page)).toMatchObject({ colindex: "3" });

    await page.keyboard.press("ArrowDown");

    const landed = await focused(page);
    expect(landed.colindex).toBe("1");
    expect(landed.text).toContain("still in onboarding");
  });

  test("End lands on the last cell, not on a covered column", async ({ page }) => {
    await open(page);
    await header(page, "Name").click();
    await rowOf(page, "adam Fischer").getByRole("gridcell").first().click();

    await page.keyboard.press("End");
    expect(await focused(page)).toMatchObject({ colindex: "4" });
    await page.keyboard.press("Home");
    expect(await focused(page)).toMatchObject({ colindex: "1", colspan: "3" });
  });

  test("the roving tabindex stays on exactly one cell", async ({ page }) => {
    await open(page);
    await rowOf(page, "adam Fischer").getByRole("gridcell").first().click();

    await expect(grid(page).locator('[tabindex="0"]')).toHaveCount(1);
    await page.keyboard.press("ArrowRight");
    await expect(grid(page).locator('[tabindex="0"]')).toHaveCount(1);
  });
});

test("a grid with wide cells has no accessibility violations", async ({ page }) => {
  await open(page);

  const results = await new AxeBuilder({ page }).include(".dg").analyze();
  expect(results.violations).toEqual([]);
});
