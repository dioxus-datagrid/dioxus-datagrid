import { defineConfig, devices } from "@playwright/test";
import path from "node:path";

// Mirrors the setup in DioxusLabs/components: one web build of the app under
// test, served on 8080, exercised in all three engines. E2E_PORT moves it off
// 8080 when another `dx serve` holds that port, such as a mobile build.
const port = Number(process.env.E2E_PORT ?? 8080);
// examples/server, for the server-side data tests.
const serverPort = Number(process.env.E2E_SERVER_PORT ?? 8093);

export default defineConfig({
  testDir: ".",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: `http://localhost:${port}`,
    trace: "on-first-retry",
  },
  // The first run builds the playground to WASM, which takes a while.
  timeout: 60_000,
  expect: { timeout: 10_000 },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "firefox", use: { ...devices["Desktop Firefox"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
  webServer: [
    {
      cwd: path.join(__dirname, "../../playground"),
      command: `dx run --web --release --port ${port}`,
      port,
      timeout: 30 * 60 * 1000,
      // Locally, reuse a `dx serve` that is already running.
      reuseExistingServer: !process.env.CI,
      stdout: "pipe",
    },
    {
      cwd: path.join(__dirname, "../../examples/server"),
      command: `dx run --web --release --port ${serverPort}`,
      port: serverPort,
      timeout: 30 * 60 * 1000,
      reuseExistingServer: !process.env.CI,
      stdout: "pipe",
    },
  ],
});
