import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

function parseInteger(value: string | undefined, defaultValue: number): number {
  return value === undefined ? defaultValue : Number.parseInt(value, 10);
}

export default defineConfig({
  root: ".",
  publicDir: false,
  plugins: [tailwindcss(), svelte()],
  build: {
    outDir: "dist",
    emptyOutDir: true,
    modulePreload: { polyfill: false },
  },
  server: {
    port: parseInteger(process.env.PORT, 8080),
    host: "127.0.0.1",
  },
  ssr: {
    noExternal: ["@ofsdk/kv"],
  },
});
