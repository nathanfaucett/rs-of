import { fileURLToPath } from "node:url";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [sveltekit()],
  resolve: {
    alias: {
      "@aicacia/oidc-client": fileURLToPath(
        new URL("../../packages/oidc-client/src/index.ts", import.meta.url),
      ),
      "@aicacia/idp-client/independent": fileURLToPath(new URL("../../packages/idp-client/src/independent/index.ts", import.meta.url)),
      "@aicacia/idp-client/setup": fileURLToPath(
        new URL(
          "../../packages/idp-client/src/setup/index.ts",
          import.meta.url,
        ),
      ),
      "@aicacia/idp-client": fileURLToPath(
        new URL("../../packages/idp-client/src/index.ts", import.meta.url),
      ),
      "@aicacia/management-client/owner": fileURLToPath(new URL("../../packages/management-client/src/owner/index.ts", import.meta.url)),
      "@aicacia/management-client/node-join": fileURLToPath(new URL("../../packages/management-client/src/node-join/index.ts", import.meta.url)),
      "@aicacia/management-client": fileURLToPath(
        new URL(
          "../../packages/management-client/src/index.ts",
          import.meta.url,
        ),
      ),
      "@aicacia/storage-client": fileURLToPath(
        new URL("../../packages/storage-client/src/index.ts", import.meta.url),
      ),
      "@aicacia/svelte-headless": fileURLToPath(
        new URL(
          "../../packages/svelte-headless/src/lib/state/storage.svelte.ts",
          import.meta.url,
        ),
      ),
    },
  },
  test: {
    include: [
      "src/lib/common/state/setupClient.test.ts",
      "src/lib/common/state/independentSetup.test.ts",
      "src/lib/common/state/localhostBaseUrl.test.ts",
      "src/lib/common/state/ownerClients.test.ts",
      "src/routes/(auth)/layout.test.ts",
      "src/routes/setup/installation/page.test.ts",
      "../password-manager/src/lib/common/storageSocket.test.ts",
    ],
  },
});
