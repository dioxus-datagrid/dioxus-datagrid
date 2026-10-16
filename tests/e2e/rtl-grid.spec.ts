import { expect, type Page, test } from "@playwright/test";

// The real grid under dir="rtl": what already holds, as a guard for the rest of
// the RTL work of Phase 14. What does not hold yet is listed in
// docs/VERIFICATION.md §23 and gets its tests when it is fixed. The setup is the
// one of column-virtualization.spec.ts, with the direction switched on the
// document.

const grid = (page: Page) => page.getByRole("grid");

async function open(page: Page, { pinned = false } = {}) {
  await page.setViewportSize({ width: 900, height: 900 });
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.getByTestId("toggle-wide").check();
  if (pinned) await page.getByTestId("toggle-pinning").check();
  await page.getByTestId("toggle-virtual-columns").check();
  await page.evaluate(() => document.documentElement.setAttribute("dir", "rtl"));
  await expect(grid(page)).toBeVisible();
}

/** Scrolls by a distance from the start edge, which is a negative number in RTL. */
const scrollFromStart = (page: Page, distance: number) =>
  grid(page).evaluate((root, distance) => {
    root.scrollLeft = -distance;
  }, distance);

test("the columns in view follow a negative scrollLeft", async ({ page }) => {
  await open(page);
  await scrollFromStart(page, 12000);
  // The same columns an LTR grid shows 12000px from its start edge.
  await expect(page.getByRole("columnheader", { name: "Metric 100", exact: true })).toBeVisible();
  await expect(page.getByRole("columnheader", { name: "Metric 1", exact: true })).toHaveCount(0);
  expect(await grid(page).evaluate((root) => root.scrollLeft)).toBe(-12000);
});

test("headers and cells line up under it", async ({ page }) => {
  await open(page);
  await scrollFromStart(page, 12000);
  await expect(page.getByRole("columnheader", { name: "Metric 100", exact: true })).toBeVisible();
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
  const gap = await page.getByRole("columnheader", { name: "Name", exact: true }).evaluate((head) => {
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
  await page.getByRole("columnheader", { name: "Name", exact: true }).click();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("End");
  const last = await page.evaluate(() => {
    const el = document.activeElement as HTMLElement;
    const root = el.closest("[role='grid']") as HTMLElement;
    const c = el.getBoundingClientRect();
    const r = root.getBoundingClientRect();
    return { col: el.getAttribute("aria-colindex"), inside: c.left >= r.left - 1 && c.right <= r.left + root.clientWidth + 1 };
  });
  expect(last).toEqual({ col: await grid(page).getAttribute("aria-colcount"), inside: true });

  await page.keyboard.press("Home");
  await expect.poll(() => page.evaluate(() => document.activeElement?.getAttribute("aria-colindex"))).toBe("1");
});
