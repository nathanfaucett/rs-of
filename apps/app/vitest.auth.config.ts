import { fileURLToPath } from "node:url";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [sveltekit()],
  resolve: {
    alias: {
      "@ofsdk/native-fetch": fileURLToPath(new URL("../../packages/native-fetch/src/index.ts", import.meta.url)),
      "@ofsdk/oidc-client": fileURLToPath(
        new URL("../../packages/oidc-client/src/index.ts", import.meta.url),
      ),
      "@ofsdk/idp-client": fileURLToPath(
        new URL("../../packages/idp-client/src/index.ts", import.meta.url),
      ),
      "@ofsdk/device-client": fileURLToPath(new URL("../../packages/device-client/src/index.ts", import.meta.url)),
            "@ofsdk/management-client": fileURLToPath(
        new URL(
          "../../packages/management-client/src/index.ts",
          import.meta.url,
        ),
      ),
      "@ofsdk/storage-client": fileURLToPath(
        new URL("../../packages/storage-client/src/index.ts", import.meta.url),
      ),
      "@ofsdk/svelte-headless": fileURLToPath(
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
