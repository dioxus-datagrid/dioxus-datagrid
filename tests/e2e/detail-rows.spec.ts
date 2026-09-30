import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 12: detail rows. The playground puts an expander column first when
// `toggle-detail-rows` is on, and gives a detail only to people over 30 — so a
// row without one is in every test's way on purpose.

const grid = (page: Page) => page.getByRole("grid");
const detailRows = (page: Page) => grid(page).locator("[data-detail-row]");
/** The rows that show a person — a detail row has cells too, but is not one. */
const bodyRows = (page: Page) =>
  grid(page).locator('[role="row"]:has([role="gridcell"]):not([data-detail-row])');
const toggle = (page: Page, row: number) => bodyRows(page).nth(row).getByRole("button");

async function open(page: Page, paged = false) {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.locator(".dg-count")).toHaveText("12 rows");
  await page.getByTestId("toggle-detail-rows").check();
  // Most of these want to see every row at once; paging is the playground's
  // default and has a test of its own below.
  await page.getByTestId("toggle-paging").setChecked(paged);
  await expect(bodyRows(page).first().getByRole("gridcell").first()).toBeVisible();
}

/** Every row's aria-rowindex, in the order they are rendered. */
const rowIndices = (page: Page) =>
  grid(page)
    .locator('[role="row"]')
    .evaluateAll((rows) => rows.map((row) => row.getAttribute("aria-rowindex")));

test("a row opens under itself and closes again", async ({ page }) => {
  await open(page);
  await expect(detailRows(page)).toHaveCount(0);

  await toggle(page, 0).click();

  await expect(detailRows(page)).toHaveCount(1);
  await expect(detailRows(page)).toContainText("zoe.bauer@example.com");
  // The button says it, not the row: a row says `aria-expanded` only in a
  // treegrid, where its children are rows.
  await expect(toggle(page, 0)).toHaveAttribute("aria-expanded", "true");
  await expect(bodyRows(page).first()).toHaveAttribute("data-expanded", "true");

  await toggle(page, 0).click();
  await expect(detailRows(page)).toHaveCount(0);
  await expect(toggle(page, 0)).toHaveAttribute("aria-expanded", "false");
});

test("the detail sits right below the row it belongs to", async ({ page }) => {
  await open(page);
  await toggle(page, 0).click();

  // The row after the first one is its detail, not the second person.
  const rows = grid(page).locator('[role="row"]');
  await expect(rows.nth(2)).toHaveAttribute("data-detail-row", "");
  await expect(rows.nth(3)).toContainText("adam Fischer");
});

test("the detail row takes its own row number", async ({ page }) => {
  await open(page);
  await expect(grid(page)).toHaveAttribute("aria-rowcount", "13");
  const before = await rowIndices(page);
  await toggle(page, 0).click();
  const after = await rowIndices(page);

  // One row more, numbered straight through: a detail row that took no number
  // would leave every row after it saying the wrong one.
  expect(after.length).toBe(before.length + 1);
  expect(after).toEqual(after.map((_, index) => String(index + 1)));
  await expect(grid(page)).toHaveAttribute("aria-rowcount", "14");
});

test("a detail takes its place on a page, as any row does", async ({ page }) => {
  await open(page, true);
  await expect(bodyRows(page)).toHaveCount(5);

  await toggle(page, 0).click();

  // Five rows to a page, and the detail is one of them, so the last person
  // moves to the next page — the same rule a group follows.
  await expect(bodyRows(page)).toHaveCount(4);
  await expect(detailRows(page)).toHaveCount(1);
  await expect(page.getByText("Page 1 of 3")).toBeVisible();
});

test("the button points at the row it opened", async ({ page }) => {
  await open(page);
  // Closed, it points at nothing: the row it would name is not there.
  await expect(toggle(page, 0)).not.toHaveAttribute("aria-controls", /.*/);

  await toggle(page, 0).click();

  const id = await toggle(page, 0).getAttribute("aria-controls");
  expect(id).toBeTruthy();
  await expect(detailRows(page)).toHaveAttribute("id", id ?? "");
});

test("a row with nothing to show has no button", async ({ page }) => {
  await open(page);

  // adam Fischer is 25, so there is nothing to open.
  await expect(bodyRows(page).nth(1)).toContainText("adam Fischer");
  await expect(bodyRows(page).nth(1).getByRole("button")).toHaveCount(0);
  await expect(bodyRows(page).nth(1)).not.toHaveAttribute("data-expanded", /.*/);
});

test("Enter on an expander cell opens the row, and Escape leaves it alone", async ({ page }) => {
  await open(page);

  await bodyRows(page).first().getByRole("gridcell").first().click();
  // The click on the cell opened it; Enter closes it again.
  await expect(detailRows(page)).toHaveCount(1);
  await page.keyboard.press("Enter");
  await expect(detailRows(page)).toHaveCount(0);
  await page.keyboard.press("Enter");
  await expect(detailRows(page)).toHaveCount(1);
});

test("the arrows step over a detail row, not into its columns", async ({ page }) => {
  await open(page);
  await toggle(page, 0).click();
  await bodyRows(page).first().getByRole("gridcell").nth(1).click();

  // Down from the first row lands on the detail, which is one cell wide.
  await page.keyboard.press("ArrowDown");
  await expect(detailRows(page).getByRole("gridcell")).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await expect(detailRows(page).getByRole("gridcell")).toBeFocused();

  // And on down to the person after it.
  await page.keyboard.press("ArrowDown");
  await expect(bodyRows(page).nth(1).getByRole("gridcell").first()).toBeFocused();
  await expect(bodyRows(page).nth(1)).toContainText("adam Fischer");
});

test("what the detail holds is part of the page", async ({ page }) => {
  await open(page);
  await toggle(page, 0).click();

  // The grid is one tab stop, and what the application put in the detail is
  // reachable after it.
  const action = page.getByTestId("detail-action-1");
  await expect(action).toBeVisible();
  await action.focus();
  await expect(action).toBeFocused();
});

test("an open row stays open while the grid is sorted", async ({ page }) => {
  await open(page);
  await toggle(page, 0).click();
  await expect(detailRows(page)).toContainText("zoe.bauer@example.com");

  await grid(page).getByRole("columnheader", { name: "Name" }).click();

  // Zoe moved, and her detail moved with her.
  await expect(detailRows(page)).toHaveCount(1);
  await expect(detailRows(page)).toContainText("zoe.bauer@example.com");
  const rows = grid(page).locator('[role="row"]');
  const position = await rows.evaluateAll((all) =>
    all.findIndex((row) => row.hasAttribute("data-detail-row")),
  );
  await expect(rows.nth(position - 1)).toContainText("Zoe Bauer");
});

test("a virtualized grid does not open them", async ({ page }) => {
  await open(page);
  await page.getByTestId("toggle-virtualized").check();
  await expect(page.locator(".dg-count")).toContainText("100,000");

  // No button at all: its rows are placed by counting equal heights.
  await expect(bodyRows(page).first().getByRole("button")).toHaveCount(0);
  await expect(detailRows(page)).toHaveCount(0);
});

test("an open row has no axe violations", async ({ page }) => {
  await open(page);
  await toggle(page, 0).click();

  const results = await new AxeBuilder({ page }).include(".dg").analyze();
  expect(results.violations).toEqual([]);
});
