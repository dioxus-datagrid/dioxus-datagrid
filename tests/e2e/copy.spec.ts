import { expect, type Page, test } from "@playwright/test";

// Phase 12: Ctrl+C puts the selection on the clipboard as tab-separated text.
//
// The tests watch what the grid hands to the clipboard rather than reading the
// clipboard back: reading it is refused in every engine without a permission
// the grid must not ask for (docs/VERIFICATION.md 15), and what matters here is
// that the right text was handed over.

const grid = (page: Page) => page.getByRole("grid");

const cell = (page: Page, row: number, column: number) =>
  grid(page)
    .locator('[role="row"]:has([role="gridcell"])')
    .nth(row)
    .getByRole("gridcell")
    .nth(column);

async function open(
  page: Page,
  cells: "none" | "single" | "range" = "range",
  rows: "none" | "single" | "multi" = "multi",
) {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(page.locator(".dg-count")).toHaveText("12 rows");
  await page.getByTestId(`cell-selection-${cells}`).check();
  await page.getByTestId(`selection-${rows}`).check();

  // Both clipboard routes report what they were given instead of writing.
  await page.evaluate(() => {
    const copied: string[] = [];
    (window as unknown as { __copied: string[] }).__copied = copied;
    if (navigator.clipboard) {
      navigator.clipboard.writeText = async (text: string) => {
        copied.push(text);
      };
    }
    document.execCommand = (command: string) => {
      if (command === "copy") {
        copied.push(document.querySelector("textarea")?.value ?? "");
      }
      return true;
    };
  });
}

/** What the grid handed to the clipboard, in order. */
const copied = (page: Page) =>
  page.evaluate(() => (window as unknown as { __copied: string[] }).__copied);

async function copy(page: Page) {
  await page.keyboard.press("Control+c");
  await expect.poll(async () => (await copied(page)).length).toBeGreaterThan(0);
  return (await copied(page))[0];
}

test("a rectangle copies as rows of tab-separated cells", async ({ page }) => {
  await open(page);
  await cell(page, 0, 1).click();
  await page.keyboard.press("Shift+ArrowRight");
  await page.keyboard.press("Shift+ArrowDown");

  // Email and Department of the first two people, the rows separated by CRLF.
  expect(await copy(page)).toBe(
    "zoe.bauer@example.com\tengineering\r\nadam.fischer@example.com\tsales",
  );
});

test("one cell copies as itself, without a line break", async ({ page }) => {
  await open(page);
  await cell(page, 2, 0).click();

  const text = await copy(page);
  expect(text).toBe("Mia Kaufmann");
  expect(text).not.toContain("\r\n");
});

test("selected rows copy every visible column", async ({ page }) => {
  // No cell selection, so the row selection is what Ctrl+C means.
  await open(page, "none");
  await cell(page, 0, 0).click();
  await cell(page, 2, 0).click({ modifiers: ["Control"] });

  expect(await copy(page)).toBe(
    [
      "Zoe Bauer\tzoe.bauer@example.com\tengineering\t30",
      "Mia Kaufmann\tmia.kaufmann@example.com\tengineering\t30",
    ].join("\r\n"),
  );
});

test("with nothing selected the focused cell is copied", async ({ page }) => {
  // Neither rows nor cells can be selected, so a click only moves the focus.
  await open(page, "none", "none");
  await cell(page, 0, 1).click();

  expect(await copy(page)).toBe("zoe.bauer@example.com");
});

test("a header copies nothing", async ({ page }) => {
  await open(page, "none", "none");
  await page.getByRole("columnheader", { name: "Name" }).evaluate((el: HTMLElement) => el.focus());

  await page.keyboard.press("Control+c");
  // Nothing to copy, so nothing was handed over — and the page did not error.
  await expect.poll(async () => (await copied(page)).length).toBe(0);
});

test("the copy survives the older clipboard route", async ({ page }) => {
  await open(page);
  // The modern API refuses, as a headless browser without the permission does.
  await page.evaluate(() => {
    navigator.clipboard.writeText = () => Promise.reject(new Error("denied"));
  });
  await cell(page, 0, 0).click();

  // execCommand still reports what it was given.
  expect(await copy(page)).toBe("Zoe Bauer");
});
