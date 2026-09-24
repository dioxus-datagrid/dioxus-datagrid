import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 11: multi-level column headers. The playground puts Name and Email
// under "Person" and Department and Age under "Work". The rows are derived from
// the columns' own groups, so reordering or hiding a column has to rearrange
// them.

const grid = (page: Page) => page.getByRole("grid");
const header = (page: Page, name: string) => page.getByRole("columnheader", { name, exact: true });
const groupRow = (page: Page, level = 0) => page.locator(`[data-group-header-row="${level}"]`);

/** The cells of a group header row as `[label, colindex, colspan]`. */
const groupCells = (page: Page, level = 0) =>
  groupRow(page, level)
    .getByRole("columnheader")
    .evaluateAll((cells) =>
      cells.map((cell) => [
        cell.textContent?.trim() ?? "",
        cell.getAttribute("aria-colindex"),
        cell.getAttribute("aria-colspan"),
      ]),
    );

async function open(page: Page) {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.locator(".dg-count")).toHaveText("12 rows");
  await page.getByTestId("toggle-header-groups").check();
  await expect(groupRow(page)).toBeVisible();
}

/** Shows or hides a column through the picker. */
async function setColumn(page: Page, name: string, shown: boolean) {
  const picker = page.locator(".dg-columns");
  await picker.locator("summary").click();
  await picker.getByRole("checkbox", { name }).setChecked(shown);
  await picker.locator("summary").click();
}

test("a group covers the columns that name it", async ({ page }) => {
  await open(page);

  expect(await groupCells(page)).toEqual([
    ["Person", "1", "2"],
    ["Work", "3", "2"],
  ]);
  // One group row above the four column headers.
  await expect(groupRow(page)).toHaveAttribute("aria-rowindex", "1");
  await expect(page.locator('[role="row"][aria-rowindex="2"]')).toContainText("Name");
});

test("the header row counts towards the rows of the grid", async ({ page }) => {
  await open(page);
  await page.getByTestId("toggle-paging").uncheck();

  // Twelve rows under two header rows.
  await expect(grid(page)).toHaveAttribute("aria-rowcount", "14");
  const first = grid(page).getByRole("row").filter({ hasText: "Zoe Bauer" });
  await expect(first).toHaveAttribute("aria-rowindex", "3");
});

test("hiding a column narrows the group above it", async ({ page }) => {
  await open(page);
  await setColumn(page, "Email", false);

  expect(await groupCells(page)).toEqual([
    ["Person", "1", null],
    ["Work", "2", "2"],
  ]);
});

test("a column without a group gets an empty cell above it", async ({ page }) => {
  await open(page);
  await setColumn(page, "Salary", true);

  // Salary names no group, so the row still covers every column.
  expect(await groupCells(page)).toEqual([
    ["Person", "1", "2"],
    ["Work", "3", "2"],
    ["", "5", null],
  ]);
});

test("moving a column out of its group takes it out of the header too", async ({ page }) => {
  await open(page);
  await page.getByTestId("toggle-reorder").check();

  // Email is the second column, under "Person". Moved right twice it sits
  // between the two columns of "Work", which splits that group in two.
  await header(page, "Email").evaluate((el: HTMLElement) => el.focus());
  await page.keyboard.press("Alt+Shift+ArrowRight");

  expect(await groupCells(page)).toEqual([
    ["Person", "1", null],
    ["Work", "2", null],
    ["Person", "3", null],
    ["Work", "4", null],
  ]);
});

test("a grouped grid counts the group header rows too", async ({ page }) => {
  await open(page);
  await page.getByTestId("toggle-paging").uncheck();
  await page.getByTestId("toggle-grouping").check();
  await page
    .locator(".dg-group-panel")
    .getByRole("combobox", { name: "Group by" })
    .selectOption("department");
  await expect(page.locator(".dg")).toHaveAttribute("role", "treegrid");

  // Six groups and twelve rows under two header rows: the first group row is
  // the third row of the grid, not the second.
  const first = page.locator("[data-group-row]").first();
  await expect(first).toHaveAttribute("aria-rowindex", "3");

  // And the keyboard agrees: down from the group's own row and back again
  // returns to it, which it would not if the rows were counted from the wrong
  // header height.
  await first.getByRole("gridcell").click();
  await page.keyboard.press("ArrowDown");
  await expect(first.getByRole("gridcell")).not.toBeFocused();
  await page.keyboard.press("ArrowUp");
  await expect(first.getByRole("gridcell")).toBeFocused();
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
        group: cell?.getAttribute("data-group-header"),
      };
    });

  test("the arrows reach the group above a column and come back", async ({ page }) => {
    await open(page);
    await header(page, "Email").evaluate((el: HTMLElement) => el.focus());

    await page.keyboard.press("ArrowUp");
    expect(await focused(page)).toMatchObject({ text: "Person", group: "0" });

    // Back down into the column it stands over.
    await page.keyboard.press("ArrowDown");
    expect(await focused(page)).toMatchObject({ colindex: "1", text: "Name" });
  });

  test("a group is one stop, so the arrows move to the next group", async ({ page }) => {
    await open(page);
    await header(page, "Name").evaluate((el: HTMLElement) => el.focus());
    await page.keyboard.press("ArrowUp");
    expect(await focused(page)).toMatchObject({ text: "Person" });

    // Not the second column of "Person": that is the same cell.
    await page.keyboard.press("ArrowRight");
    expect(await focused(page)).toMatchObject({ text: "Work", colindex: "3" });

    await page.keyboard.press("ArrowLeft");
    expect(await focused(page)).toMatchObject({ text: "Person", colindex: "1" });
  });

  test("the grid is still a single tab stop", async ({ page }) => {
    await open(page);
    await header(page, "Name").evaluate((el: HTMLElement) => el.focus());
    await page.keyboard.press("ArrowUp");

    await expect(grid(page).locator('[tabindex="0"]')).toHaveCount(1);
  });
});

test("a multi-level header has no accessibility violations", async ({ page }) => {
  await open(page);

  const results = await new AxeBuilder({ page }).include(".dg").analyze();
  expect(results.violations).toEqual([]);
});
