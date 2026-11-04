import devtoolsJson from "vite-plugin-devtools-json";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vitest/config";
import { sveltekit } from "@sveltejs/kit/vite";

function parseInteger(value?: string, defaultValue?: number): number {
  try {
    return Number.parseInt(value, 10);
  } catch {
    return defaultValue;
  }
}

export default defineConfig({
  plugins: [tailwindcss(), sveltekit(), devtoolsJson()],
  server: {
    port: parseInteger(process.env.PORT, 5173),
    strictPort: true,
  },
  test: {
    expect: {
      requireAssertions: true,
    },
    projects: [
      {
        extends: "./vite.config.ts",
        test: {
          name: "server",
          environment: "node",
          include: ["src/**/*.{test,spec}.{js,ts}"],
          exclude: ["src/**/*.svelte.{test,spec}.{js,ts}"],
        },
      },
    ],
  },
});
