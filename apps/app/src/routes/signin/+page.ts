import { redirect } from "@sveltejs/kit";
import { isTauri } from "@tauri-apps/api/core";
import { ensureTauriIdpApiUrl, getIdpApiUrl } from "$lib/common/state/idpClient.svelte";
import { nodeSetupRoute } from "$lib/common/state/nodeSetup";
import { ensureOidcClientId } from "$lib/common/state/oidc.svelte";
import { observeOnboarding } from "$lib/common/state/onboarding";

export async function load() {
  if (!isTauri() && !getIdpApiUrl()) throw redirect(302, "/config");
  const route = await nodeSetupRoute();
  if (route && route !== "/signin") redirect(302, route);
  const observation = await observeOnboarding();
  if (observation.state !== "ready") throw redirect(302, "/setup/installation");
  if (isTauri()) await ensureTauriIdpApiUrl();
  await ensureOidcClientId(observation);
}
