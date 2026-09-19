import { expect, type Page, test } from "@playwright/test";

// examples/fullstack: the grid querying SQLite through a Dioxus server function.
// Real requests over HTTP, no simulation.

const URL = `http://127.0.0.1:${process.env.E2E_FULLSTACK_PORT ?? 8094}/`;
const grid = (page: Page) => page.getByRole("grid");
const status = (page: Page) => page.locator(".status");
const cells = (page: Page, column: number) =>
  page.locator(`.body [role='row'] [aria-colindex='${column}']`);

/** A formatted salary such as "€71,919" as a number. */
const salary = (text: string) => Number(text.replace(/[^0-9]/g, ""));

async function settled(page: Page) {
  await expect(status(page)).toHaveAttribute("data-state", "idle");
  await expect(grid(page)).not.toHaveAttribute("aria-busy", "true");
}

test.beforeEach(async ({ page }) => {
  await page.goto(URL);
  await expect(grid(page)).toHaveAttribute("aria-rowcount", "5001", { timeout: 30_000 });
  await settled(page);
});

test("the first page comes from the database", async ({ page }) => {
  await expect(page.locator(".body [role='row']")).toHaveCount(20);
  await expect(cells(page, 1).first()).toHaveText(/^\w+ \w+$/);
});

test("sorting runs on the server", async ({ page }) => {
  await page.getByRole("columnheader", { name: "Salary", exact: true }).click();
  await settled(page);
  const ascending = (await cells(page, 4).allTextContents()).map(salary);
  expect(ascending).toEqual([...ascending].sort((a, b) => a - b));

  await page.getByRole("columnheader", { name: "Salary", exact: true }).click();
  await settled(page);
  const descending = (await cells(page, 4).allTextContents()).map(salary);
  expect(descending[0]).toBeGreaterThanOrEqual(ascending[ascending.length - 1]);
});

test("search and column filters combine on the server", async ({ page }) => {
  await page.getByRole("searchbox").fill("hans");
  await expect(grid(page)).not.toHaveAttribute("aria-rowcount", "5001");
  await settled(page);
  const afterSearch = Number(await grid(page).getAttribute("aria-rowcount"));
  for (const name of await cells(page, 1).allTextContents()) expect(name).toMatch(/hans/i);

  await page.getByLabel("Filter City").fill("berlin");
  await expect(grid(page)).not.toHaveAttribute("aria-rowcount", String(afterSearch));
  await settled(page);
  for (const city of await cells(page, 3).allTextContents()) expect(city).toBe("Berlin");
  expect(Number(await grid(page).getAttribute("aria-rowcount"))).toBeLessThan(afterSearch);
});

test("SQL wildcards typed by the user match literally", async ({ page }) => {
  await page.getByRole("searchbox").fill("%");
  await expect(grid(page)).toHaveAttribute("aria-rowcount", "1");
  await settled(page);
  await expect(page.locator(".body [role='row']")).toHaveCount(0);
});

test("paging fetches the next rows", async ({ page }) => {
  const firstPage = await cells(page, 1).allTextContents();
  await page.getByRole("button", { name: "Next page" }).click();
  await settled(page);
  await expect(page.locator(".body [role='row']").first()).toHaveAttribute("aria-rowindex", "22");
  expect(await cells(page, 1).allTextContents()).not.toEqual(firstPage);
});

test("the filter bar's operators run as SQL", async ({ page }) => {
  await page.getByLabel("Filter Salary").fill(">95000");
  await expect(grid(page)).not.toHaveAttribute("aria-rowcount", "5001");
  await settled(page);
  for (const text of await cells(page, 4).allTextContents()) expect(salary(text)).toBeGreaterThan(95_000);
});

test("the value list comes from the server and filters there", async ({ page }) => {
  await page.getByRole("button", { name: "Filter options for City" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("button", { name: "Values" }).click();
  // Four cities, 1,250 employees each.
  await expect(dialog.getByRole("listitem")).toHaveText([
    "Berlin (1,250)",
    "Frankfurt (1,250)",
    "Hamburg (1,250)",
    "Munich (1,250)",
  ]);

  await dialog.getByRole("checkbox", { name: "Select all" }).uncheck();
  await dialog.getByRole("checkbox", { name: /Hamburg/ }).check();
  await dialog.getByRole("button", { name: "Apply" }).click();
  await expect(grid(page)).toHaveAttribute("aria-rowcount", "1251");
  await settled(page);
  for (const city of await cells(page, 3).allTextContents()) expect(city).toBe("Hamburg");
});

/** Moves grid focus to the salary cell of the first row, by keyboard. */
async function toFirstSalary(page: Page) {
  await grid(page).locator("[tabindex='0']").focus();
  await page.keyboard.press("ControlOrMeta+Home");
  await page.keyboard.press("ArrowDown");
  for (let step = 0; step < 3; step++) await page.keyboard.press("ArrowRight");
  await expect(cells(page, 4).first()).toBeFocused();
}

test("a salary the server refuses is taken back, with its reason", async ({ page }) => {
  const before = await cells(page, 4).first().textContent();
  await toFirstSalary(page);
  await page.keyboard.press("Enter");
  const editor = grid(page).getByRole("textbox", { name: "Salary" });
  await expect(editor).toBeFocused();
  await page.keyboard.press("ControlOrMeta+A");
  await page.keyboard.type("900000");
  await page.keyboard.press("Enter");

  await expect(page.locator(".edit-status")).toHaveText(/900000 is above the salary band/);
  await expect(cells(page, 4).first()).toHaveText(before ?? "");
});

test("a saved salary is in the database", async ({ page }, testInfo) => {
  // One value per browser: the database outlives a test, and saving the value
  // it already holds would save nothing.
  const salary = 55_500 + ["chromium", "firefox", "webkit"].indexOf(testInfo.project.name);
  const shown = `€${salary.toLocaleString("en-US")}`;
  await toFirstSalary(page);
  await page.keyboard.press("F2");
  await page.keyboard.press("ControlOrMeta+A");
  await page.keyboard.type(String(salary));
  await page.keyboard.press("Enter");
  await expect(cells(page, 4).first()).toHaveText(shown);
  await expect(page.locator(".body [role='row']").first()).not.toHaveAttribute("data-saving", "true");

  await page.reload();
  await expect(grid(page)).toHaveAttribute("aria-rowcount", "5001", { timeout: 30_000 });
  await settled(page);
  await expect(cells(page, 4).first()).toHaveText(shown);
});

test("grouping runs in SQL, one page at a time", async ({ page }) => {
  const tree = page.locator(".grid");
  const rows = tree.locator(".body [role='row']");
  await page.locator(".group-panel").getByRole("combobox", { name: "Group by" }).selectOption("department");
  await expect(tree).toHaveAttribute("role", "treegrid", { timeout: 10_000 });
  await expect(status(page)).toHaveAttribute("data-state", "idle");

  // Twenty rows a page, the first a group header: 1,000 employees a department.
  await expect(rows).toHaveCount(20);
  await expect(rows.first()).toContainText("Department: engineering");
  await expect(rows.first()).toContainText("1,000 rows");
  // 5,000 rows, five group headers and five footers with the server's sums.
  await expect(tree).toHaveAttribute("aria-rowcount", "5011");

  await page.getByRole("button", { name: "Next page" }).click();
  await expect(status(page)).toHaveAttribute("data-state", "idle");
  await expect(rows.first()).toHaveAttribute("aria-rowindex", "22");
  await expect(rows.first()).toHaveAttribute("aria-level", "2");
});
