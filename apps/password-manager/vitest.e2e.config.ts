import { defineConfig } from "vitest/config";
import config from "./vitest.config.ts";
import { passwordManagerURL } from "../../e2e/backend.mjs";
import { frontendEnv } from "../../e2e/frontend-env.mjs";

Object.assign(process.env, frontendEnv);

export default defineConfig({
  ...config,
  test: {
    ...config.test,
    include: ["src/test/*.backend.test.ts"],
    exclude: [],

    browser: {
      ...config.test?.browser,
      instances: [{ browser: "chromium", globalSetup: ["../../e2e/backend.mjs"] }],
      api: {
        host: "127.0.0.1",
        port: Number(new URL(passwordManagerURL).port),
        strictPort: true,
      },
    },
    setupFiles: [],
    testTimeout: 60000,
    fileParallelism: false,
  },
});
