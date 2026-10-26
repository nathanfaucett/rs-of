<script lang="ts">
import { goto } from "$app/navigation";
import { resolve } from "$app/paths";
import { flushSync, onMount } from "svelte";
import Browser from "$lib/collections/secrets/Browser.svelte";
import { notifications } from "$lib/common/state/notifications.svelte";
import { getSdk } from "$lib/common/state/sdk";
import type { PageProps } from "./$types";

let { data }: PageProps = $props();
const sdk = getSdk();
let session = $state.raw(sdk.session);
onMount(() => sdk.subscribe((next) => {
    const wasAuthenticated = session.status === "authenticated";
    session = next;
    if ((wasAuthenticated && next.status !== "authenticated") || next.status === "ended" || next.status === "disposed") flushSync();
}));
async function signOut(): Promise<void> {
    try { await sdk.signout(); }
    catch { notifications.add("Local credential removal failed. Clear browser site data before signing in again."); }
    await goto(resolve("/signin"));
}
</script>

{#if session.status === 'authenticated' && session.identity?.subject === data.userInfo?.sub}
    <button class="btn secondary" type="button" onclick={() => void signOut()}>Sign out</button>
    {#key session.generation}
        <Browser userId={session.identity.subject} />
    {/key}
{/if}
