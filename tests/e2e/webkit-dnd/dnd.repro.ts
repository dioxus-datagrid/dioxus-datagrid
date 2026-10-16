import { expect, type Page, test } from "@playwright/test";

// Does Playwright drive native HTML5 drag and drop, per engine?
//
// One page, no server, no framework: a draggable source, a drop target, and a
// log of every drag event the page received.

const PAGE = `
<!doctype html>
<meta charset="utf-8">
<style>
  div { font: 16px sans-serif; padding: 24px; margin: 16px; width: 200px; }
  #source { background: #cde; border: 2px solid #468; }
  #target { background: #edc; border: 2px dashed #864; }
</style>
<div id="source" draggable="true">drag me</div>
<div id="target">drop here</div>
<pre id="log"></pre>
<script>
  window.__log = [];
  const note = (text) => {
    window.__log.push(text);
    document.getElementById("log").textContent = window.__log.join(", ");
  };
  const source = document.getElementById("source");
  const target = document.getElementById("target");
  source.addEventListener("dragstart", (event) => {
    event.dataTransfer.setData("text/plain", "payload");
    note("dragstart");
  });
  source.addEventListener("dragend", () => note("dragend"));
  // Only the first dragover is logged; it repeats while the pointer moves.
  target.addEventListener("dragover", (event) => {
    event.preventDefault();
    if (!window.__log.includes("dragover")) note("dragover");
  });
  target.addEventListener("drop", (event) => {
    event.preventDefault();
    note("drop:" + event.dataTransfer.getData("text/plain"));
  });
</script>
`;

const log = (page: Page) => page.evaluate(() => (window as unknown as { __log: string[] }).__log);

test("dragTo", async ({ page }) => {
  await page.setContent(PAGE);
  await page.locator("#source").dragTo(page.locator("#target"));
  const events = await log(page);
  console.log("dragTo events:", JSON.stringify(events));
  expect(events).toContain("drop:payload");
});

test("the mouse, step by step", async ({ page }) => {
  await page.setContent(PAGE);
  const start = await page.locator("#source").boundingBox();
  const end = await page.locator("#target").boundingBox();
  if (!start || !end) throw new Error("no boxes");
  await page.mouse.move(start.x + start.width / 2, start.y + start.height / 2);
  await page.mouse.down();
  // Several moves: a browser starts a drag only after the pointer has travelled.
  await page.mouse.move(start.x + start.width / 2, start.y + start.height / 2 + 8);
  await page.mouse.move(end.x + end.width / 2, end.y + end.height / 2, { steps: 12 });
  await page.mouse.up();
  const events = await log(page);
  console.log("mouse events:", JSON.stringify(events));
  expect(events).toContain("drop:payload");
});
