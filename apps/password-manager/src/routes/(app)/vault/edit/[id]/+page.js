import { error } from "@sveltejs/kit";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";

/** @type {import('./$types').PageLoad} */
export function load({ params }) {
    const vaultData = vaultStateModule.vaultData;
    if (!vaultData) {
        error(404, "Vault not available");
    }

    // We need to get the secret from the vault collection
    // The workflow is created in the component, so we'll pass the ID and let the component fetch it
    // For now, return the ID and let the component handle loading
    return {
        id: params.id
    };
}
