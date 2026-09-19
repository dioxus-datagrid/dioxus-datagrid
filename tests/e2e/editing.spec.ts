import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 9: editing, with the keyboard only. Pointer clicks set up the
// playground's controls; everything in the grid happens through keys.
// The first row is Zoe Bauer, zoe.bauer@example.com, engineering, 30.

const grid = (page: Page) => page.getByRole("grid");
const bodyRows = (page: Page) => grid(page).locator(".dg-body [role='row']");
const cell = (page: Page, row: number, column: number) =>
  bodyRows(page).nth(row).locator(`[role='gridcell'][aria-colindex='${column}']`);
const count = (page: Page) => page.locator(".dg-count");
const status = (page: Page) => page.locator(".dg-edit-status");
const editor = (page: Page) => grid(page).locator("[data-editor]");
const SELECT_ALL = "ControlOrMeta+A";

async function editMode(page: Page, mode: "cell" | "row" | "dialog" | "batch") {
  await page.getByTestId(`edit-${mode}`).check();
}

/** Moves grid focus to a body cell the way a keyboard user does: into the
 * grid, then down and across with the arrow keys. `row` counts from 0 on the
 * page, `column` from 1. */
async function goTo(page: Page, row: number, column: number) {
  await grid(page).locator("[tabindex='0']").focus();
  await page.keyboard.press("ControlOrMeta+Home");
  for (let step = 0; step <= row; step++) await page.keyboard.press("ArrowDown");
  for (let step = 1; step < column; step++) await page.keyboard.press("ArrowRight");
  await expect(cell(page, row, column)).toBeFocused();
}

/** Types over whatever the focused editor holds. */
async function retype(page: Page, text: string) {
  await page.keyboard.press(SELECT_ALL);
  await page.keyboard.type(text);
}

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(count(page)).toHaveText("12 rows");
});

test("without an editor, Enter on a cell edits nothing", async ({ page }) => {
  await goTo(page, 0, 1);
  await page.keyboard.press("Enter");
  await expect(editor(page)).toHaveCount(0);
  await expect(page.locator("[aria-readonly]")).toHaveCount(0);
});

test.describe("cell editing", () => {
  test.beforeEach(async ({ page }) => editMode(page, "cell"));

  test("Enter edits, Tab commits and moves on, Escape cancels", async ({ page }) => {
    await goTo(page, 0, 1);
    await page.keyboard.press("Enter");
    const name = grid(page).getByRole("textbox", { name: "Name" });
    await expect(name).toBeFocused();
    await expect(name).toHaveValue("Zoe Bauer");

    await retype(page, "Zoe Brandt");
    await page.keyboard.press("Tab");
    const email = grid(page).getByRole("textbox", { name: "Email" });
    await expect(email).toBeFocused();
    await expect(cell(page, 0, 1)).toHaveText("Zoe Brandt");

    await retype(page, "not changed");
    await page.keyboard.press("Escape");
    await expect(editor(page)).toHaveCount(0);
    await expect(cell(page, 0, 2)).toBeFocused();
    await expect(cell(page, 0, 2)).toHaveText("zoe.bauer@example.com");
  });

  test("Enter commits and moves down, F2 edits there", async ({ page }) => {
    await goTo(page, 0, 4);
    await page.keyboard.press("F2");
    await retype(page, "31");
    await page.keyboard.press("Enter");
    await expect(cell(page, 0, 4)).toHaveText("31");
    await expect(cell(page, 1, 4)).toBeFocused();
    await expect(status(page)).toHaveText("Saved");

    // Arrow keys work again once the edit is over.
    await page.keyboard.press("ArrowUp");
    await expect(cell(page, 0, 4)).toBeFocused();
  });

  test("a value that is refused keeps the editor open with its message", async ({ page }) => {
    await goTo(page, 0, 4);
    await page.keyboard.press("Enter");
    await retype(page, "old");
    await page.keyboard.press("Enter");

    const age = grid(page).getByRole("textbox", { name: "Age" });
    await expect(age).toHaveAttribute("aria-invalid", "true");
    await expect(grid(page).getByRole("alert")).toHaveText("Enter a number");
    await expect(age).toBeFocused();

    // The column's own rule, after the number was read.
    await retype(page, "12");
    await page.keyboard.press("Enter");
    await expect(grid(page).getByRole("alert")).toHaveText("Between 16 and 99");

    await page.keyboard.press("Escape");
    await expect(cell(page, 0, 4)).toHaveText("30");
    await expect(cell(page, 0, 4)).toBeFocused();
  });

  test("a list column offers its choices", async ({ page }) => {
    await goTo(page, 0, 3);
    await page.keyboard.press("Enter");
    const department = grid(page).getByRole("combobox", { name: "Department" });
    await expect(department).toBeFocused();
    await department.selectOption("legal");
    await page.keyboard.press("Enter");
    await expect(cell(page, 0, 3)).toHaveText("legal");
  });

  test("a failed save takes the edit back and says why", async ({ page }) => {
    await page.getByTestId("toggle-fail-saves").check();
    await goTo(page, 0, 1);
    await page.keyboard.press("Enter");
    await retype(page, "Nobody");
    await page.keyboard.press("Enter");

    await expect(status(page)).toHaveText("Could not save: saving is switched off");
    await expect(cell(page, 0, 1)).toHaveText("Zoe Bauer");
  });

  test("an open editor has no axe violations, with or without an error", async ({ page }) => {
    await goTo(page, 0, 4);
    await page.keyboard.press("Enter");
    await expect(editor(page)).toBeFocused();
    let results = await new AxeBuilder({ page }).include(".dg-wrapper").analyze();
    expect(results.violations).toEqual([]);

    await retype(page, "old");
    await page.keyboard.press("Enter");
    await expect(grid(page).getByRole("alert")).toBeVisible();
    results = await new AxeBuilder({ page }).include(".dg-wrapper").analyze();
    expect(results.violations).toEqual([]);
  });
});

test.describe("row editing", () => {
  test.beforeEach(async ({ page }) => editMode(page, "row"));

  test("Enter edits the whole row, Tab moves between its editors, Enter saves", async ({
    page,
  }) => {
    await goTo(page, 1, 2);
    await page.keyboard.press("Enter");
    await expect(editor(page)).toHaveCount(4);
    const email = grid(page).getByRole("textbox", { name: "Email" });
    await expect(email).toBeFocused();

    await page.keyboard.press("Tab");
    await expect(grid(page).getByRole("combobox", { name: "Department" })).toBeFocused();
    await page.keyboard.press("Tab");
    await retype(page, "26");
    // Past the last editor, Tab wraps to the first.
    await page.keyboard.press("Tab");
    await expect(grid(page).getByRole("textbox", { name: "Name" })).toBeFocused();
    await retype(page, "Adam Fischer");
    await page.keyboard.press("Enter");

    await expect(editor(page)).toHaveCount(0);
    await expect(cell(page, 1, 1)).toHaveText("Adam Fischer");
    await expect(cell(page, 1, 4)).toHaveText("26");
    // Focus returns to the cell whose editor had it last.
    await expect(cell(page, 1, 1)).toBeFocused();
  });

  test("an invalid row stays open and says why", async ({ page }) => {
    await goTo(page, 0, 1);
    await page.keyboard.press("Enter");
    await page.keyboard.press("Tab");
    await retype(page, "nowhere");
    await page.keyboard.press("Enter");
    await expect(grid(page).getByRole("alert")).toHaveText("Not an email address");
    await expect(status(page)).toHaveText("Please correct the marked fields");

    const results = await new AxeBuilder({ page }).include(".dg-wrapper").analyze();
    expect(results.violations).toEqual([]);

    await page.keyboard.press("Escape");
    await expect(cell(page, 0, 2)).toHaveText("zoe.bauer@example.com");
  });
});

test.describe("dialog editing", () => {
  test.beforeEach(async ({ page }) => editMode(page, "dialog"));

  test("Enter opens a form, focus stays inside, Enter saves", async ({ page }) => {
    await goTo(page, 2, 1);
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Edit row" });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("textbox", { name: "Name" })).toBeFocused();

    // Name, Email, Department, Age, Save, Cancel, then round to Name again.
    for (let step = 0; step < 6; step++) await page.keyboard.press("Tab");
    await expect(dialog.getByRole("textbox", { name: "Name" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(dialog.getByRole("button", { name: "Cancel" })).toBeFocused();

    await dialog.getByRole("textbox", { name: "Age" }).focus();
    await retype(page, "33");
    await page.keyboard.press("Enter");
    await expect(dialog).toHaveCount(0);
    await expect(cell(page, 2, 4)).toHaveText("33");
    await expect(cell(page, 2, 1)).toBeFocused();
  });

  test("the open form has no axe violations", async ({ page }) => {
    await goTo(page, 0, 1);
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("textbox", { name: "Name" }).focus();
    await retype(page, " ");
    await page.keyboard.press("Enter");
    await expect(dialog.getByRole("alert").first()).toBeVisible();

    const results = await new AxeBuilder({ page }).include("[data-edit-dialog]").analyze();
    expect(results.violations).toEqual([]);

    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
    await expect(cell(page, 0, 1)).toBeFocused();
  });

  test("Add opens an empty form that checks every field", async ({ page }) => {
    await page.getByRole("button", { name: "Add" }).focus();
    await page.keyboard.press("Enter");
    const dialog = page.getByRole("dialog", { name: "New row" });
    await expect(dialog.getByRole("textbox", { name: "Name" })).toBeFocused();

    await page.keyboard.press("Enter");
    await expect(dialog.getByText("Enter a name")).toBeVisible();
    await expect(dialog.getByText("Please correct the marked fields")).toBeVisible();

    await page.keyboard.type("Petra Neu");
    await page.keyboard.press("Tab");
    await page.keyboard.type("petra@example.com");
    await page.keyboard.press("Enter");
    await expect(dialog).toHaveCount(0);
    await expect(count(page)).toHaveText("13 rows");
  });
});

test.describe("batch editing", () => {
  test.beforeEach(async ({ page }) => editMode(page, "batch"));

  test("changes are marked and counted until saved", async ({ page }) => {
    await goTo(page, 0, 4);
    await page.keyboard.press("Enter");
    await retype(page, "40");
    await page.keyboard.press("Enter");
    // Enter moved down; edit that row too.
    await page.keyboard.press("F2");
    await retype(page, "50");
    await page.keyboard.press("Enter");

    await expect(grid(page).locator("[data-changed='true']")).toHaveCount(2);
    await expect(status(page)).toHaveText("2 unsaved changes");

    await page.getByRole("button", { name: "Save changes" }).focus();
    await page.keyboard.press("Enter");
    await expect(grid(page).locator("[data-changed]")).toHaveCount(0);
    await expect(cell(page, 0, 4)).toHaveText("40");
    await expect(cell(page, 1, 4)).toHaveText("50");
  });

  test("Discard takes every change back", async ({ page }) => {
    await goTo(page, 0, 1);
    await page.keyboard.press("Enter");
    await retype(page, "Temporary");
    await page.keyboard.press("Enter");
    await expect(cell(page, 0, 1)).toHaveText("Temporary");

    await page.getByRole("button", { name: "Discard changes" }).focus();
    await page.keyboard.press("Enter");
    await expect(cell(page, 0, 1)).toHaveText("Zoe Bauer");
  });

  test("a deleted row is struck through until saved", async ({ page }) => {
    await goTo(page, 0, 1);
    await page.keyboard.press("Delete");
    await page.getByRole("alertdialog").getByRole("button", { name: "Delete" }).focus();
    await page.keyboard.press("Enter");
    await expect(bodyRows(page).nth(0)).toHaveAttribute("data-deleted", "true");
    await expect(count(page)).toHaveText("12 rows");

    await page.getByRole("button", { name: "Save changes" }).focus();
    await page.keyboard.press("Enter");
    await expect(count(page)).toHaveText("11 rows");
  });
});

test("Delete asks first, starting on Keep", async ({ page }) => {
  await editMode(page, "cell");
  await goTo(page, 0, 1);
  await page.keyboard.press("Delete");
  const confirm = page.getByRole("alertdialog", { name: "Delete this row?" });
  await expect(confirm.getByRole("button", { name: "Keep" })).toBeFocused();

  const results = await new AxeBuilder({ page }).include("[data-delete-confirm]").analyze();
  expect(results.violations).toEqual([]);

  await page.keyboard.press("Escape");
  await expect(confirm).toHaveCount(0);
  await expect(count(page)).toHaveText("12 rows");
  await expect(cell(page, 0, 1)).toBeFocused();

  await page.keyboard.press("Delete");
  await page.keyboard.press("Shift+Tab");
  await expect(confirm.getByRole("button", { name: "Delete" })).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(count(page)).toHaveText("11 rows");
  await expect(cell(page, 0, 1)).toHaveText("adam Fischer");
});

test("turning editing off makes the grid read-only again", async ({ page }) => {
  await editMode(page, "cell");
  await expect(page.locator("[aria-readonly]")).toHaveCount(0);
  await page.getByTestId("edit-off").check();
  await goTo(page, 0, 1);
  await page.keyboard.press("Enter");
  await expect(editor(page)).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Add" })).toHaveCount(0);
});

test("leaving batch mode drops the unsaved batch", async ({ page }) => {
  await editMode(page, "batch");
  await goTo(page, 0, 4);
  await page.keyboard.press("Enter");
  await retype(page, "60");
  await page.keyboard.press("Enter");
  await expect(cell(page, 0, 4)).toHaveText("60");

  await editMode(page, "cell");
  await expect(cell(page, 0, 4)).toHaveText("30");
  await expect(grid(page).locator("[data-changed]")).toHaveCount(0);
});

// The theme's error colours are backgrounds in the dark scheme; messages have
// to stay readable there too.
test.describe("dark theme", () => {
  test.use({ colorScheme: "dark" });

  test("messages and a failed save have no axe violations", async ({ page }) => {
    await page.getByTestId("toggle-fail-saves").check();
    await editMode(page, "cell");
    await goTo(page, 0, 1);
    await page.keyboard.press("Enter");
    await retype(page, "Nobody");
    await page.keyboard.press("Enter");
    await expect(status(page)).toHaveAttribute("data-state", "failed");
    await goTo(page, 0, 4);
    await page.keyboard.press("Enter");
    await retype(page, "old");
    await page.keyboard.press("Enter");
    await expect(grid(page).getByRole("alert")).toBeVisible();

    let results = await new AxeBuilder({ page }).include(".dg-wrapper").analyze();
    expect(results.violations).toEqual([]);

    await page.keyboard.press("Escape");
    await editMode(page, "dialog");
    await goTo(page, 1, 1);
    await page.keyboard.press("Enter");
    await page.keyboard.press("Tab");
    await retype(page, "nope");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog").getByRole("alert").first()).toBeVisible();
    results = await new AxeBuilder({ page }).include("[data-edit-dialog]").analyze();
    expect(results.violations).toEqual([]);
  });
});
