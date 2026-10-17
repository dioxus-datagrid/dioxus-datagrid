import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 14: the toolbar, one row above the table. The playground mounts
// data_grid_toolbar with a DataGridExport and a "Refresh" button of its own, and
// turns off the grid's own search box and column picker while it does.

const grid = (page: Page) => page.getByRole("grid");
const bar = (page: Page) => page.getByRole("group", { name: "Table actions" });

async function open(page: Page, extra: (page: Page) => Promise<void> = async () => {}) {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.getByTestId("toggle-toolbar").check();
  await extra(page);
  await expect(grid(page)).toBeVisible();
}

test("it holds the search, the count, the column picker and entries of the application's", async ({ page }) => {
  await open(page);
  await expect(bar(page)).toBeVisible();
  await expect(bar(page).getByRole("searchbox")).toBeVisible();
  await expect(bar(page).getByText(/\d+ rows?/)).toBeVisible();
  await expect(bar(page).locator("summary")).toHaveText("Columns");
  await expect(bar(page).getByRole("button", { name: "Refresh" })).toBeVisible();
  await expect(bar(page).getByRole("button", { name: "Export as CSV" })).toBeVisible();
});

test("the grid's own search box and picker are gone, not there twice", async ({ page }) => {
  await open(page);
  await expect(page.getByRole("searchbox")).toHaveCount(1);
  await expect(page.locator("details.dg-columns")).toHaveCount(1);
});

test("the search box filters, and the count follows", async ({ page }) => {
  await open(page);
  const count = bar(page).locator(".dg-count");
  const before = await count.textContent();
  await bar(page).getByRole("searchbox").fill("zzzzzz-nothing");
  await expect(count).toHaveText(/^0 rows/);
  expect(before).not.toBe("0 rows");
  await bar(page).getByRole("searchbox").fill("");
  await expect(count).toHaveText(before!);
});

test("the column picker hides a column", async ({ page }) => {
  await open(page);
  await bar(page).locator("summary").click();
  await bar(page).getByRole("checkbox", { name: "Age", exact: true }).uncheck();
  await expect(page.getByRole("columnheader", { name: "Age", exact: true })).toHaveCount(0);
  await bar(page).getByRole("checkbox", { name: "Age", exact: true }).check();
  await expect(page.getByRole("columnheader", { name: "Age", exact: true })).toBeVisible();
});

test("an entry of the application's is a button like the rest", async ({ page }) => {
  await open(page);
  await expect(page.getByTestId("toolbar-log")).toHaveText("refreshed: 0");
  await bar(page).getByRole("button", { name: "Refresh" }).click();
  await expect(page.getByTestId("toolbar-log")).toHaveText("refreshed: 1");
});

test("the editing buttons appear when the grid is editable, once", async ({ page }) => {
  await open(page);
  // Not editable: no add, edit or delete.
  await expect(bar(page).getByRole("button", { name: "Add", exact: true })).toHaveCount(0);

  await page.getByRole("radio", { name: "Row", exact: true }).first().check();
  await expect(bar(page).getByRole("button", { name: "Add", exact: true })).toBeVisible();
  await expect(bar(page).getByRole("button", { name: "Edit", exact: true })).toBeVisible();
  await expect(bar(page).getByRole("button", { name: "Delete", exact: true })).toBeVisible();
  // The editor's own toolbar is off, so they are not on screen a second time.
  await expect(page.getByRole("button", { name: "Delete", exact: true })).toHaveCount(1);
});

test("Delete works on the focused row from the toolbar", async ({ page }) => {
  await open(page, async (page) => {
    await page.getByRole("radio", { name: "Row", exact: true }).first().check();
  });
  const count = bar(page).locator(".dg-count");
  await expect(count).toHaveText(/^12 rows/);
  await page.locator(".dg-body [role='row'] [aria-colindex='1']").nth(1).click();
  await bar(page).getByRole("button", { name: "Delete", exact: true }).click();
  // Deleting asks first.
  await page.getByRole("alertdialog").getByRole("button", { name: "Delete", exact: true }).click();
  await expect(count).toHaveText(/^11 rows/);
});

test("it wraps instead of overflowing on a narrow screen", async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 800 });
  await open(page);
  const box = (await bar(page).boundingBox())!;
  expect(box.x + box.width).toBeLessThanOrEqual(360 + 1);
  // Every control inside it is in the window too: they wrapped onto more lines.
  const right = await bar(page).evaluate((element) =>
    Math.max(...Array.from(element.querySelectorAll("input, button, summary, .dg-count")).map((el) => el.getBoundingClientRect().right)),
  );
  expect(right).toBeLessThanOrEqual(361);
});

test("German names the group in German", async ({ page }) => {
  await open(page, async (page) => {
    await page.getByRole("radio", { name: "Deutsch" }).check();
  });
  await expect(page.getByRole("group", { name: "Tabellenaktionen" })).toBeVisible();
});

test("it has no axe violations", async ({ page }) => {
  await open(page, async (page) => {
    await page.getByRole("radio", { name: "Row", exact: true }).first().check();
  });
  const results = await new AxeBuilder({ page }).include(".dg-toolbar-bar").analyze();
  expect(results.violations).toEqual([]);
});
