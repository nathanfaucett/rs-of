import { error, redirect } from "@sveltejs/kit";
import { isTauri } from "@tauri-apps/api/core";
import { getIdpApiUrl } from "$lib/common/state/idpClient.svelte";
import { ensureOidcClientId } from "$lib/common/state/oidc.svelte";
import { getSetupStage } from "$lib/common/state/setupClient.svelte";

export async function load() {
  if (isTauri()) {
    if ((await getSetupStage()) !== "ready") throw redirect(302, "/setup");
    return;
  }
  if (!getIdpApiUrl()) throw redirect(302, "/config");
  const stage = await getSetupStage().catch(() =>
    error(
      503,
      "Unable to determine setup readiness. Retry when the configured services are reachable.",
    ),
  );
  if (stage !== "ready") throw redirect(302, "/setup/installation");
  await ensureOidcClientId().catch(() =>
    error(
      503,
      "Verified public login Client is unavailable. Retry when the configured services are reachable.",
    ),
  );
}
