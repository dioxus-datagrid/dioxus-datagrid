// Regenerates docs/screenshot.png from the running playground:
//   node screenshot.mjs   (with `dx serve` running on :8080)
import { chromium } from "@playwright/test";

const browser = await chromium.launch();
const page = await (await browser.newContext({
  viewport: { width: 1000, height: 560 },
  deviceScaleFactor: 2,
  colorScheme: "light",
})).newPage();

await page.goto("http://localhost:8080");
await page.locator(".dg-body [role='row']").first().waitFor();

// Show a sorted column and a selection, so the image demonstrates the features.
await page.getByRole("columnheader", { name: "Department", exact: true }).click();
const names = page.locator(".dg-body [role='row'] [role='gridcell'][aria-colindex='1']");
await names.nth(1).click();
await names.nth(2).click();
await page.mouse.move(0, 0);

await page.locator("main").screenshot({ path: "../../docs/screenshot.png" });
await browser.close();
console.log("wrote docs/screenshot.png");
