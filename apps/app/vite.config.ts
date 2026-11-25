import { paraglideVitePlugin } from "@inlang/paraglide-js";
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
  const portless = PORTLESS_URL?.match(/^https:\/\/((?:[a-z0-9-]+\.)*)of\.localhost(:[0-9]+)?$/);
  if (command === "serve" && !isPreview && portless && PORTLESS_URL) {
    const unified = `https://${portless[1] ?? ""}unified.localhost${portless[2] ?? ""}`;
    const env: Record<string, string | undefined> = loadEnv(mode, process.cwd(), "PUBLIC_");
    const idp = `${unified}/idp`;
    const { PUBLIC_IDP_BASE_URL: configuredIdp } = env;
    if (configuredIdp === undefined || configuredIdp.replace(/\/$/, "") === idp) {
      const defaults = {
        PUBLIC_URL: PORTLESS_URL,
        PUBLIC_IDP_BASE_URL: idp,
        ...(mode === "seeded"
          ? {
              PUBLIC_OIDC_CLIENT_ID: "local-login",
              PUBLIC_REQUIRE_SEEDED_FIXTURE_HEADER: "1",
              PUBLIC_OIDC_APPLICATION_URI: `${PORTLESS_URL}/local-login`,
            }
          : { PUBLIC_UNIFIED_ORIGIN: unified }),
        PUBLIC_MANAGEMENT_BASE_URL: `${unified}/management`,
        PUBLIC_DEVICE_BASE_URL: `${unified}/device`,
        PUBLIC_STORAGE_BASE_URL: `${unified}/storage`,
      };
      for (const [key, value] of Object.entries(defaults)) {
        process.env[key] ??= env[key] ?? value;
      }
    }
  }

  return {
    plugins: [
      tailwindcss(),
      sveltekit(),
      devtoolsJson(),
      paraglideVitePlugin({
        project: "./project.inlang",
        outdir: "./src/lib/paraglide",
        strategy: ["localStorage", "baseLocale"],
      }),
    ],
    server: {
      port: parseInteger(PORT, 5173),
      strictPort: true,
    },
    preview: {
      port: parseInteger(PORT, 5173),
      strictPort: true,
    },
  };
});
