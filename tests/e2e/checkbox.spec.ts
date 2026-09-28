import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 12: the checkbox column. The playground puts it first when
// `toggle-checkbox-column` is on, so the Name column becomes the second.
//
// The header's box shows three states, and the third one — some rows selected,
// not all — is a DOM property no attribute can express, so it is asserted as
// one (docs/VERIFICATION.md 16).

const grid = (page: Page) => page.getByRole("grid");
const bodyRows = (page: Page) => grid(page).locator('[role="row"]:has([role="gridcell"])');
const selectAll = (page: Page) => grid(page).locator("[data-select-all]");
const rowBox = (page: Page, row: number) =>
  bodyRows(page).nth(row).getByRole("checkbox", { name: "Select row" });

async function open(page: Page, rows: "none" | "single" | "multi" = "multi") {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.locator(".dg-count")).toHaveText("12 rows");
  await page.getByTestId(`selection-${rows}`).check();
  await page.getByTestId("toggle-checkbox-column").check();
  await expect(bodyRows(page).first().getByRole("gridcell").first()).toBeVisible();
}

/** Whether the select-all box is showing the mixed state. */
const mixed = (page: Page) =>
  selectAll(page).evaluate((box) => (box as HTMLInputElement).indeterminate);

/** The keys the playground reports as selected. */
const selected = (page: Page) => page.locator('[data-testid="selected-keys"]');

test("a box in a row selects that row, and a second click lets it go", async ({ page }) => {
  await open(page);

  await rowBox(page, 0).click();
  await expect(rowBox(page, 0)).toBeChecked();
  await expect(bodyRows(page).first()).toHaveAttribute("aria-selected", "true");
  await expect(selected(page)).toHaveText("selected: [1]");

  // A checkbox toggles — unlike a click on a cell, which only ever adds.
  await rowBox(page, 0).click();
  await expect(rowBox(page, 0)).not.toBeChecked();
  await expect(selected(page)).toHaveText("selected: []");
});

test("Shift and a box reach from the row before to this one", async ({ page }) => {
  await open(page);

  await rowBox(page, 0).click();
  await rowBox(page, 2).click({ modifiers: ["Shift"] });

  await expect(rowBox(page, 1)).toBeChecked();
  await expect(selected(page)).toHaveText("selected: [1, 2, 3]");
});

test("the header box shows none, some and all of the page", async ({ page }) => {
  await open(page);
  await expect(selectAll(page)).toHaveAttribute("data-select-all", "none");
  await expect(selectAll(page)).not.toBeChecked();
  expect(await mixed(page)).toBe(false);

  await rowBox(page, 0).click();
  await expect(selectAll(page)).toHaveAttribute("data-select-all", "partial");
  await expect(selectAll(page)).not.toBeChecked();
  // The third state, which only the DOM property can carry.
  expect(await mixed(page)).toBe(true);

  await selectAll(page).click();
  await expect(selectAll(page)).toHaveAttribute("data-select-all", "all");
  await expect(selectAll(page)).toBeChecked();
  expect(await mixed(page)).toBe(false);
});

test("the header box selects every row on show, and clears them again", async ({ page }) => {
  await open(page);
  const rows = await bodyRows(page).count();

  await selectAll(page).click();
  await expect(page.locator('[role="row"][data-selected="true"]')).toHaveCount(rows);

  await selectAll(page).click();
  await expect(page.locator('[role="row"][data-selected="true"]')).toHaveCount(0);
  await expect(selected(page)).toHaveText("selected: []");
});

test("Space on the header cell toggles them all, without sorting", async ({ page }) => {
  await open(page);
  const names = await bodyRows(page)
    .locator('[role="gridcell"]')
    .nth(1)
    .textContent();

  // Focus the header cell of the checkbox column, as arrowing there would.
  await grid(page).getByRole("columnheader", { name: "Select all rows" }).evaluate((el: HTMLElement) => el.focus());
  await page.keyboard.press(" ");

  await expect(selectAll(page)).toBeChecked();
  // Nothing was sorted: the rows are in the same order.
  await expect(bodyRows(page).locator('[role="gridcell"]').nth(1)).toHaveText(names ?? "");
});

test("Space on a checkbox cell toggles its row, as anywhere else in the row", async ({ page }) => {
  await open(page);

  await bodyRows(page).nth(1).getByRole("gridcell").first().click();
  // The click toggled it on; Space toggles it off again.
  await expect(rowBox(page, 1)).toBeChecked();
  await page.keyboard.press(" ");
  await expect(rowBox(page, 1)).not.toBeChecked();
});

test("the header box speaks only about the page in front of the user", async ({ page }) => {
  await open(page);
  await page.getByTestId("toggle-paging").check();
  await expect(bodyRows(page)).toHaveCount(5);

  await selectAll(page).click();
  await expect(selectAll(page)).toHaveAttribute("data-select-all", "all");

  // The next page is untouched, so its box starts empty again.
  await page.getByRole("button", { name: "Next" }).click();
  await expect(selectAll(page)).toHaveAttribute("data-select-all", "none");
  await expect(rowBox(page, 0)).not.toBeChecked();

  // And the first page kept its rows.
  await page.getByRole("button", { name: "Previous" }).click();
  await expect(selectAll(page)).toHaveAttribute("data-select-all", "all");
});

test("a single selection gets boxes but no select all", async ({ page }) => {
  await open(page, "single");

  await expect(selectAll(page)).toHaveCount(0);
  await rowBox(page, 0).click();
  await rowBox(page, 1).click();

  // One row at a time, as the mode promises.
  await expect(rowBox(page, 0)).not.toBeChecked();
  await expect(rowBox(page, 1)).toBeChecked();
});

test("a grid that cannot select rows draws no boxes", async ({ page }) => {
  await open(page, "none");

  await expect(grid(page).getByRole("checkbox")).toHaveCount(0);
  // The column is still a column, with a cell in every row.
  await expect(bodyRows(page).first().getByRole("gridcell")).toHaveCount(5);
});

test("copying selected rows leaves the checkbox column out", async ({ page }) => {
  await open(page);
  await page.getByTestId("cell-selection-none").check();
  await page.evaluate(() => {
    const copied: string[] = [];
    (window as unknown as { __copied: string[] }).__copied = copied;
    document.execCommand = () => {
      copied.push(document.querySelector("textarea")?.value ?? "");
      return true;
    };
    if (navigator.clipboard) {
      navigator.clipboard.writeText = async (text: string) => {
        copied.push(text);
      };
    }
  });

  await rowBox(page, 0).click();
  await page.keyboard.press("Control+c");

  await expect
    .poll(async () => await page.evaluate(() => (window as unknown as { __copied: string[] }).__copied[0]))
    .toBe("Zoe Bauer\tzoe.bauer@example.com\tengineering\t30");
});

test("the checkbox column has no axe violations", async ({ page }) => {
  await open(page);
  await rowBox(page, 0).click();

  const results = await new AxeBuilder({ page }).include(".dg").analyze();
  expect(results.violations).toEqual([]);
});
