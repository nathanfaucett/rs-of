import { defineConfig } from "vitest/config";
import config from "./vitest.config.ts";
import { frontendEnv } from "../../e2e/frontend-env.mjs";

Object.assign(process.env, frontendEnv);

export default defineConfig({
  ...config,
  test: {
    ...config.test,
    include: ["src/test/*.backend.test.ts"],
    exclude: [],
    globalSetup: ["../../e2e/backend.mjs"],
    browser: {
      ...config.test?.browser,
      api: { host: "127.0.0.1", port: 5174, strictPort: true },
    },
    setupFiles: [],
    testTimeout: 60000,
    fileParallelism: false,
  },
});
