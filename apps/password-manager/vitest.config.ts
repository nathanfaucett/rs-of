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
            "@aicacia/storage-client": source(
                "../../packages/storage-client/src/index.ts",
            ),
            "@aicacia/storage-db-collection": source(
                "../../packages/storage-db-collection/src/index.ts",
            ),
        },
    },
    test: { environment: "jsdom", include: ["src/**/*.test.ts"] },
});
