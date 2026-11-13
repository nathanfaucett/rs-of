import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [sveltekit()],
  test: {
    include: [
      "src/lib/common/state/webLoginClient.test.ts",
      "src/lib/common/state/setupClient.test.ts",
      "src/routes/(auth)/authorize/registration.test.ts",
    ],
    environment: "node",
  },
});
