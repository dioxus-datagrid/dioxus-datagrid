import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 12: selecting cells, and rectangles of them. The playground's columns
// are Name, Email, Department and Age; rows are selectable at the same time,
// which is the point — the two selections are separate.

const grid = (page: Page) => page.getByRole("grid");
const selectedCells = (page: Page) => page.locator("[data-cell-selected]");

/** The cell at a row and column of the body, both zero-based. */
const cell = (page: Page, row: number, column: number) =>
  grid(page).locator('[role="row"]:has([role="gridcell"])').nth(row).getByRole("gridcell").nth(column);

async function open(
  page: Page,
  mode: "none" | "single" | "range" = "range",
  rows: "none" | "single" | "multi" = "multi",
) {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.locator(".dg-count")).toHaveText("12 rows");
  await page.getByTestId(`cell-selection-${mode}`).check();
  await page.getByTestId(`selection-${rows}`).check();
}

/** Every selected cell as "row,col" from its aria attributes, for readability. */
const shape = (page: Page) =>
  selectedCells(page).evaluateAll((cells) =>
    cells.map((el) => {
      const row = el.closest('[role="row"]')?.getAttribute("aria-rowindex");
      return `${row},${el.getAttribute("aria-colindex")}`;
    }),
  );

test("a grid that does not select cells says nothing about them", async ({ page }) => {
  await open(page, "none", "none");

  await cell(page, 0, 0).click();
  await expect(selectedCells(page)).toHaveCount(0);
  await expect(cell(page, 0, 0)).not.toHaveAttribute("aria-selected", /.*/);
  await expect(grid(page)).not.toHaveAttribute("aria-multiselectable", "true");
});

test("a click selects that one cell", async ({ page }) => {
  await open(page);

  await cell(page, 1, 1).click();
  await expect(selectedCells(page)).toHaveCount(1);
  await expect(cell(page, 1, 1)).toHaveAttribute("aria-selected", "true");
  // Every other cell says where it stands, rather than staying silent.
  await expect(cell(page, 1, 2)).toHaveAttribute("aria-selected", "false");
  await expect(grid(page)).toHaveAttribute("aria-multiselectable", "true");
});

test("Shift and the arrows grow a rectangle from the anchor", async ({ page }) => {
  await open(page);
  await cell(page, 1, 1).click();

  await page.keyboard.press("Shift+ArrowRight");
  await page.keyboard.press("Shift+ArrowDown");
  // Two rows by two columns, starting at the clicked cell.
  expect(await shape(page)).toEqual(["3,2", "3,3", "4,2", "4,3"]);

  // Back towards the anchor shrinks the same rectangle instead of starting one.
  await page.keyboard.press("Shift+ArrowUp");
  expect(await shape(page)).toEqual(["3,2", "3,3"]);
});

test("a rectangle can be grown upwards and to the left", async ({ page }) => {
  await open(page);
  await cell(page, 2, 2).click();

  await page.keyboard.press("Shift+ArrowLeft");
  await page.keyboard.press("Shift+ArrowUp");
  // The anchor is the bottom right corner now; the rectangle reads the same way.
  expect(await shape(page)).toEqual(["3,2", "3,3", "4,2", "4,3"]);
});

test("an arrow without Shift starts over at the cell it reached", async ({ page }) => {
  await open(page);
  await cell(page, 0, 0).click();
  await page.keyboard.press("Shift+ArrowDown");
  await expect(selectedCells(page)).toHaveCount(2);

  await page.keyboard.press("ArrowDown");
  expect(await shape(page)).toEqual(["4,1"]);
});

test("a shift-click reaches from the anchor to the cell clicked", async ({ page }) => {
  await open(page);
  await cell(page, 0, 0).click();

  await cell(page, 2, 2).click({ modifiers: ["Shift"] });
  expect(await shape(page)).toEqual(["2,1", "2,2", "2,3", "3,1", "3,2", "3,3", "4,1", "4,2", "4,3"]);
});

test("single mode keeps the selection on one cell", async ({ page }) => {
  await open(page, "single", "none");
  await cell(page, 0, 0).click();

  await page.keyboard.press("Shift+ArrowDown");
  // The focus moved, and the selection came along — but alone.
  await expect(selectedCells(page)).toHaveCount(1);
  expect(await shape(page)).toEqual(["3,1"]);
  await expect(grid(page)).not.toHaveAttribute("aria-multiselectable", "true");
});

test("the header cannot be selected", async ({ page }) => {
  await open(page);
  await cell(page, 0, 0).click();
  const before = await shape(page);

  // Up from the first row is the column header, which is not a cell.
  await page.keyboard.press("Shift+ArrowUp");
  await expect(page.getByRole("columnheader", { name: "Name" })).toBeFocused();
  await expect(selectedCells(page)).toHaveCount(before.length);
});

test("rows and cells are selected separately", async ({ page }) => {
  await open(page);

  // A click selects the cell and, because the playground selects rows too, the
  // row. They are different selections and say so differently.
  await cell(page, 1, 1).click();
  const row = grid(page).locator('[role="row"]:has([role="gridcell"])').nth(1);
  await expect(row).toHaveAttribute("data-selected", "true");
  await expect(selectedCells(page)).toHaveCount(1);

  // Space is the row's key and leaves the cell rectangle alone.
  await page.keyboard.press(" ");
  await expect(row).toHaveAttribute("data-selected", "false");
  await expect(selectedCells(page)).toHaveCount(1);
});

test("a selected rectangle has no accessibility violations", async ({ page }) => {
  await open(page);
  await cell(page, 0, 0).click();
  await page.keyboard.press("Shift+ArrowRight");
  await page.keyboard.press("Shift+ArrowDown");

  const results = await new AxeBuilder({ page }).include(".dg").analyze();
  expect(results.violations).toEqual([]);
});
