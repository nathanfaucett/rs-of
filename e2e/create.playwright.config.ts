import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "create-installation.e2e.ts",
  timeout: 180_000,
  expect: { timeout: 20_000 },
  outputDir: "../.tmp/create-e2e-results",
  use: { ...devices["Desktop Chrome"] },
});
