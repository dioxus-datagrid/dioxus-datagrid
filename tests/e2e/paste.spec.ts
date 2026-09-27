import { expect, type Page, test } from "@playwright/test";

// Phase 12: pasting tab-separated text into editable cells.
//
// A real Ctrl+V cannot be synthesized: keys injected over the debugging protocol
// do not paste, not even into a plain input, and reading the clipboard back is
// refused anyway (docs/VERIFICATION.md 15). So the tests dispatch the same event
// the browser dispatches for a paste — a paste event carrying a DataTransfer —
// which exercises everything from the listener inwards. Whether the keystroke
// reaches a page whose focus is on a grid cell is measured by hand instead; see
// docs/VERIFICATION.md 15.

const grid = (page: Page) => page.getByRole("grid");
const status = (page: Page) => page.locator(".dg-edit-status");
const selectedCells = (page: Page) => page.locator("[data-cell-selected]");
const bodyRows = (page: Page) => grid(page).locator('[role="row"]:has([role="gridcell"])');

/** The cell at a row and column of the body, both zero-based. */
const cell = (page: Page, row: number, column: number) =>
  bodyRows(page).nth(row).getByRole("gridcell").nth(column);

async function open(page: Page, editing: "off" | "cell" | "batch" = "cell") {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.locator(".dg-count")).toHaveText("12 rows");
  await page.getByTestId("cell-selection-range").check();
  await page.getByTestId(`edit-${editing}`).check();
}

/**
 * Hands the page a paste, the way the browser does: a paste event carrying a
 * DataTransfer, dispatched at whatever has focus.
 */
async function paste(page: Page, text: string) {
  await page.evaluate((text) => {
    const data = new DataTransfer();
    data.setData("text/plain", text);
    const event = new ClipboardEvent("paste", {
      clipboardData: data,
      bubbles: true,
      cancelable: true,
    });
    // WebKit ignores clipboardData in the constructor, so it is put in place.
    if (!event.clipboardData) {
      Object.defineProperty(event, "clipboardData", { value: data });
    }
    (document.activeElement ?? document.body).dispatchEvent(event);
  }, text);
}

test("a block lands at the focused cell and spreads right and down", async ({ page }) => {
  await open(page);
  await cell(page, 0, 0).click();

  await paste(page, "Zora Bauer\tzora@example.com\r\nAdi Fischer\tadi@example.com");

  await expect(cell(page, 0, 0)).toHaveText("Zora Bauer");
  await expect(cell(page, 0, 1)).toHaveText("zora@example.com");
  await expect(cell(page, 1, 0)).toHaveText("Adi Fischer");
  await expect(cell(page, 1, 1)).toHaveText("adi@example.com");
  // Nothing beyond the block moved.
  await expect(cell(page, 0, 2)).toHaveText("engineering");
  await expect(cell(page, 2, 0)).toHaveText("Mia Kaufmann");
  await expect(status(page)).toHaveText("Saved");
});

test("one cell fills the selected rectangle", async ({ page }) => {
  await open(page);
  // The Department column of the first three rows.
  await cell(page, 0, 2).click();
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Shift+ArrowDown");

  await paste(page, "sales");

  await expect(cell(page, 0, 2)).toHaveText("sales");
  await expect(cell(page, 1, 2)).toHaveText("sales");
  await expect(cell(page, 2, 2)).toHaveText("sales");
  await expect(cell(page, 3, 2)).not.toHaveText("sales");
});

test("the pasted block is what stays selected", async ({ page }) => {
  await open(page);
  await cell(page, 0, 0).click();

  await paste(page, "Zora\tzora@example.com\r\nAdi\tadi@example.com");

  // Two rows by two columns, and the corner is where the paste started.
  await expect(selectedCells(page)).toHaveCount(4);
  await expect(cell(page, 0, 0)).toHaveAttribute("aria-selected", "true");
  await expect(cell(page, 1, 1)).toHaveAttribute("aria-selected", "true");
  await expect(cell(page, 0, 2)).toHaveAttribute("aria-selected", "false");
});

test("a value the column refuses leaves its whole row alone", async ({ page }) => {
  await open(page);
  await cell(page, 0, 0).click();

  // The first row's email is not one; the second row is fine.
  await paste(page, "Zora\tnot-an-email\r\nAdi\tadi@example.com");

  await expect(cell(page, 0, 0)).toHaveText("Zoe Bauer");
  await expect(cell(page, 1, 0)).toHaveText("Adi");
  await expect(status(page)).toHaveText("Could not paste one row");
  await expect(status(page)).toHaveAttribute("data-state", "failed");
});

test("what hangs over the last column is dropped", async ({ page }) => {
  await open(page);
  // Age is the last of the four visible columns.
  await cell(page, 0, 3).click();

  await paste(page, "44\t99");

  await expect(cell(page, 0, 3)).toHaveText("44");
  // There is no fifth column for the second value, so it is gone rather than
  // wrapped onto the next row or column.
  await expect(grid(page).getByRole("gridcell", { name: "99", exact: true })).toHaveCount(0);
});

test("a paste past the last row adds none", async ({ page }) => {
  await open(page);
  const before = await bodyRows(page).count();
  await cell(page, before - 1, 0).click();

  await paste(page, "Last\r\nOver the edge");

  await expect(cell(page, before - 1, 0)).toHaveText("Last");
  await expect(bodyRows(page)).toHaveCount(before);
  await expect(grid(page).getByRole("gridcell", { name: "Over the edge" })).toHaveCount(0);
});

test("an open editor keeps its own paste", async ({ page }) => {
  await open(page);
  const below = (await cell(page, 1, 0).textContent()) ?? "";
  await cell(page, 0, 0).click();
  await page.keyboard.press("Enter");
  await expect(grid(page).locator("[data-editor]")).toBeFocused();

  await paste(page, "Zora\r\nAdi");
  await page.keyboard.press("Escape");

  // The grid stayed out of it: nothing was written behind the editor's back.
  await expect(cell(page, 0, 0)).toHaveText("Zoe Bauer");
  await expect(cell(page, 1, 0)).toHaveText(below);
});

test("a grid that cannot be edited cannot be pasted into", async ({ page }) => {
  await open(page, "off");
  await cell(page, 0, 0).click();

  await paste(page, "Zora");

  await expect(cell(page, 0, 0)).toHaveText("Zoe Bauer");
});

test("a paste in a batch is collected rather than saved", async ({ page }) => {
  await open(page, "batch");
  await cell(page, 0, 0).click();

  await paste(page, "Zora\r\nAdi");

  await expect(cell(page, 0, 0)).toHaveText("Zora");
  await expect(status(page)).toHaveText("2 unsaved changes");
  // Discarding takes the paste back, which is what a batch is for.
  await page.getByRole("button", { name: "Discard changes" }).click();
  await expect(cell(page, 0, 0)).toHaveText("Zoe Bauer");
});

test("a header cannot be pasted into", async ({ page }) => {
  await open(page);
  await page.getByRole("columnheader", { name: "Name" }).evaluate((el: HTMLElement) => el.focus());

  await paste(page, "Zora");

  await expect(cell(page, 0, 0)).toHaveText("Zoe Bauer");
  await expect(status(page)).toHaveAttribute("data-state", "idle");
});
