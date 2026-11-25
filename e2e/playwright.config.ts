import { defineConfig, devices } from "@playwright/test";
import { appURL, passwordManagerURL, root } from "./backend.mjs";
import { appEnv, frontendEnv } from "./frontend-env.mjs";

const { CI } = process.env;

export default defineConfig({
  testDir: ".",
  testMatch: "*.e2e.ts",
  globalSetup: "./backend.mjs",
  timeout: 60000,
  expect: { timeout: 15000 },
  workers: 1,
  fullyParallel: false,
  forbidOnly: Boolean(CI),
  retries: 0,
  outputDir: `${root}.tmp/e2e-results`,
  reporter: [["list"], ["html", { outputFolder: `${root}.tmp/e2e-report`, open: "never" }]],
  use: { ...devices["Desktop Chrome"], trace: "retain-on-failure" },
  projects: [
    {
      name: "app",
      testMatch: ["app.e2e.ts", "owners.e2e.ts", "user-creation.e2e.ts"],
    },
    { name: "password-manager", testMatch: "password-manager*.e2e.ts" },
  ],
  webServer: [
    {
      command: `pnpm --filter @ofsdk/app exec vite dev --mode test --host 127.0.0.1 --port ${new URL(appURL).port} --strictPort`,
      cwd: root,
      url: appURL,
      env: appEnv,
      reuseExistingServer: false,
      timeout: 120000,
    },
    {
      command: `pnpm --filter @ofsdk/password-manager exec vite dev --mode test --host 127.0.0.1 --port ${new URL(passwordManagerURL).port} --strictPort`,
      cwd: root,
      url: passwordManagerURL,
      env: frontendEnv,
      reuseExistingServer: false,
      timeout: 120000,
    },
  ],
});
