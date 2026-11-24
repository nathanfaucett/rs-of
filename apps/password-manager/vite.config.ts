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
    /^https:\/\/([a-z0-9-]+\.)?password-manager\.localhost(:[0-9]+)?$/
  );
  if (command === "serve" && !isPreview && portless) {
    const env: Record<string, string | undefined> = loadEnv(
      mode,
      fileURLToPath(new URL(".", import.meta.url)),
      "PUBLIC_"
    );
    const key = "PUBLIC_UNIFIED_ORIGIN";
    process.env[key] ??=
      env[key] ?? `https://${portless[1] ?? ""}unified.localhost${portless[2] ?? ""}`;
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
