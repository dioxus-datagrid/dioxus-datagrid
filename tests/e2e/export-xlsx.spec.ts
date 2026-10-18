import { expect, type Page, test } from "@playwright/test";
import ExcelJS from "exceljs";

// Phase 13: Excel. The export add-on's "Export as Excel" button downloads a workbook
// through the same Blob and clicked link as the CSV, but the content is bytes, so it
// travels over the eval channel as base64. The workbook is read back with ExcelJS,
// which is not the library that wrote it: if it opens here, the file is a real one.

const grid = (page: Page) => page.getByRole("grid");

async function open(page: Page, extra: (page: Page) => Promise<void> = async () => {}) {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.getByTestId("toggle-export").check();
  await extra(page);
  await expect(page.getByRole("button", { name: /Export as Excel|Als Excel exportieren/ })).toBeVisible();
}

/** Clicks the Excel button and reads the workbook the browser was handed. */
async function downloadWorkbook(page: Page) {
  const [download] = await Promise.all([
    page.waitForEvent("download"),
    page.getByRole("button", { name: /Export as Excel|Als Excel exportieren/ }).click(),
  ]);
  const path = await download.path();
  const workbook = new ExcelJS.Workbook();
  await workbook.xlsx.readFile(path);
  return { name: download.suggestedFilename(), workbook, sheet: workbook.worksheets[0] };
}

/** One row of a sheet as plain values, from the first column on. */
const values = (sheet: ExcelJS.Worksheet, row: number) =>
  (sheet.getRow(row).values as unknown[]).slice(1);

test("downloads a workbook that opens, with the columns the grid shows", async ({ page }) => {
  await open(page);
  const { name, workbook, sheet } = await downloadWorkbook(page);

  expect(name).toBe("employees.xlsx");
  expect(workbook.worksheets).toHaveLength(1);
  expect(sheet.name).toBe("Export");
  // Salary and Since start hidden, so they are not in the file.
  expect(values(sheet, 1)).toEqual(["Name", "Email", "Department", "Age"]);
});

test("holds every row, not the page that is shown", async ({ page }) => {
  await open(page);
  const shown = await page.locator(".dg-body [role='row']").count();
  expect(shown).toBe(5);

  const { sheet } = await downloadWorkbook(page);
  // The header and twelve rows.
  expect(sheet.rowCount).toBe(13);
});

test("numbers are numbers, so a column can be summed", async ({ page }) => {
  await open(page);
  const { sheet } = await downloadWorkbook(page);

  const ages = [2, 3, 4, 5].map((row) => sheet.getCell(row, 4).value);
  expect(ages.every((age) => typeof age === "number")).toBe(true);
  // Text stays text.
  expect(typeof sheet.getCell(2, 1).value).toBe("string");
});

test("holds the rows as they are filtered and sorted", async ({ page }) => {
  await open(page);
  await page.getByPlaceholder("Search all columns").fill("research");
  await page.getByRole("columnheader", { name: "Name", exact: true }).click();

  const { sheet } = await downloadWorkbook(page);
  const rows = Array.from({ length: sheet.rowCount - 1 }, (_, i) => values(sheet, i + 2));
  expect(rows.length).toBeGreaterThan(0);
  expect(rows.every((row) => row[2] === "research")).toBe(true);
  const names = rows.map((row) => String(row[0]));
  expect(names).toEqual([...names].sort((a, b) => a.localeCompare(b, "en", { sensitivity: "base" })));
});

test("a shown column keeps its number format and a date is a date", async ({ page }) => {
  await open(page, async (page) => {
    await page.locator(".dg-columns").locator("summary").click();
    for (const name of ["Salary", "Since"]) {
      await page.locator(".dg-columns").getByRole("checkbox", { name, exact: true }).check();
    }
  });
  const { sheet } = await downloadWorkbook(page);

  expect(values(sheet, 1)).toEqual(["Name", "Email", "Department", "Age", "Salary", "Since"]);
  const salary = sheet.getCell(2, 5);
  expect(typeof salary.value).toBe("number");
  // The format the grid shows it in, as an Excel format, not as text.
  expect(salary.numFmt).toContain("#,##0.00");
  const since = sheet.getCell(2, 6);
  expect(since.value).toBeInstanceOf(Date);
  expect(since.numFmt).toContain("yyyy");
});

test("the header is bold and stays in view", async ({ page }) => {
  await open(page);
  const { sheet } = await downloadWorkbook(page);
  expect(sheet.getCell(1, 1).font?.bold).toBe(true);
  expect(sheet.views[0]).toMatchObject({ state: "frozen", ySplit: 1 });
  expect(sheet.autoFilter).toBeTruthy();
});

test("a column hidden by the user is not in the file", async ({ page }) => {
  await open(page, async (page) => {
    await page.locator(".dg-columns").locator("summary").click();
    await page.locator(".dg-columns").getByRole("checkbox", { name: "Email" }).uncheck();
  });
  const { sheet } = await downloadWorkbook(page);
  expect(values(sheet, 1)).toEqual(["Name", "Department", "Age"]);
});

test("grouped, the workbook has headings, an outline and the totals", async ({ page }) => {
  await open(page, async (page) => {
    await page.getByTestId("toggle-grouping").check();
    await page.getByTestId("toggle-totals").check();
    await page.getByRole("combobox", { name: "Group by" }).selectOption("department");
  });
  const { sheet } = await downloadWorkbook(page);

  const first = String(sheet.getCell(2, 1).value);
  expect(first).toMatch(/^Department: /);
  // The rows of a group are an outline; its heading is not.
  expect(sheet.getRow(2).outlineLevel ?? 0).toBe(0);
  expect(sheet.getRow(3).outlineLevel).toBe(1);
  // No filter: it would hide a heading along with its rows.
  expect(sheet.autoFilter).toBeFalsy();

  // A group's aggregates are the last row of its outline, and the first column says
  // what they are rather than holding a bare number.
  expect(String(sheet.getCell(6, 1).value)).toBe("Count 3");
  expect(sheet.getRow(6).outlineLevel).toBe(1);

  const last = sheet.getRow(sheet.rowCount);
  expect(String(last.getCell(1).value)).toMatch(/^Totals/);
});

test("says what it is in the language of the grid", async ({ page }) => {
  await open(page, async (page) => {
    await page.getByRole("radio", { name: "Deutsch" }).check();
  });
  await expect(page.getByRole("button", { name: "Als Excel exportieren" })).toBeVisible();
  const { sheet } = await downloadWorkbook(page);
  expect(values(sheet, 1)).toEqual(["Name", "E-Mail", "Abteilung", "Alter"]);
});

test("the grid is still usable after a download", async ({ page }) => {
  await open(page);
  await downloadWorkbook(page);
  await expect(grid(page)).toBeVisible();
  await page.getByRole("button", { name: "Next page" }).click();
  await expect(page.getByText(/Page 2 of/)).toBeVisible();
});

test("a CSV and a workbook come from the same click path and both work", async ({ page }) => {
  await open(page);
  const [csv] = await Promise.all([
    page.waitForEvent("download"),
    page.getByRole("button", { name: "Export as CSV" }).click(),
  ]);
  expect(csv.suggestedFilename()).toBe("employees.csv");
  const { name } = await downloadWorkbook(page);
  expect(name).toBe("employees.xlsx");
});
