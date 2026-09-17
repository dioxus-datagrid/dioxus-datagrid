import { expect, type Page, test } from "@playwright/test";

// Phase 5: column resizing, showing and hiding columns, and state that survives
// a reload. The playground's columns are Name, Email, Department (all Auto)
// and Age (fixed 88px), none with a min_width, so the minimum is the library's
// default of 48px.

const MIN_WIDTH = 48;
const STEP = 16;

const grid = (page: Page) => page.getByRole("grid");
const header = (page: Page, name: string) => page.getByRole("columnheader", { name, exact: true });
const handle = (page: Page, name: string) => header(page, name).locator("[data-resize-handle]");
const picker = (page: Page) => page.locator(".dg-columns");
const pickerOption = (page: Page, name: string) => picker(page).getByRole("checkbox", { name });

async function widthOf(page: Page, name: string) {
  return header(page, name).evaluate((el) => el.getBoundingClientRect().width);
}

/** Drags a column's resize handle horizontally by `dx` pixels, in steps. */
async function dragHandle(page: Page, name: string, dx: number) {
  const box = await handle(page, name).boundingBox();
  if (!box) throw new Error(`no resize handle on ${name}`);
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + dx, y, { steps: 8 });
  await page.mouse.up();
}

async function openPicker(page: Page) {
  await picker(page).locator("summary").click();
}

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await expect(grid(page)).toHaveAttribute("aria-colcount", "4");
  // The component links its stylesheet itself, and WebKit can render the grid
  // before it applies. Until then every header spans the page, and any width
  // measured is meaningless. Age is fixed at 88px once the layout is in.
  await expect.poll(() => widthOf(page, "Age")).toBeCloseTo(88, 0);
});

test.describe("resizing", () => {
  test("dragging a handle widens the column by the distance dragged", async ({ page }) => {
    const before = await widthOf(page, "Name");
    await dragHandle(page, "Name", 120);
    await expect.poll(() => widthOf(page, "Name")).toBeCloseTo(before + 120, 0);
  });

  test("a drag that ends over the header does not sort it", async ({ page }) => {
    await dragHandle(page, "Name", -30);
    await expect(header(page, "Name")).toHaveAttribute("aria-sort", "none");

    // And the next real click still sorts.
    await header(page, "Name").click();
    await expect(header(page, "Name")).toHaveAttribute("aria-sort", "ascending");
  });

  test("resizing stops at the minimum width", async ({ page }) => {
    await dragHandle(page, "Email", -1000);
    await expect.poll(() => widthOf(page, "Email")).toBeCloseTo(MIN_WIDTH, 0);
  });

  test("the pointer is followed beyond the narrow handle", async ({ page }) => {
    // Well past the handle and over the next column: the grid root tracks the
    // drag, not the handle (ADR-0002). Kept inside the 1280px browser window,
    // outside of which no pointer events exist at all.
    const before = await widthOf(page, "Department");
    await dragHandle(page, "Department", 150);
    await expect.poll(() => widthOf(page, "Department")).toBeCloseTo(before + 150, 0);
  });

  test("widening the last column keeps following the pointer outside the grid", async ({ page }) => {
    const gridRight = await grid(page).evaluate((el) => el.getBoundingClientRect().right);
    const handleX = (await handle(page, "Age").boundingBox())!.x;
    const before = await widthOf(page, "Age");

    // Ends past the grid's right edge, where only the drag overlay sees it, but
    // inside the browser window, beyond which there are no events to follow.
    const windowRight = await page.evaluate(() => window.innerWidth);
    const dx = Math.floor(windowRight - 8 - handleX);
    expect(handleX + dx).toBeGreaterThan(gridRight + 20);
    await dragHandle(page, "Age", dx);
    await expect.poll(() => widthOf(page, "Age")).toBeCloseTo(before + dx, 0);
    await expect(page.locator("[data-resize-overlay]")).toHaveCount(0);
  });

  test("Alt+Arrow keys resize the focused header", async ({ page }) => {
    await grid(page).locator("[tabindex='0']").focus();
    await expect(header(page, "Name")).toBeFocused();
    await expect(header(page, "Name")).toHaveAttribute("aria-keyshortcuts", "Alt+ArrowLeft Alt+ArrowRight");

    const before = await widthOf(page, "Name");
    await page.keyboard.press("Alt+ArrowRight");
    await expect.poll(() => widthOf(page, "Name")).toBeCloseTo(before + STEP, 0);
    await page.keyboard.press("Alt+ArrowLeft");
    await page.keyboard.press("Alt+ArrowLeft");
    await expect.poll(() => widthOf(page, "Name")).toBeCloseTo(before - STEP, 0);

    // Resizing is not navigation: focus stays on the same header.
    await expect(header(page, "Name")).toBeFocused();
  });

  test("double-clicking a handle restores the defined width", async ({ page }) => {
    const before = await widthOf(page, "Age");
    await dragHandle(page, "Age", 80);
    await expect.poll(() => widthOf(page, "Age")).toBeCloseTo(before + 80, 0);

    await handle(page, "Age").dblclick();
    await expect.poll(() => widthOf(page, "Age")).toBeCloseTo(before, 0);
  });
});

test.describe("showing and hiding columns", () => {
  test("unchecking a column removes it and renumbers the rest", async ({ page }) => {
    await openPicker(page);
    await pickerOption(page, "Email").uncheck();

    await expect(grid(page)).toHaveAttribute("aria-colcount", "3");
    await expect(header(page, "Email")).toHaveCount(0);
    await expect(header(page, "Department")).toHaveAttribute("aria-colindex", "2");

    await pickerOption(page, "Email").check();
    await expect(grid(page)).toHaveAttribute("aria-colcount", "4");
    await expect(header(page, "Email")).toHaveAttribute("aria-colindex", "2");
  });

  test("the last visible column cannot be hidden", async ({ page }) => {
    await openPicker(page);
    for (const name of ["Name", "Email", "Department"]) {
      await pickerOption(page, name).uncheck();
    }
    await expect(grid(page)).toHaveAttribute("aria-colcount", "1");
    await expect(pickerOption(page, "Age")).toBeChecked();
    await expect(pickerOption(page, "Age")).toBeDisabled();
  });

  test("hiding the focused column keeps the grid a single tab stop", async ({ page }) => {
    await grid(page).locator("[tabindex='0']").focus();
    for (let i = 0; i < 3; i++) await page.keyboard.press("ArrowRight");
    await expect(header(page, "Age")).toBeFocused();

    await openPicker(page);
    await pickerOption(page, "Age").uncheck();

    const tabStops = grid(page).locator("[tabindex='0']");
    await expect(tabStops).toHaveCount(1);
    await expect(tabStops).toHaveAttribute("aria-colindex", "3");
  });
});

test.describe("persistence", () => {
  test("sort, widths and hidden columns survive a reload", async ({ page }) => {
    await header(page, "Name").click();
    await expect(header(page, "Name")).toHaveAttribute("aria-sort", "ascending");
    await dragHandle(page, "Name", 100);
    const width = await widthOf(page, "Name");
    await openPicker(page);
    await pickerOption(page, "Email").uncheck();
    await expect(grid(page)).toHaveAttribute("aria-colcount", "3");

    await page.reload();

    await expect(grid(page)).toHaveAttribute("aria-colcount", "3");
    await expect(header(page, "Name")).toHaveAttribute("aria-sort", "ascending");
    await expect.poll(() => widthOf(page, "Name")).toBeCloseTo(width, 0);
  });

  test("resetting the saved state starts over", async ({ page }) => {
    await header(page, "Name").click();
    await expect(header(page, "Name")).toHaveAttribute("aria-sort", "ascending");

    await page.getByTestId("reset-state").click();
    await expect(header(page, "Name")).toHaveAttribute("aria-sort", "none");
    await page.reload();
    await expect(header(page, "Name")).toHaveAttribute("aria-sort", "none");
  });
});
