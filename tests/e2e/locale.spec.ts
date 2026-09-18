import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 7 acceptance: the playground switches to German, and the grid's texts,
// number and date formats follow; axe finds nothing in either language.

const grid = (page: Page) => page.getByRole("grid");
const firstRow = (page: Page) => page.locator(".dg-body [role='row']").first();
const cell = (page: Page, column: number) =>
  firstRow(page).locator(`[role='gridcell'][aria-colindex='${column}']`);

/** Shows the salary and start date columns, which start hidden. */
async function showFormattedColumns(page: Page) {
  await page.locator(".dg-columns summary").click();
  for (const name of [/^(Salary|Gehalt)$/, /^(Since|Seit)$/]) {
    await page.locator(".dg-columns-menu label").filter({ hasText: name }).getByRole("checkbox").check();
  }
  await page.locator(".dg-columns summary").click();
  await expect(grid(page)).toHaveAttribute("aria-colcount", "6");
}

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.locator(".dg-body [role='row']")).toHaveCount(5);
});

test("English formats numbers and dates the English way", async ({ page }) => {
  await showFormattedColumns(page);

  // Zoe Bauer, the first row: salary and start date are derived from the id.
  await expect(cell(page, 5)).toHaveText("€4,719.25");
  await expect(cell(page, 6)).toHaveText("10/14/2011");
  await expect(cell(page, 5)).toHaveAttribute("data-align", "end");
  await expect(page.getByRole("columnheader", { name: "Salary" })).toHaveAttribute("data-align", "end");
});

test("German translates the grid's texts and formats", async ({ page }) => {
  await showFormattedColumns(page);
  await page.getByTestId("language-de").check();

  await expect(cell(page, 5)).toHaveText("4.719,25 €");
  await expect(cell(page, 6)).toHaveText("14.10.2011");
  await expect(page.locator(".dg-count")).toHaveText("12 Zeilen");
  await expect(page.getByRole("button", { name: "Nächste Seite" })).toBeVisible();
  await expect(page.getByRole("status").filter({ hasText: "Seite" })).toHaveText("Seite 1 von 3");
  await expect(page.getByRole("textbox", { name: "Abteilung filtern" })).toBeVisible();
  await expect(page.locator(".dg-wrapper")).toHaveAttribute("lang", "de");

  // Switching back restores English without a reload.
  await page.getByTestId("language-en").check();
  await expect(page.locator(".dg-count")).toHaveText("12 rows");
  await expect(cell(page, 5)).toHaveText("€4,719.25");
});

for (const language of ["en", "de"] as const) {
  test(`has no axe violations in ${language === "en" ? "English" : "German"}`, async ({ page }) => {
    await showFormattedColumns(page);
    await page.getByTestId(`language-${language}`).check();
    await expect(page.locator(".dg-wrapper")).toHaveAttribute("lang", language);

    const results = await new AxeBuilder({ page }).include(".dg-wrapper").analyze();
    expect(results.violations).toEqual([]);
  });
}
