import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
import auth from "./vitest.auth.config.ts";

export default defineConfig({
    ...auth,
    resolve: { ...auth.resolve, conditions: ["browser"] },
    test: {
        include: ["src/lib/common/components/maintenance.interaction.test.ts", "src/lib/common/components/joinSetup.interaction.test.ts", "src/lib/common/components/sourceApproval.interaction.test.ts", "src/lib/common/components/residency.interaction.test.ts", "src/lib/common/components/independentSetup.interaction.test.ts"],
        environment: fileURLToPath(
            new URL(
                "./src/lib/common/components/residency.environment.ts",
                import.meta.url,
            ),
        ),
    },
});
