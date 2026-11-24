import { defineConfig, devices } from "@playwright/test";

const { JOIN_TEST_BROWSER_HOME: browserHome } = process.env;

export default defineConfig({
  testDir: ".",
  testMatch: "join.e2e.ts",
  outputDir: "../.tmp/join-e2e-results",
  timeout: 90_000,
  use: {
    ...devices["Desktop Chrome"],
    ...(browserHome ? { launchOptions: { env: { ...process.env, HOME: browserHome } } } : {}),
  },
  workers: 1,
});
