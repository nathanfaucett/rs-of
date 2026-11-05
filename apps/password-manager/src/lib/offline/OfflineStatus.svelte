<script lang="ts">
import { onMount } from "svelte";
import { dev } from "$app/environment";
import { checkLocalStorage, offlineError, prepareOffline } from "./offline";

let status = $state("Checking offline setup…");
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
        status = dev ? "Offline startup requires a production build, not the development server." : await prepareOffline();
                if (storageError && status === "Offline ready") status = "Offline assets ready; local vault storage is unavailable.";
    } catch (cause) {
        status = offlineError(cause);
    } finally {
        busy = false;
    }
}

onMount(() => { void check(); });
</script>

<svelte:window bind:online />

<aside aria-label="Offline and storage status" class="max-h-48 shrink-0 overflow-auto border-b border-line p-3 text-sm">
    <p role="status">{status}</p>
    {#if !online}<p>Network is offline. Local vault operations do not need a server.</p>{/if}
    {#if storageError}<p role="alert">{storageError}</p>{/if}
    <p>Browser storage can be evicted. Private browsing can remove data when closed. Local storage is not a backup. Keep an external encrypted backup.</p>
    <button class="btn btn-secondary btn-sm" type="button" disabled={busy} onclick={() => void check()}>Check offline setup</button>
</aside>
