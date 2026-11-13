import { sveltekit } from "@sveltejs/kit/vite";
import { playwright } from "@vitest/browser-playwright";
import { defineConfig } from "vitest/config";
import { appEnv } from "../../e2e/frontend-env.mjs";

Object.assign(process.env, appEnv);

export default defineConfig({
  plugins: [sveltekit()],
  server: { fs: { allow: ["../.."] } },
  test: {
    include: ["src/**/*.backend.test.ts"],

    testTimeout: 60000,
    browser: {
      api: { host: "127.0.0.1", port: 5173, strictPort: true },
      enabled: true,
      provider: playwright(),
      instances: [
        { browser: "chromium", globalSetup: ["../../e2e/backend.mjs"] },
      ],
      headless: true,
    },
  },
});
