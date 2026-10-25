import { redirect } from "@sveltejs/kit";
import { isTauri } from "@tauri-apps/api/core";
import { getIdpApiUrl } from "$lib/common/state/idpClient.svelte";
import { observeOnboarding } from "$lib/common/state/onboarding";

export async function load() {
    if (!isTauri() && !getIdpApiUrl()) throw redirect(302, "/config");
    const observation = await observeOnboarding();
    return { observation };
}
