import { fileURLToPath } from "node:url";
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

const { VITE_BASE_PATH } = process.env;

/** @type {import('@sveltejs/kit').Config} */
const config = {
  // Consult https://svelte.dev/docs/kit/integrations
  // for more information about preprocessors
  preprocess: vitePreprocess(),
  kit: {
    serviceWorker: { register: false },
    // adapter-auto only supports some environments, see https://svelte.dev/docs/kit/adapter-auto for a list.
    // If your environment is not supported, or you settled on a specific environment, switch out the adapter.
    // See https://svelte.dev/docs/kit/adapters for more information about adapters.
    adapter: adapter({
      fallback: "index.html",
    }),
    paths: {
      base: VITE_BASE_PATH,
      relative: false,
    },
    alias: {
      $fixtures: fileURLToPath(new URL("./src/test/fixtures", import.meta.url)),
      "@ofsdk/storage-client": fileURLToPath(
        new URL("../../packages/storage-client/src/index.ts", import.meta.url)
      ),
    },
  },
};

export default config;
