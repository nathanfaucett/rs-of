import { redirect } from "@sveltejs/kit";
import { isTauri } from "@tauri-apps/api/core";
import { ensureTauriIdpApiUrl, getIdpApiUrl } from "$lib/common/state/idpClient.svelte";
import { nodeSetupStatus } from "$lib/common/state/nodeSetup";
import { ensureOidcClientId } from "$lib/common/state/oidc.svelte";
import { observeOnboarding } from "$lib/common/state/onboarding";

export async function load() {
  if (!isTauri() && !getIdpApiUrl()) throw redirect(302, "/config");
  const node = await nodeSetupStatus();
  if (node && node.route !== "/signin") redirect(302, node.route);
  const observation = await observeOnboarding(node);
  if (observation.state !== "ready") throw redirect(302, "/setup/installation");
  if (isTauri()) await ensureTauriIdpApiUrl();
  await ensureOidcClientId(observation);
}
