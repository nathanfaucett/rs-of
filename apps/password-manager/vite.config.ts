import { fileURLToPath } from "node:url";

import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import devtoolsJson from "vite-plugin-devtools-json";

const { PORT } = process.env;

function parseInteger(value: string | undefined, defaultValue: number): number {
  const parsed = Number.parseInt(value ?? "", 10);
  return Number.isNaN(parsed) ? defaultValue : parsed;
}

export default defineConfig({
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
});
