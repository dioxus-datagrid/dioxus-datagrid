import AxeBuilder from "@axe-core/playwright";
import { expect, type Locator, type Page, test } from "@playwright/test";

// The playground renders twelve employees, five per page, unsorted:
//   Zoe Bauer, adam Fischer, Mia Kaufmann, Carol Weber, bob Schneider,
//   Ingrid Vogel, Hugo Brandt, Lena Hoffmann, Nils Krause, Ava Richter,
//   Theo Lang, Yara Nowak

const grid = (page: Page) => page.getByRole("grid");
const bodyRows = (page: Page) => page.locator(".dg-body [role='row']");
const names = (page: Page) => page.locator(".dg-body [role='row'] [role='gridcell'][aria-colindex='1']");
const header = (page: Page, name: string) => page.getByRole("columnheader", { name, exact: true });
const tabbable = (page: Page) => grid(page).locator("[tabindex='0']");
const selectedKeys = (page: Page) => page.getByTestId("selected-keys");

/** Focuses the grid's single tab stop, the way Tab would. */
async function focusGrid(page: Page) {
  await tabbable(page).focus();
}

/** The focused element's position, read back from the ARIA attributes. */
async function focusedCell(page: Page) {
  return page.evaluate(() => {
    const cell = document.activeElement as HTMLElement;
    return {
      role: cell.getAttribute("role"),
      row: cell.closest("[role='row']")?.getAttribute("aria-rowindex"),
      col: cell.getAttribute("aria-colindex"),
      text: cell.textContent?.trim(),
    };
  });
}

async function chooseSelection(page: Page, mode: "none" | "single" | "multi") {
  await page.getByTestId(`selection-${mode}`).check();
}

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await expect(bodyRows(page)).toHaveCount(5);
});

test.describe("rendering", () => {
  test("renders the first page with row and column counts", async ({ page }) => {
    await expect(grid(page)).toHaveAttribute("aria-rowcount", "13");
    await expect(grid(page)).toHaveAttribute("aria-colcount", "4");
    await expect(names(page)).toHaveText([
      "Zoe Bauer",
      "adam Fischer",
      "Mia Kaufmann",
      "Carol Weber",
      "bob Schneider",
    ]);
  });

  test("has no axe violations", async ({ page }) => {
    const results = await new AxeBuilder({ page }).include(".dg-wrapper").analyze();
    expect(results.violations).toEqual([]);
  });
});

// The theme switches through prefers-color-scheme, so contrast has to hold in
// both schemes independently.
test.describe("dark theme", () => {
  test.use({ colorScheme: "dark" });

  test("has no axe violations", async ({ page }) => {
    // Confirms the dark palette is actually in effect, so a pass means something.
    // Polled: the component links its stylesheet itself, so it can land after
    // the grid is already in the DOM.
    const header = page.getByRole("columnheader", { name: "Name", exact: true });
    await expect
      .poll(() => header.evaluate((el) => getComputedStyle(el).backgroundColor))
      .toBe("rgb(26, 26, 26)");

    const results = await new AxeBuilder({ page }).include(".dg-wrapper").analyze();
    expect(results.violations).toEqual([]);
  });
});

test.describe("sorting", () => {
  test("a header click cycles ascending, descending, unsorted", async ({ page }) => {
    const name = header(page, "Name");
    await expect(name).toHaveAttribute("aria-sort", "none");

    await name.click();
    await expect(name).toHaveAttribute("aria-sort", "ascending");
    // Case-insensitive, so "adam" and "bob" are not pushed behind the capitals.
    await expect(names(page)).toHaveText([
      "adam Fischer",
      "Ava Richter",
      "bob Schneider",
      "Carol Weber",
      "Hugo Brandt",
    ]);

    await name.click();
    await expect(name).toHaveAttribute("aria-sort", "descending");
    await expect(names(page).first()).toHaveText("Zoe Bauer");
    await expect(names(page).nth(1)).toHaveText("Yara Nowak");

    await name.click();
    await expect(name).toHaveAttribute("aria-sort", "none");
    await expect(names(page).first()).toHaveText("Zoe Bauer");
    await expect(names(page).nth(1)).toHaveText("adam Fischer");
  });

  test("shift-click adds a secondary sort column", async ({ page }) => {
    await header(page, "Department").click();
    await header(page, "Age").click({ modifiers: ["Shift"] });

    await expect(header(page, "Department")).toHaveAttribute("data-sort-priority", "0");
    await expect(header(page, "Age")).toHaveAttribute("data-sort-priority", "1");
    // Engineering sorted by age, Zoe before Mia because the sort is stable, then
    // finance by age.
    await expect(names(page)).toHaveText([
      "Ingrid Vogel",
      "Zoe Bauer",
      "Mia Kaufmann",
      "Theo Lang",
      "bob Schneider",
    ]);
  });

  test("a plain click replaces a multi-column sort", async ({ page }) => {
    await header(page, "Department").click();
    await header(page, "Age").click({ modifiers: ["Shift"] });
    await header(page, "Name").click();

    await expect(header(page, "Name")).toHaveAttribute("aria-sort", "ascending");
    await expect(header(page, "Department")).toHaveAttribute("aria-sort", "none");
    await expect(header(page, "Age")).toHaveAttribute("aria-sort", "none");
  });

  test("Enter sorts from the keyboard, Shift+Enter adds a column", async ({ page }) => {
    await focusGrid(page);
    await page.keyboard.press("Enter");
    await expect(header(page, "Name")).toHaveAttribute("aria-sort", "ascending");

    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("Shift+Enter");
    await expect(header(page, "Department")).toHaveAttribute("aria-sort", "ascending");
    // The first column kept its sort rather than being replaced.
    await expect(header(page, "Name")).toHaveAttribute("aria-sort", "ascending");
    await expect(header(page, "Name")).toHaveAttribute("data-sort-priority", "0");
  });
});

test.describe("filtering and search", () => {
  test("a column filter narrows the rows and the row count", async ({ page }) => {
    await page.getByRole("textbox", { name: "Filter Department" }).fill("eng");

    await expect(names(page)).toHaveText(["Zoe Bauer", "Mia Kaufmann", "Ingrid Vogel"]);
    await expect(grid(page)).toHaveAttribute("aria-rowcount", "4");
    await expect(page.locator(".dg-count")).toHaveText("3 rows");
  });

  test("column filters combine", async ({ page }) => {
    await page.getByRole("textbox", { name: "Filter Department" }).fill("eng");
    await page.getByRole("textbox", { name: "Filter Name" }).fill("mia");

    await expect(names(page)).toHaveText(["Mia Kaufmann"]);
  });

  test("search spans every filterable column", async ({ page }) => {
    // Matches by department, which is not the column being displayed first.
    await page.getByRole("searchbox").fill("research");
    await expect(names(page)).toHaveText(["Ava Richter", "Yara Nowak"]);

    // And by email.
    await page.getByRole("searchbox").fill("theo.lang@");
    await expect(names(page)).toHaveText(["Theo Lang"]);
  });

  test("searching returns to the first page", async ({ page }) => {
    await page.getByRole("button", { name: "Next page" }).click();
    await expect(page.getByRole("status").filter({ hasText: "Page" })).toHaveText("Page 2 of 3");

    await page.getByRole("searchbox").fill("e");
    await expect(page.getByRole("status").filter({ hasText: "Page" })).toHaveText(/^Page 1 of/);
  });

  test("shows an empty state when nothing matches", async ({ page }) => {
    await page.getByRole("searchbox").fill("no such employee");

    await expect(bodyRows(page)).toHaveCount(0);
    await expect(page.getByText("No matching rows")).toBeVisible();
  });
});

test.describe("paging", () => {
  const pageStatus = (page: Page) => page.getByRole("status").filter({ hasText: "Page" });

  test("moves between pages and keeps counting rows across them", async ({ page }) => {
    const previous = page.getByRole("button", { name: "Previous page" });
    const next = page.getByRole("button", { name: "Next page" });

    await expect(pageStatus(page)).toHaveText("Page 1 of 3");
    await expect(previous).toBeDisabled();

    await next.click();
    await expect(pageStatus(page)).toHaveText("Page 2 of 3");
    await expect(names(page)).toHaveText([
      "Ingrid Vogel",
      "Hugo Brandt",
      "Lena Hoffmann",
      "Nils Krause",
      "Ava Richter",
    ]);
    // Row 6 of the data, plus the header row.
    await expect(bodyRows(page).first()).toHaveAttribute("aria-rowindex", "7");
    // aria-rowcount still describes every page.
    await expect(grid(page)).toHaveAttribute("aria-rowcount", "13");

    await next.click();
    await expect(pageStatus(page)).toHaveText("Page 3 of 3");
    await expect(names(page)).toHaveText(["Theo Lang", "Yara Nowak"]);
    await expect(next).toBeDisabled();

    await previous.click();
    await expect(pageStatus(page)).toHaveText("Page 2 of 3");
  });

  test("turning paging off shows every row", async ({ page }) => {
    await page.getByTestId("toggle-paging").uncheck();

    await expect(bodyRows(page)).toHaveCount(12);
    await expect(pageStatus(page)).toHaveCount(0);
  });
});

test.describe("keyboard navigation", () => {
  test("the grid is a single tab stop", async ({ page }) => {
    await expect(tabbable(page)).toHaveCount(1);
    await expect(tabbable(page)).toHaveAttribute("role", "columnheader");
  });

  test("arrow keys, Home and End move focus and the tab stop with it", async ({ page }) => {
    await focusGrid(page);
    expect(await focusedCell(page)).toMatchObject({ role: "columnheader", row: "1", col: "1" });

    await page.keyboard.press("ArrowDown");
    expect(await focusedCell(page)).toMatchObject({ role: "gridcell", row: "2", col: "1", text: "Zoe Bauer" });

    await page.keyboard.press("ArrowRight");
    expect(await focusedCell(page)).toMatchObject({ row: "2", col: "2" });

    await page.keyboard.press("End");
    expect(await focusedCell(page)).toMatchObject({ row: "2", col: "4" });

    await page.keyboard.press("Home");
    expect(await focusedCell(page)).toMatchObject({ row: "2", col: "1" });

    // The roving tabindex followed the focus.
    await expect(tabbable(page)).toHaveCount(1);
    await expect(tabbable(page)).toHaveText("Zoe Bauer");
  });

  test("Ctrl+Home and Ctrl+End jump to the corners of the page", async ({ page }) => {
    await focusGrid(page);

    await page.keyboard.press("Control+End");
    expect(await focusedCell(page)).toMatchObject({ row: "6", col: "4" });

    await page.keyboard.press("Control+Home");
    expect(await focusedCell(page)).toMatchObject({ role: "columnheader", row: "1", col: "1" });
  });

  test("movement stops at the edges instead of wrapping", async ({ page }) => {
    await focusGrid(page);

    await page.keyboard.press("ArrowUp");
    await page.keyboard.press("ArrowLeft");
    expect(await focusedCell(page)).toMatchObject({ row: "1", col: "1" });
  });
});

test.describe("selection", () => {
  const rowAt = (page: Page, index: number): Locator => bodyRows(page).nth(index);

  test("with selection off, rows are not selectable", async ({ page }) => {
    await chooseSelection(page, "none");
    await expect(grid(page)).not.toHaveAttribute("aria-multiselectable", /.*/);

    await names(page).first().click();
    await expect(rowAt(page, 0)).not.toHaveAttribute("aria-selected", /.*/);
    await expect(selectedKeys(page)).toHaveText("selected: []");
  });

  test("single selection keeps one row", async ({ page }) => {
    await chooseSelection(page, "single");
    await expect(grid(page)).not.toHaveAttribute("aria-multiselectable", /.*/);

    await names(page).nth(0).click();
    await expect(rowAt(page, 0)).toHaveAttribute("aria-selected", "true");

    await names(page).nth(2).click();
    await expect(rowAt(page, 0)).toHaveAttribute("aria-selected", "false");
    await expect(rowAt(page, 2)).toHaveAttribute("aria-selected", "true");
    // Mia Kaufmann has id 3.
    await expect(selectedKeys(page)).toHaveText("selected: [3]");
  });

  test("multi selection accumulates clicks", async ({ page }) => {
    await chooseSelection(page, "multi");
    await expect(grid(page)).toHaveAttribute("aria-multiselectable", "true");

    await names(page).nth(0).click();
    await names(page).nth(3).click();

    await expect(selectedKeys(page)).toHaveText("selected: [1, 4]");
    await expect(page.locator(".dg-count")).toHaveText("12 rows, 2 selected");
  });

  test("Space toggles the focused row", async ({ page }) => {
    await chooseSelection(page, "multi");
    await focusGrid(page);
    await page.keyboard.press("ArrowDown");

    await page.keyboard.press(" ");
    await expect(rowAt(page, 0)).toHaveAttribute("aria-selected", "true");

    await page.keyboard.press(" ");
    await expect(rowAt(page, 0)).toHaveAttribute("aria-selected", "false");
  });

  test("Shift+Space selects a range from the anchor", async ({ page }) => {
    await chooseSelection(page, "multi");
    await focusGrid(page);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press(" ");

    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Shift+ ");

    // Rows 1 through 4 on the page: ids 1, 2, 3 and 4.
    await expect(selectedKeys(page)).toHaveText("selected: [1, 2, 3, 4]");
  });

  test("Shift+Arrow extends the selection while moving", async ({ page }) => {
    await chooseSelection(page, "multi");
    await focusGrid(page);
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press(" ");

    await page.keyboard.press("Shift+ArrowDown");
    await page.keyboard.press("Shift+ArrowDown");

    await expect(selectedKeys(page)).toHaveText("selected: [1, 2, 3]");
    expect(await focusedCell(page)).toMatchObject({ row: "4" });
  });

  test("selection survives sorting because it is keyed by row identity", async ({ page }) => {
    await chooseSelection(page, "multi");
    // Select Zoe Bauer, the first row unsorted.
    await names(page).first().click();
    await expect(selectedKeys(page)).toHaveText("selected: [1]");

    // Sort descending so Zoe stays first but on a re-rendered row.
    await header(page, "Name").click();
    await header(page, "Name").click();

    await expect(names(page).first()).toHaveText("Zoe Bauer");
    await expect(rowAt(page, 0)).toHaveAttribute("aria-selected", "true");
    await expect(rowAt(page, 1)).toHaveAttribute("aria-selected", "false");
  });
});
