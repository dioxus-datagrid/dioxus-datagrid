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
