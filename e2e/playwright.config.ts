import { defineConfig, devices } from "@playwright/test";
import { appURL, passwordManagerURL, root } from "./backend.mjs";
import { frontendEnv as env } from "./frontend-env.mjs";

export default defineConfig({
  testDir: ".",
  testMatch: "*.e2e.ts",
  globalSetup: "./backend.mjs",
  timeout: 60000,
  expect: { timeout: 15000 },
  workers: 1,
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: 0,
  outputDir: "../.scratch/e2e-results",
  reporter: [["list"], ["html", { outputFolder: `${root}.scratch/e2e-report`, open: "never" }]],
  use: { ...devices["Desktop Chrome"], trace: "retain-on-failure" },
  projects: [
    {
      name: "app",
      testMatch: ["app.e2e.ts", "owners.e2e.ts"],
    },
    { name: "password-manager", testMatch: "password-manager.e2e.ts" },
  ],
  webServer: [
    {
      command:
        "pnpm --filter @ofsdk/app exec vite dev --mode test --host 127.0.0.1 --port 5173 --strictPort",
      cwd: root,
      url: appURL,
      env,
      reuseExistingServer: false,
      timeout: 120000,
    },
    {
      command:
        "pnpm --filter @ofsdk/password-manager exec vite dev --mode test --host 127.0.0.1 --port 5174 --strictPort",
      cwd: root,
      url: passwordManagerURL,
      env,
      reuseExistingServer: false,
      timeout: 120000,
    },
  ],
});
