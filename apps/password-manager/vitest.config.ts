import { fileURLToPath } from "node:url";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vitest/config";

const source = (path: string) => fileURLToPath(new URL(path, import.meta.url));
export default defineConfig({
    plugins: [sveltekit()],
    resolve: {
        conditions: ["browser"],
        alias: {
            $lib: source("./src/lib"),
            "@of/sdk": source("../../packages/sdk/src/index.ts"),
            "@aicacia/oidc-client": source(
                "../../packages/oidc-client/src/index.ts",
            ),
            "@aicacia/storage-client": source(
                "../../packages/storage-client/src/index.ts",
            ),
        },
    },
    test: { environment: "jsdom", include: ["src/**/*.test.ts"] },
});
