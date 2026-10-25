<script lang="ts">
import { goto, invalidateAll } from "$app/navigation";
import { resolve } from "$app/paths";
import Browser from "$lib/collections/secrets/Browser.svelte";
import { getIdpApiUrl } from "$lib/common/state/idpClient.svelte";
import { getOidcClient } from "$lib/common/state/oidc.svelte";
import type { PageProps } from "./$types";

let { data }: PageProps = $props();
let validating = $state(false);

async function signOut(): Promise<void> {
    validating = true;
    getOidcClient().clearStoredTokenResponse();
    await goto(resolve("/signin"));
}

async function authenticationChanged(event: StorageEvent): Promise<void> {
    if (event.key !== null && !event.key.startsWith("oidc.token")) return;
    validating = true;
    try {
        await invalidateAll();
    } finally {
        validating = false;
    }
}
</script>

<svelte:window onstorage={(event) => void authenticationChanged(event)} />
{#if !validating && data.userInfo?.sub}
    <button class="btn secondary" type="button" onclick={() => void signOut()}>Sign out</button>
    {#key JSON.stringify([getIdpApiUrl(), data.userInfo.sub])}
        <Browser userId={data.userInfo.sub} />
    {/key}
{/if}
