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
  if (
    command === "serve" &&
    !isPreview &&
    PORTLESS_URL === "https://password-manager.localhost:1355"
  ) {
    const localDefaults = {
      PUBLIC_IDP_BASE_URL: "https://unified.localhost:1355/idp",
      PUBLIC_OIDC_APPLICATION_URI: "https://of.localhost:1355/password-manager",
      PUBLIC_OIDC_SCOPES: "openid profile email storage",
      PUBLIC_STORAGE_URL: "https://unified.localhost:1355/storage",
    };
    const env = loadEnv(mode, fileURLToPath(new URL(".", import.meta.url)), "PUBLIC_");
    for (const [key, value] of Object.entries(localDefaults)) {
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
