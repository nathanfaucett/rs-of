import { fileURLToPath } from "node:url";
import { sveltekit } from "@sveltejs/kit/vite";
import { playwright } from "@vitest/browser-playwright";
import { defineConfig } from "vitest/config";

const source = (path: string) => fileURLToPath(new URL(path, import.meta.url));
export default defineConfig({
    cacheDir: ".svelte-kit/vitest-cache",
    plugins: [sveltekit()],
    server: {
        fs: {
            allow: [
                source("../../"),
                fileURLToPath(
                    new URL(".", import.meta.resolve("@ofsdk/sql/package.json")),
                ),
            ],
        },
    },
    resolve: {
        conditions: ["browser"],
        alias: {
            $lib: source("./src/lib"),
        },
    },
    test: {
        environment: "jsdom",
        include: ["src/**/*.test.ts"],
        browser: {
            enabled: true,
            provider: playwright(),
            instances: [{ browser: "chromium" }],
            headless: true,
        },
    },
});
