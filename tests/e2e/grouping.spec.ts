import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 10: grouping and aggregates. Groups of the twelve playground rows by
// department, ascending: engineering (3), finance (2), legal (1), research (2),
// sales (2), support (2).

const grid = (page: Page) => page.locator(".dg");
const panel = (page: Page) => page.locator(".dg-group-panel");
const groupRows = (page: Page) => grid(page).locator("[data-group-row]");
const groupRow = (page: Page, name: string) =>
  groupRows(page).filter({ hasText: `Department: ${name}` });
const header = (page: Page, label: string) =>
  grid(page).getByRole("columnheader", { name: label, exact: true });

async function open(page: Page, { paged = false, totals = false } = {}) {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.locator(".dg-count")).toHaveText("12 rows");
  await page.getByTestId("toggle-grouping").check();
  if (!paged) await page.getByTestId("toggle-paging").uncheck();
  if (totals) await page.getByTestId("toggle-totals").check();
}

async function groupByDepartment(page: Page) {
  await panel(page).getByRole("combobox", { name: "Group by" }).selectOption("department");
  await expect(grid(page)).toHaveAttribute("role", "treegrid");
}

/** Where DOM focus is: the text of the focused cell's row. */
const focusedRow = (page: Page) =>
  page.evaluate(() => (document.activeElement?.closest("[role='row']") as HTMLElement)?.innerText);

test("a column picked from the panel groups the rows under headers", async ({ page }) => {
  await open(page);
  await expect(panel(page)).toContainText("Drag a column header here to group by it");
  await groupByDepartment(page);

  await expect(groupRows(page)).toHaveCount(6);
  const first = groupRows(page).first();
  await expect(first).toContainText("Department: engineering");
  await expect(first).toContainText("3 rows");
  await expect(first).toHaveAttribute("aria-level", "1");
  await expect(first).toHaveAttribute("aria-expanded", "true");
  await expect(first).toHaveAttribute("aria-posinset", "1");
  await expect(first).toHaveAttribute("aria-setsize", "6");
  // Header, six group headers, twelve rows.
  await expect(grid(page)).toHaveAttribute("aria-rowcount", "19");
  await expect(grid(page).locator(".dg-body [role='row'][aria-level='2']")).toHaveCount(12);

  // The panel lists the grouped column and removes it again.
  await panel(page).getByRole("button", { name: "Stop grouping by Department" }).click();
  await expect(grid(page)).toHaveAttribute("role", "grid");
  await expect(groupRows(page)).toHaveCount(0);
});

test("a header dragged onto the panel groups by its column", async ({ page, browserName }) => {
  // WebKit in Playwright starts no HTML drag from a mouse drag.
  test.skip(browserName === "webkit", "no HTML drag and drop under WebKit automation");
  await open(page);
  await expect(header(page, "Department")).toHaveAttribute("draggable", "true");
  await header(page, "Department").dragTo(panel(page));
  await expect(grid(page)).toHaveAttribute("role", "treegrid");
  await expect(groupRow(page, "engineering")).toBeVisible();
  // A grouped column is not offered again.
  await expect(header(page, "Department")).not.toHaveAttribute("draggable", "true");
});

test.describe("keyboard", () => {
  test.beforeEach(async ({ page }) => {
    await open(page);
    await groupByDepartment(page);
    // Into the grid, onto the first group's header.
    await grid(page).locator("[tabindex='0']").focus();
    await page.keyboard.press("ControlOrMeta+Home");
    await page.keyboard.press("ArrowDown");
    await expect(groupRow(page, "engineering").locator("[data-group-cell]")).toBeFocused();
  });

  test("left collapses a group, right expands it, as in a treegrid", async ({ page }) => {
    const engineering = groupRow(page, "engineering");
    await page.keyboard.press("ArrowLeft");
    await expect(engineering).toHaveAttribute("aria-expanded", "false");
    await expect(grid(page).getByText("Zoe Bauer")).toHaveCount(0);
    // Down goes straight to the next group.
    await page.keyboard.press("ArrowDown");
    expect(await focusedRow(page)).toContain("Department: finance");
    await page.keyboard.press("ArrowUp");

    await page.keyboard.press("ArrowRight");
    await expect(engineering).toHaveAttribute("aria-expanded", "true");
    await expect(grid(page).getByText("Zoe Bauer")).toBeVisible();
    await page.keyboard.press("ArrowDown");
    expect(await focusedRow(page)).toContain("Zoe Bauer");
  });

  test("Enter and Space toggle a group", async ({ page }) => {
    const engineering = groupRow(page, "engineering");
    await page.keyboard.press("Enter");
    await expect(engineering).toHaveAttribute("aria-expanded", "false");
    await page.keyboard.press(" ");
    await expect(engineering).toHaveAttribute("aria-expanded", "true");
  });

  test("on a data row the arrows still move between cells", async ({ page }) => {
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowRight");
    const focused = await page.evaluate(() => document.activeElement?.getAttribute("aria-colindex"));
    expect(focused).toBe("2");
    await expect(groupRow(page, "engineering")).toHaveAttribute("aria-expanded", "true");
  });

  test("collapse all and expand all act on every group", async ({ page }) => {
    await panel(page).getByRole("button", { name: "Collapse all" }).click();
    await expect(grid(page).locator("[aria-expanded='false']")).toHaveCount(6);
    await expect(grid(page).locator(".dg-body [aria-level='2']")).toHaveCount(0);
    await panel(page).getByRole("button", { name: "Expand all" }).click();
    await expect(grid(page).locator("[aria-expanded='true']")).toHaveCount(6);
  });
});

test("groups continue across pages", async ({ page }) => {
  await open(page, { paged: true });
  await groupByDepartment(page);
  // Five rows a page: engineering's header and rows, then finance's header.
  const rows = grid(page).locator(".dg-body [role='row']");
  await expect(rows).toHaveCount(5);
  await expect(rows.last()).toContainText("Department: finance");
  await page.getByRole("button", { name: "Next page" }).click();
  // Finance's rows open page two, under its header on page one.
  await expect(rows.first()).toContainText("bob Schneider");
  await expect(rows.first()).toHaveAttribute("aria-rowindex", "7");
});

test.describe("aggregates", () => {
  test.beforeEach(async ({ page }) => {
    await open(page, { totals: true });
    await groupByDepartment(page);
  });

  test("each group has a footer and the grid its totals", async ({ page }) => {
    const footers = grid(page).locator("[data-group-footer]");
    await expect(footers).toHaveCount(6);
    // Engineering: 30, 30 and 28.
    const engineering = footers.first();
    await expect(engineering.locator("[data-aggregate='count']")).toContainText("3");
    await expect(engineering.locator("[data-aggregate='min']")).toContainText("28");
    await expect(engineering.locator("[data-aggregate='average']")).toContainText("29.33");
    await expect(engineering.locator("[data-aggregate='max']")).toContainText("30");

    const totals = page.locator(".dg-foot");
    await expect(totals).toContainText("Count 12");
    await expect(totals.locator("[data-aggregate='max']")).toContainText("52");
    // Header, 6 × (header, footer), 12 rows, the totals.
    await expect(grid(page)).toHaveAttribute("aria-rowcount", "26");
  });

  test("a collapsed group shows its aggregates in its header", async ({ page }) => {
    await groupRow(page, "engineering").click();
    await expect(groupRow(page, "engineering")).toContainText("Average of Age: 29.33");
    // Only the visible columns: Salary is hidden.
    await expect(groupRow(page, "engineering")).not.toContainText("Salary");
  });

  test("the keyboard reaches the totals last", async ({ page }) => {
    await grid(page).locator("[tabindex='0']").focus();
    await page.keyboard.press("ControlOrMeta+End");
    const row = await page.evaluate(
      () => (document.activeElement?.closest("[role='row']") as HTMLElement).getAttribute("aria-rowindex"),
    );
    expect(row).toBe("26");
    await expect(page.locator(".dg-foot [role='gridcell']").last()).toBeFocused();
  });

  test("grouped with totals, the grid has no axe violations", async ({ page }) => {
    await groupRow(page, "finance").click();
    const results = await new AxeBuilder({ page }).include(".dg-wrapper").analyze();
    expect(results.violations).toEqual([]);
  });
});

test.describe("dark theme", () => {
  test.use({ colorScheme: "dark" });

  test("grouped with totals, the grid has no axe violations", async ({ page }) => {
    await open(page, { totals: true });
    await groupByDepartment(page);
    await groupRow(page, "legal").click();
    const results = await new AxeBuilder({ page }).include(".dg-wrapper").analyze();
    expect(results.violations).toEqual([]);
  });
});

test.describe("100,000 rows", () => {
  test("group, collapse and scroll without delay", async ({ page }) => {
    await open(page);
    await page.getByTestId("toggle-virtualized").check();
    await expect(grid(page)).toHaveAttribute("aria-rowcount", "100001");

    // Grouping sorts and groups every row; it has to feel immediate.
    const started = Date.now();
    await groupByDepartment(page);
    await expect(groupRows(page).first()).toBeVisible();
    const grouping = Date.now() - started;
    expect(grouping).toBeLessThan(1500);

    // Only what is in view is rendered, headers included.
    expect(await grid(page).locator(".dg-body [role='row']").count()).toBeLessThan(80);

    const collapsing = Date.now();
    await panel(page).getByRole("button", { name: "Collapse all" }).click();
    await expect(groupRows(page)).toHaveCount(6);
    expect(Date.now() - collapsing).toBeLessThan(1500);

    await panel(page).getByRole("button", { name: "Expand all" }).click();
    await grid(page).locator("[tabindex='0']").focus();
    await page.keyboard.press("ControlOrMeta+End");
    const focused = await page.evaluate(() => document.activeElement?.closest("[role='row']")?.getAttribute("aria-rowindex"));
    // Header, 6 group headers and 100,000 rows.
    expect(focused).toBe("100007");
  });
});
