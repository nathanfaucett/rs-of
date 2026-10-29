import { defineConfig } from "vitest/config";
import authConfig from "../../../../vitest.auth.config.ts";

export default defineConfig({
    ...authConfig,
    test: {
        ...authConfig.test,
        include: ["src/routes/**/authorize/*.test.ts"],
    },
});
