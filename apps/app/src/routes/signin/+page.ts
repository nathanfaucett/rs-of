import { redirect } from "@sveltejs/kit";
import { isTauri } from "@tauri-apps/api/core";
import {
    getIdpApiUrl,
    ensureTauriIdpApiUrl,
} from "$lib/common/state/idpClient.svelte";
import { ensureOidcClientId } from "$lib/common/state/oidc.svelte";
import { observeOnboarding } from "$lib/common/state/onboarding";

export async function load() {
    if (!isTauri() && !getIdpApiUrl()) throw redirect(302, "/config");
    const observation = await observeOnboarding();
    if (observation.state !== "ready")
        throw redirect(302, "/setup/installation");
    if (observation.action === "device_setup")
        throw redirect(302, "/setup/device");
    if (isTauri()) await ensureTauriIdpApiUrl();
    await ensureOidcClientId(observation);
}
