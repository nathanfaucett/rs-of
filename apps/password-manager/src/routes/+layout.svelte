<script lang="ts">
import "../app.css";

import { getTheme } from "@ofsdk/svelte-headless";
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

$effect(() => {
    if (!authState.session) return;
    return () => void syncStateModule.closeSync();
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

<svelte:window onfocus={checkInactivity} onpageshow={checkInactivity} />
<svelte:document onvisibilitychange={checkInactivity} onpointerdown={recordActivity} onkeydown={recordActivity} oninput={recordActivity} />

<OfflineStatus />
{@render children()}
{#if authState.isAuthenticated}
  <Notifications />
{/if}
