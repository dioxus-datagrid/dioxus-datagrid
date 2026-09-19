import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";

// Phase 8: the filter menu, the value list and the filter bar's operators, on
// the playground's twelve employees. Ages: 30 25 30 41 41 28 52 34 45 23 38 31.

const count = (page: Page) => page.locator(".dg-count");
const menuButton = (page: Page, column: string) =>
  page.getByRole("button", { name: `Filter options for ${column}` });
const dialog = (page: Page) => page.getByRole("dialog");

test.beforeEach(async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await expect(count(page)).toHaveText("12 rows");
});

test("the filter bar compares numbers", async ({ page }) => {
  const age = page.getByRole("textbox", { name: "Filter Age" });

  await age.fill(">40");
  await expect(count(page)).toHaveText("4 rows");
  await age.fill("30");
  await expect(count(page)).toHaveText("2 rows");
  await age.fill("25..31");
  await expect(count(page)).toHaveText("5 rows");
});

test("a condition from the menu filters and the menu returns focus", async ({ page }) => {
  await menuButton(page, "Age").click();
  await expect(menuButton(page, "Age")).toHaveAttribute("aria-expanded", "true");
  // Focus moves into the menu, onto the first operator.
  await expect(dialog(page).getByRole("combobox", { name: "Operator" }).first()).toBeFocused();

  await dialog(page).getByRole("combobox", { name: "Operator" }).first().selectOption("between");
  await dialog(page).getByRole("textbox", { name: "Value" }).fill("30");
  await dialog(page).getByRole("textbox", { name: "Up to" }).fill("40");
  await dialog(page).getByRole("button", { name: "Apply" }).click();

  await expect(dialog(page)).toHaveCount(0);
  await expect(menuButton(page, "Age")).toBeFocused();
  await expect(count(page)).toHaveText("5 rows");
  await expect(page.getByRole("columnheader", { name: "Age" })).toHaveAttribute("data-filtered", "true");
});

test("two conditions combine with or", async ({ page }) => {
  await menuButton(page, "Age").click();
  const operators = dialog(page).getByRole("combobox", { name: "Operator" });
  await operators.first().selectOption("less");
  await dialog(page).getByRole("textbox", { name: "Value" }).first().fill("25");
  await dialog(page).getByRole("radio", { name: "or" }).check();
  await operators.nth(1).selectOption("greater");
  await dialog(page).getByRole("textbox", { name: "Value" }).nth(1).fill("44");
  await dialog(page).getByRole("button", { name: "Apply" }).click();

  // 23, 45 and 52.
  await expect(count(page)).toHaveText("3 rows");
});

test("the value list ticks departments and counts them", async ({ page }) => {
  await menuButton(page, "Department").click();
  await dialog(page).getByRole("button", { name: "Values" }).click();
  await expect(dialog(page).getByRole("checkbox", { name: /engineering \(3\)/ })).toBeChecked();

  await dialog(page).getByRole("checkbox", { name: "Select all" }).uncheck();
  await dialog(page).getByRole("checkbox", { name: /engineering/ }).check();
  await dialog(page).getByRole("checkbox", { name: /legal/ }).check();
  await dialog(page).getByRole("button", { name: "Apply" }).click();
  await expect(count(page)).toHaveText("4 rows");

  // Another column's filter narrows the list, but not the column's own.
  await page.getByRole("textbox", { name: "Filter Age" }).fill(">40");
  await expect(count(page)).toHaveText("1 row");
  await menuButton(page, "Department").click();
  await expect(dialog(page).getByRole("checkbox", { name: /legal \(1\)/ })).toBeChecked();
  await expect(dialog(page).getByRole("checkbox", { name: /finance \(1\)/ })).not.toBeChecked();
  await expect(dialog(page).getByRole("checkbox", { name: /engineering/ })).toHaveCount(0);
});

test("the value list can be searched", async ({ page }) => {
  await menuButton(page, "Department").click();
  await dialog(page).getByRole("button", { name: "Values" }).click();
  await dialog(page).getByRole("searchbox", { name: "Search values" }).fill("s");

  await expect(dialog(page).getByRole("listitem")).toHaveText([/research/, /sales/, /support/]);
});

test("Escape and a click outside close the menu without applying", async ({ page }) => {
  await menuButton(page, "Age").click();
  await dialog(page).getByRole("textbox", { name: "Value" }).fill("30");
  await page.keyboard.press("Escape");
  await expect(dialog(page)).toHaveCount(0);
  await expect(menuButton(page, "Age")).toBeFocused();
  await expect(count(page)).toHaveText("12 rows");

  await menuButton(page, "Age").click();
  await page.mouse.click(5, 5);
  await expect(dialog(page)).toHaveCount(0);
});

test("Clear removes the column's filters", async ({ page }) => {
  await page.getByRole("textbox", { name: "Filter Age" }).fill(">40");
  await expect(count(page)).toHaveText("4 rows");

  await menuButton(page, "Age").click();
  await dialog(page).getByRole("button", { name: "Clear" }).click();
  await expect(count(page)).toHaveText("12 rows");
  await expect(page.getByRole("textbox", { name: "Filter Age" })).toHaveValue("");
});

test("the menu speaks German", async ({ page }) => {
  await page.getByTestId("language-de").check();
  await page.getByRole("button", { name: "Filteroptionen für Alter" }).click();
  const operator = dialog(page).getByRole("combobox", { name: "Operator" }).first();
  await expect(operator.locator("option:checked")).toHaveText("gleich");
  await expect(dialog(page).getByRole("button", { name: "Anwenden" })).toBeVisible();
});

for (const mode of ["Condition", "Values"] as const) {
  test(`an open menu has no axe violations (${mode})`, async ({ page }) => {
    await menuButton(page, "Department").click();
    await dialog(page).getByRole("button", { name: mode }).click();
    await expect(dialog(page)).toBeVisible();

    const results = await new AxeBuilder({ page }).include(".dg-wrapper").analyze();
    expect(results.violations).toEqual([]);
  });
}

test("every menu stays inside a phone's screen", async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 740 });
  const buttons = page.locator("[data-filter-trigger]");
  const total = await buttons.count();
  expect(total).toBeGreaterThan(1);

  for (let index = 0; index < total; index++) {
    await buttons.nth(index).click();
    const panel = dialog(page);
    await expect(panel).toBeVisible();
    // The value list is the wider half; wait for it where there is one.
    const values = panel.getByRole("button", { name: "Values" });
    if (await values.count()) {
      await values.click();
      await expect(panel.getByRole("listitem").first()).toBeVisible();
    }
    await expect
      .poll(async () => {
        const box = await panel.boundingBox();
        return box !== null && box.x >= 0 && box.x + box.width <= 360;
      })
      .toBe(true);
    // A click outside closes it. Not Escape: WebKit leaves focus on the page
    // after a button click, so the key would not reach the panel.
    await page.mouse.click(2, 2);
    await expect(panel).toHaveCount(0);
  }
});
