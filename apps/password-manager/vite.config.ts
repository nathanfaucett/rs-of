import { fileURLToPath } from "node:url";

import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig, loadEnv } from "vite";
import devtoolsJson from "vite-plugin-devtools-json";

const { PORT, PORTLESS_URL } = process.env;

function parseInteger(value: string | undefined, defaultValue: number): number {
  const parsed = Number.parseInt(value ?? "", 10);
  return Number.isNaN(parsed) ? defaultValue : parsed;
}

export default defineConfig(({ command, mode, isPreview }) => {
  const portless = PORTLESS_URL?.match(
    /^https:\/\/((?:[a-z0-9-]+\.)*)password-manager\.localhost(:[0-9]+)?$/
  );
  if (command === "serve" && !isPreview && portless) {
    const env: Record<string, string | undefined> = loadEnv(
      mode,
      fileURLToPath(new URL(".", import.meta.url)),
      "PUBLIC_"
    );
    const suffix = `${portless[2] ?? ""}`;
    const prefix = portless[1] ?? "";
    const unified = `https://${prefix}unified.localhost${suffix}`;
    const defaults =
      mode === "seeded"
        ? {
            PUBLIC_OIDC_CLIENT_ID: "password-manager",
            PUBLIC_OIDC_APPLICATION_URI: `https://${prefix}of.localhost${suffix}/password-manager`,
            PUBLIC_IDP_BASE_URL: `${unified}/idp`,
            PUBLIC_STORAGE_BASE_URL: `${unified}/storage`,
          }
        : {
            PUBLIC_UNIFIED_ORIGIN: unified,
            PUBLIC_NODE_SETUP_URL: `https://${prefix}of.localhost${suffix}/setup`,
          };
    for (const [key, value] of Object.entries(defaults)) {
      process.env[key] ??= env[key] ?? value;
    }
  }

  return {
    plugins: [tailwindcss(), sveltekit(), devtoolsJson()],
    ssr: {
      external: ["lucide-svelte"],
    },
    build: {
      rolldownOptions: {
        external: ["lucide-svelte"],
      },
    },
    server: {
      port: parseInteger(PORT, 5173),
      strictPort: true,
      fs: {
        allow: [
          fileURLToPath(new URL(".", import.meta.url)),
          fileURLToPath(new URL("../../", import.meta.url)),
          fileURLToPath(new URL(".", import.meta.resolve("@ofsdk/sql/package.json"))),
        ],
      },
    },
    preview: {
      port: parseInteger(PORT, 5173),
      strictPort: true,
    },
  };
});
