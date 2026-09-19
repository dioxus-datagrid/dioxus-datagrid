import { expect, type Page, test } from "@playwright/test";

// Phase 6 acceptance, in a real browser: examples/server with its simulated
// latency. playwright.config.ts builds and serves the example next to the
// playground. A one-letter search takes three times the base latency of 400ms,
// which is what makes an out-of-order answer reproducible.

const URL = `http://127.0.0.1:${process.env.E2E_SERVER_PORT ?? 8093}/`;
const grid = (page: Page) => page.getByRole("grid");
const firstName = (page: Page) => page.locator(".body [role='row'] [aria-colindex='1']").first();
const log = (page: Page) => page.locator(".log li");

test.beforeEach(async ({ page }) => {
  await page.goto(URL);
  await expect(page.locator(".status")).toHaveAttribute("data-state", "idle", { timeout: 30_000 });
  // 5,000 rows, the header and the totals.
  await expect(grid(page)).toHaveAttribute("aria-rowcount", "5002");
});

test("typing is debounced into one request", async ({ page }) => {
  const before = await log(page).count();
  await page.getByRole("searchbox").pressSequentially("Berlin", { delay: 60 });
  await expect(page.locator(".status")).toHaveAttribute("data-state", "idle", { timeout: 10_000 });
  await expect(grid(page)).not.toHaveAttribute("aria-rowcount", "5002");
  expect(await log(page).count()).toBe(before + 1);
  await expect(log(page).first()).toContainText('search "Berlin"');
});

test("an out-of-order answer is cancelled, the newer one shown", async ({ page }) => {
  const search = page.getByRole("searchbox");
  // "H" is the slowest query (3x latency); wait past the debounce so it is sent.
  await search.fill("H");
  await expect(log(page).first()).toContainText('search "H"');
  await expect(page.locator(".status")).toHaveAttribute("data-state", "loading");
  // Then a longer, faster query replaces it while it is still running.
  await search.fill("Hans");
  await expect(log(page).first()).toContainText('search "Hans"');
  await expect(page.locator(".status")).toHaveAttribute("data-state", "idle", { timeout: 10_000 });

  await expect(page.locator(".log li[data-outcome='cancelled']")).toContainText('search "H"');
  await expect(firstName(page)).toContainText("Hans");
  // Wait out the slow request's latency: nothing may overwrite the result.
  await page.waitForTimeout(1500);
  await expect(firstName(page)).toContainText("Hans");
  await expect(page.getByRole("searchbox")).toHaveValue("Hans");
});

test("a failed request shows an error and retry recovers", async ({ page }) => {
  await page.getByRole("button", { name: "Fail next request" }).click();
  await page.getByRole("button", { name: "Next page" }).click();
  await expect(page.locator(".status")).toHaveAttribute("data-state", "error", { timeout: 10_000 });
  await expect(page.locator(".status")).toContainText("did not respond");

  await page.locator(".status").getByRole("button", { name: "Retry" }).click();
  await expect(page.locator(".status")).toHaveAttribute("data-state", "idle", { timeout: 10_000 });
  await expect(page.getByRole("navigation", { name: "Pagination" })).toHaveAttribute("data-page", "1");
  await expect(grid(page).locator(".body [role='row']").first()).toHaveAttribute("aria-rowindex", "17");
});

test("sorting loads at once and marks the grid busy meanwhile", async ({ page }) => {
  await page.getByRole("columnheader", { name: "Salary", exact: true }).click();
  await expect(grid(page)).toHaveAttribute("aria-busy", "true");
  await expect(page.locator(".status")).toHaveAttribute("data-state", "idle", { timeout: 10_000 });
  await expect(grid(page)).not.toHaveAttribute("aria-busy", "true");
  await expect(page.getByRole("columnheader", { name: "Salary", exact: true })).toHaveAttribute("aria-sort", "ascending");
  await expect(log(page).first()).toContainText("sort salary Asc");
});

test("grouping runs on the server, aggregates included", async ({ page }) => {
  const tree = page.locator(".grid");
  await page.locator(".group-panel").getByRole("combobox", { name: "Group by" }).selectOption("department");
  await expect(page.locator(".status")).toHaveAttribute("data-state", "idle", { timeout: 10_000 });
  await expect(tree).toHaveAttribute("role", "treegrid");
  await expect(log(page).first()).toContainText("group by department");

  // 1,000 employees in each of five departments.
  const engineering = tree.locator("[data-group-row]").first();
  await expect(engineering).toContainText("Department: engineering");
  await expect(engineering).toContainText("1,000 rows");
  // 5,000 rows, five group headers and footers, the header and the totals.
  await expect(tree).toHaveAttribute("aria-rowcount", "5012");

  // Collapsing asks the server again; the group's sum moves into its header.
  await engineering.click();
  await expect(page.locator(".status")).toHaveAttribute("data-state", "idle", { timeout: 10_000 });
  await expect(engineering).toHaveAttribute("aria-expanded", "false");
  await expect(engineering).toContainText("Sum of Salary");
  // Finance follows at once, with its rows filling the page.
  await expect(tree.locator("[data-group-row]").nth(1)).toContainText("Department: finance");
  await expect(tree.locator(".body [role='row']").nth(2)).toHaveAttribute("aria-level", "2");
});
