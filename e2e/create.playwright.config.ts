import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "create-installation.e2e.ts",
  timeout: 240_000,
  expect: { timeout: 20_000 },
  outputDir: "../.tmp/create-e2e-results",
  use: { ...devices["Desktop Chrome"], actionTimeout: 20_000, navigationTimeout: 20_000 },
});
