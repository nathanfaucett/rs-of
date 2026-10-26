import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
export default defineConfig({
    resolve: {
        alias: {
            "@aicacia/oidc-client": fileURLToPath(
                new URL("../oidc-client/src/index.ts", import.meta.url),
            ),
            "@aicacia/storage-client": fileURLToPath(
                new URL("../storage-client/src/index.ts", import.meta.url),
            ),
        },
    },
});
