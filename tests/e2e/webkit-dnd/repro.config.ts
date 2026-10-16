import { defineConfig, devices } from "@playwright/test";

// Named `*.repro.ts` so the suite next door does not pick it up.
export default defineConfig({
  testDir: ".",
  testMatch: "**/*.repro.ts",
  reporter: "list",
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "firefox", use: { ...devices["Desktop Firefox"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
});
