import { expect, type Page, test } from "@playwright/test";
import { readFileSync } from "node:fs";

// Phase 13: the rows leave the grid as a file. The playground's export add-on
// downloads a CSV through a Blob and a clicked link, which is `document::eval`
// because Dioxus has no API for saving a file.

const grid = (page: Page) => page.getByRole("grid");

async function open(page: Page) {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.getByTestId("toggle-export").check();
  await expect(page.getByRole("button", { name: "Export as CSV" })).toBeVisible();
}

/** Clicks the export button and reads the file the browser was handed. */
async function downloadCsv(page: Page) {
  const [download] = await Promise.all([
    page.waitForEvent("download"),
    page.getByRole("button", { name: /Export as CSV|Als CSV exportieren/ }).click(),
  ]);
  const path = await download.path();
  return { name: download.suggestedFilename(), text: readFileSync(path, "utf-8") };
}

const lines = (text: string) => text.split("\r\n").filter((line) => line !== "");

test("downloads a CSV with the columns the grid shows", async ({ page }) => {
  await open(page);
  const { name, text } = await downloadCsv(page);

  expect(name).toBe("employees.csv");
  // Salary and Since start hidden, so they are not in the file.
  expect(lines(text)[0]).toBe("Name,Email,Department,Age");
  expect(text.startsWith("﻿")).toBe(false);
});

test("holds every row, not the page that is shown", async ({ page }) => {
  await open(page);
  // The playground pages by five.
  await expect(page.getByRole("button", { name: "Next page" })).toBeVisible();
  const shown = await page.locator(".dg-body [role='row']").count();
  expect(shown).toBe(5);

  const { text } = await downloadCsv(page);
  expect(lines(text).length - 1).toBeGreaterThan(shown);
});

test("holds the rows as they are filtered and sorted", async ({ page }) => {
  await open(page);
  await page.getByPlaceholder("Search all columns").fill("research");
  await page.getByRole("columnheader", { name: "Name", exact: true }).click();

  const { text } = await downloadCsv(page);
  const rows = lines(text).slice(1);
  expect(rows.length).toBeGreaterThan(0);
  expect(rows.every((row) => row.includes("research"))).toBe(true);

  const names = rows.map((row) => row.split(",")[0]);
  expect(names).toEqual([...names].sort((a, b) => a.localeCompare(b, "en", { sensitivity: "base" })));
});

test("leaves out a column the user hid", async ({ page }) => {
  await open(page);
  await page.locator(".dg-columns").locator("summary").click();
  await page.locator(".dg-columns").getByRole("checkbox", { name: "Email" }).uncheck();

  const { text } = await downloadCsv(page);
  expect(lines(text)[0]).toBe("Name,Department,Age");
});

test("says what it is in the language of the grid", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.getByTestId("toggle-export").check();
  await page.getByRole("radio", { name: "Deutsch" }).check();

  await expect(page.getByRole("group", { name: "Export" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Als CSV exportieren" })).toBeVisible();
  const { text } = await downloadCsv(page);
  // The headings are the application's, in the language it set.
  expect(lines(text)[0]).toBe("Name,E-Mail,Abteilung,Alter");
});

test("the grid is still usable after a download", async ({ page }) => {
  await open(page);
  await downloadCsv(page);
  await expect(grid(page)).toBeVisible();
  await page.getByRole("button", { name: "Next page" }).click();
  await expect(page.getByText(/Page 2 of/)).toBeVisible();
});
