<script lang="ts">
import { onMount } from "svelte";
import { dev } from "$app/environment";
import { checkLocalStorage, offlineError, prepareOffline } from "./offline";

let status = $state<string | null>(null);
let storageError = $state("");
let busy = $state(false);
let online = $state(true);

async function check() {
    busy = true;
    storageError = "";
    try {
        await checkLocalStorage();
    } catch (cause) {
        storageError = cause instanceof Error && !(cause instanceof DOMException) ? cause.message : offlineError(cause);
    }
    try {
        if (!dev) {
            status = await prepareOffline();
            if (storageError && status === "Offline ready") status = "Offline assets ready; local vault storage is unavailable.";
        }
    } catch (cause) {
        if (!dev) status = offlineError(cause);
    } finally {
        busy = false;
    }
}

onMount(() => { void check(); });
</script>

<svelte:window bind:online />

{#if status || storageError || !online}
<aside aria-label="Offline and storage status" class="max-h-48 shrink-0 overflow-auto border-b border-line p-3 text-sm">
    {#if status}<p role="status">{status}</p>{/if}
    {#if !online}<p>Network is offline. Local vault operations do not need a server.</p>{/if}
    {#if storageError}<p role="alert">{storageError}</p>{/if}
    <p>Browser storage can be evicted. Private browsing can remove data when closed. Local storage is not a backup. Keep an external encrypted backup.</p>
    <button class="btn btn-secondary btn-sm" type="button" disabled={busy} onclick={() => void check()}>Check offline setup</button>
</aside>
{/if}
