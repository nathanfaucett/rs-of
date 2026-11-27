<script lang="ts">
import "../app.css";

import { getTheme } from "@ofsdk/svelte-headless";
import { onMount } from "svelte";
import favicon from "$lib/assets/favicon.svg";
import { authState } from "$lib/auth/auth.svelte";
import Notifications from "$lib/shared/components/Notifications.svelte";
import OfflineStatus from "$lib/shared/offline/OfflineStatus.svelte";
import { syncStateModule } from "$lib/shared/state/sync.svelte";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import type { LayoutProps } from "./$types";

let { children }: LayoutProps = $props();

function checkInactivity() {
    vaultStateModule.vaultData?.inactivity.check();
}

function recordActivity() {
    vaultStateModule.vaultData?.inactivity.activity();
}

onMount(() => {
    const unwatchDetach = syncStateModule.watchDetach();
    const unwatchSession = authState.watchSession((ended, previous) => {
        if (ended) {
            void vaultStateModule.showVaultSelector();
            if (previous) syncStateModule.clearPendingConfirmation(previous);
        }
        void syncStateModule.closeSync();
    });
    return () => {
        unwatchDetach();
        unwatchSession();
        void syncStateModule.closeSync();
    };
});

$effect(() => {
    if (getTheme() === "dark") {
        document.documentElement.classList.add("dark");
    } else {
        document.documentElement.classList.remove("dark");
    }
});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<svelte:window onstorage={authState.checkStoredSession} onfocus={checkInactivity} onpageshow={checkInactivity} onpagehide={() => void syncStateModule.closeSync()} />
<svelte:document onvisibilitychange={checkInactivity} onpointerdown={recordActivity} onkeydown={recordActivity} oninput={recordActivity} />

<OfflineStatus />
{@render children()}
{#if authState.isAuthenticated}
  <Notifications />
{/if}
