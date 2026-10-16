import { expect, type Locator, type Page, test } from "@playwright/test";

// The real grid told to read right to left (`direction: Direction::Rtl`, ADR-0044):
// what holds, and what the option mirrors. The measurements behind it are in
// docs/VERIFICATION.md §23. The setup is the one of column-virtualization.spec.ts.

const grid = (page: Page) => page.getByRole("grid");
const header = (page: Page, name: string) => page.getByRole("columnheader", { name, exact: true });

async function open(page: Page, { pinned = false, rtl = true, wide = true } = {}) {
  await page.setViewportSize({ width: 900, height: 900 });
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  if (wide) {
    await page.getByTestId("toggle-wide").check();
    await page.getByTestId("toggle-virtual-columns").check();
  }
  if (pinned) await page.getByTestId("toggle-pinning").check();
  if (rtl) await page.getByTestId("toggle-rtl").check();
  await expect(grid(page)).toBeVisible();
  if (rtl) await expect(grid(page)).toHaveAttribute("dir", "rtl");
  else await expect(grid(page)).not.toHaveAttribute("dir", "rtl");
}

/** Scrolls by a distance from the start edge, which is a negative number in RTL. */
const scrollFromStart = (page: Page, distance: number) =>
  grid(page).evaluate((root, distance) => {
    root.scrollLeft = getComputedStyle(root).direction === "rtl" ? -distance : distance;
  }, distance);

/** The focused cell: its column, and whether it is wholly inside the visible part of the grid. */
const focused = (page: Page) =>
  page.evaluate(() => {
    const el = document.activeElement as HTMLElement;
    const root = el.closest("[role='grid']") as HTMLElement;
    const c = el.getBoundingClientRect();
    const r = root.getBoundingClientRect();
    return {
      col: Number(el.getAttribute("aria-colindex")),
      inside: c.left >= r.left - 1 && c.right <= r.left + root.clientWidth + 1,
    };
  });

/** Focuses the first body cell of the first column. */
async function focusFirstCell(page: Page) {
  await header(page, "Name").click();
  await page.keyboard.press("ArrowDown");
  expect(await focused(page)).toEqual({ col: 1, inside: true });
}

test.describe("what holds", () => {
  test("the columns in view follow a negative scrollLeft", async ({ page }) => {
    await open(page);
    await scrollFromStart(page, 12000);
    await expect(header(page, "Metric 100")).toBeVisible();
    await expect(header(page, "Metric 1")).toHaveCount(0);
    expect(await grid(page).evaluate((root) => root.scrollLeft)).toBe(-12000);
  });

  test("headers and cells line up under it", async ({ page }) => {
    await open(page);
    await scrollFromStart(page, 12000);
    await expect(header(page, "Metric 100")).toBeVisible();
    const misaligned = await grid(page).evaluate((root) => {
      const heads = [...root.querySelectorAll(".dg-head [role='row']:last-child [role='columnheader']")];
      const cells = [...root.querySelectorAll(".dg-body [role='row']:first-child [role='gridcell']")];
      return heads.filter((head) => {
        const index = head.getAttribute("aria-colindex");
        const cell = cells.find((cell) => cell.getAttribute("aria-colindex") === index);
        const h = head.getBoundingClientRect();
        const c = cell?.getBoundingClientRect();
        return !c || Math.round(h.left) !== Math.round(c.left) || Math.round(h.width) !== Math.round(c.width);
      }).length;
    });
    expect(misaligned).toBe(0);
  });

  test("the first column starts at the right edge", async ({ page }) => {
    await open(page);
    const gap = await header(page, "Name").evaluate((head) => {
      const root = head.closest("[role='grid']") as HTMLElement;
      return Math.round(root.getBoundingClientRect().left + root.clientWidth - head.getBoundingClientRect().right);
    });
    expect(gap).toBe(0);
  });

  test("pinned columns hold their logical edge: start on the right, end on the left", async ({ page }) => {
    await open(page, { pinned: true });
    await scrollFromStart(page, 9000);
    const sides = await grid(page).evaluate((root) => {
      const box = root.getBoundingClientRect();
      const right = box.left + root.clientWidth;
      const start = root.querySelector("[role='columnheader'][data-pinned='start']") as HTMLElement;
      const end = root.querySelector("[role='columnheader'][data-pinned='end']") as HTMLElement;
      return {
        start: Math.round(right - start.getBoundingClientRect().right),
        end: Math.round(end.getBoundingClientRect().left - box.left),
      };
    });
    expect(sides).toEqual({ start: 0, end: 0 });
  });

  test("Home and End reach the first and the last column, scrolled into view", async ({ page }) => {
    await open(page);
    await focusFirstCell(page);
    await page.keyboard.press("End");
    expect(await focused(page)).toEqual({ col: Number(await grid(page).getAttribute("aria-colcount")), inside: true });
    await page.keyboard.press("Home");
    await expect.poll(() => focused(page).then((f) => f.col)).toBe(1);
  });
});

test.describe("what the direction mirrors", () => {
  test("the arrow keys follow the screen: left goes on to the next column, right back", async ({ page }) => {
    await open(page);
    await focusFirstCell(page);

    await page.keyboard.press("ArrowLeft");
    expect((await focused(page)).col).toBe(2);
    await page.keyboard.press("ArrowLeft");
    expect((await focused(page)).col).toBe(3);
    await page.keyboard.press("ArrowRight");
    expect((await focused(page)).col).toBe(2);
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    // Nothing before the first column.
    expect((await focused(page)).col).toBe(1);
  });

  test("left to right is unchanged", async ({ page }) => {
    await open(page, { rtl: false });
    await focusFirstCell(page);
    await page.keyboard.press("ArrowRight");
    expect((await focused(page)).col).toBe(2);
    await page.keyboard.press("ArrowLeft");
    expect((await focused(page)).col).toBe(1);
  });

  test("every column the keyboard reaches is scrolled wholly into view", async ({ page }) => {
    await open(page);
    await focusFirstCell(page);
    const outside: number[] = [];
    for (let i = 0; i < 40; i++) {
      await page.keyboard.press("ArrowLeft");
      // The scroll is set in the same pass; give the browser a frame to apply it.
      await page.waitForTimeout(40);
      const f = await focused(page);
      if (!f.inside) outside.push(f.col);
    }
    expect(outside).toEqual([]);
  });

  test("and back again, towards the start", async ({ page }) => {
    await open(page);
    await focusFirstCell(page);
    await page.keyboard.press("End");
    const outside: number[] = [];
    for (let i = 0; i < 40; i++) {
      await page.keyboard.press("ArrowRight");
      await page.waitForTimeout(40);
      const f = await focused(page);
      if (!f.inside) outside.push(f.col);
    }
    expect(outside).toEqual([]);
  });

  test("the line at the edge of a pinned block is on the side that faces the other columns", async ({ page }) => {
    await open(page, { pinned: true, wide: false });
    const shadows = await grid(page).evaluate((root) => {
      const row = root.querySelector(".dg-body [role='row']") as HTMLElement;
      const starts = [...row.querySelectorAll("[data-pinned='start']")] as HTMLElement[];
      const ends = [...row.querySelectorAll("[data-pinned='end']")] as HTMLElement[];
      // The horizontal offset of the first (and only) shadow of a cell.
      const offset = (el: HTMLElement | undefined) => {
        const match = el ? getComputedStyle(el).boxShadow.match(/(-?\d+)px\s+-?\d+px/) : null;
        return match ? Number(match[1]) : null;
      };
      return { start: offset(starts.at(-1)), end: offset(ends[0]) };
    });
    // The start block is on the right, so its line is on its left; the end block
    // is on the left, so its line is on its right.
    expect(shadows).toEqual({ start: -1, end: 1 });
  });

  test.describe("column resizing", () => {
    const edge = async (head: Locator) =>
      head.evaluate((el) => {
        const handle = el.querySelector("[data-resize-handle]") as HTMLElement;
        const h = el.getBoundingClientRect();
        const r = handle.getBoundingClientRect();
        return { handleLeftGap: Math.round(r.left - h.left), handleRightGap: Math.round(h.right - r.right) };
      });

    test("the handle is at the end of the column, which is its left edge", async ({ page }) => {
      await open(page);
      const gaps = await edge(header(page, "Name"));
      expect(gaps.handleLeftGap).toBe(0);
      expect(gaps.handleRightGap).toBeGreaterThan(40);
    });

    test("left to right keeps it at the right edge", async ({ page }) => {
      await open(page, { rtl: false });
      const gaps = await edge(header(page, "Name"));
      expect(gaps.handleRightGap).toBe(0);
    });

    test("dragging it towards the left widens the column", async ({ page }) => {
      await open(page);
      const head = header(page, "Name");
      const before = (await head.boundingBox())!.width;
      const handle = (await head.locator("[data-resize-handle]").boundingBox())!;
      const x = handle.x + handle.width / 2;
      const y = handle.y + handle.height / 2;
      await page.mouse.move(x, y);
      await page.mouse.down();
      await page.mouse.move(x - 40, y, { steps: 6 });
      await page.mouse.up();
      await expect.poll(async () => (await head.boundingBox())!.width).toBe(before + 40);
    });

    test("Alt+ArrowLeft widens the column, Alt+ArrowRight narrows it", async ({ page }) => {
      await open(page);
      const head = header(page, "Name");
      await head.click();
      const before = (await head.boundingBox())!.width;
      await page.keyboard.press("Alt+ArrowLeft");
      await expect.poll(async () => (await head.boundingBox())!.width).toBeGreaterThan(before);
      await page.keyboard.press("Alt+ArrowRight");
      await page.keyboard.press("Alt+ArrowRight");
      await expect.poll(async () => (await head.boundingBox())!.width).toBeLessThan(before);
    });
  });
});

test.describe("groups", () => {
  // As in a tree: opening is going on, which is left in right to left text. The
  // setup is the one of grouping.spec.ts, with the direction on.
  test("right collapses a group, left expands it", async ({ page }) => {
    await page.goto("/");
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await page.getByTestId("toggle-grouping").check();
    await page.getByTestId("toggle-paging").uncheck();
    await page.getByTestId("toggle-rtl").check();
    await page.locator(".dg-group-panel").getByRole("combobox", { name: "Group by" }).selectOption("department");
    // Grouped, the grid is a treegrid, which `getByRole("grid")` does not match.
    const root = page.locator(".dg");
    await expect(root).toHaveAttribute("role", "treegrid");

    const engineering = page.locator("[data-group-row]").filter({ hasText: "Department: engineering" });
    await root.locator("[tabindex='0']").focus();
    await page.keyboard.press("ControlOrMeta+Home");
    await page.keyboard.press("ArrowDown");
    await expect(engineering.locator("[data-group-cell]")).toBeFocused();

    await page.keyboard.press("ArrowRight");
    await expect(engineering).toHaveAttribute("aria-expanded", "false");
    await page.keyboard.press("ArrowLeft");
    await expect(engineering).toHaveAttribute("aria-expanded", "true");
  });
});
