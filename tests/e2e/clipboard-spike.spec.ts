import { expect, type Page, test } from "@playwright/test";

// Phase 12 spike scaffolding, not a product test: it drives the clipboard
// probes in the playground and prints what each engine allows. Nothing here
// asserts a verdict — the verdicts are what we are trying to find out, and they
// are recorded in docs/VERIFICATION.md. Goes away with the spike.
//
//   SPIKE=1 npx playwright test clipboard-spike --project=webkit --reporter=list
//
// Skipped unless SPIKE=1: a test that asserts no verdict cannot pass or fail
// meaningfully, so it has no business deciding a CI run — and a probe an engine
// never answers only runs the suite into its timeouts.

test.skip(!process.env.SPIKE, "spike scaffolding; run it with SPIKE=1");

const probe = (page: Page, id: string) => page.getByTestId(`spike-${id}-result`);

async function open(page: Page) {
  await page.goto("/");
  await page.evaluate(() => localStorage.clear());
  await page.reload();
  await page.getByTestId("toggle-clipboard-spike").check();
  await expect(page.getByTestId("clipboard-spike")).toBeVisible();
}

test("what the clipboard allows", async ({ page }, testInfo) => {
  await open(page);
  const found: Record<string, string> = {};

  // The listener the eval installed reports itself before anything is pasted.
  found["paste listener"] = (await probe(page, "paste").textContent()) ?? "";

  for (const id of ["write-api", "write-exec", "read-api"]) {
    await page.getByTestId(`spike-${id}`).click();
    await expect
      .poll(async () => (await probe(page, id).textContent()) ?? "")
      .not.toContain("pending");
    found[id] = (await probe(page, id).textContent()) ?? "";
  }

  // A real Ctrl+V cannot be synthesized — the browser does not hand automation
  // the OS clipboard. A dispatched ClipboardEvent carries its own data, which
  // is what a paste test will have to do in Phase 12 as well.
  await page.evaluate(() => {
    const data = new DataTransfer();
    data.setData("text/plain", "a\tb\nc\td");
    document.dispatchEvent(
      new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true }),
    );
  });
  await expect.poll(async () => (await probe(page, "paste").textContent()) ?? "").toContain("pasted");
  found["paste event"] = (await probe(page, "paste").textContent()) ?? "";

  const report = Object.entries(found)
    .map(([name, verdict]) => `  ${name.padEnd(14)} ${verdict}`)
    .join("\n");
  console.log(`\n[${testInfo.project.name}]\n${report}\n`);
});
