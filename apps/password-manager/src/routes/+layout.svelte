<script lang="ts">
import "../app.css";

import { untrack } from "svelte";
import { getTheme } from "@ofsdk/svelte-headless";
import { syncStateModule } from "$lib/shared/state/sync.svelte";
import favicon from "$lib/assets/favicon.svg";
import Notifications from "$lib/shared/components/Notifications.svelte";
import OfflineStatus from "$lib/shared/offline/OfflineStatus.svelte";
import { authState } from "$lib/auth/auth.svelte";
import type { LayoutProps } from "./$types";

let { children }: LayoutProps = $props();

$effect(() => {
    const user = authState.user;
    untrack(() => {
        if (user) {
            void syncStateModule.initializeSync();
        } else {
            void syncStateModule.closeSync();
        }
    });
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

{#if authState.isLoading}
  <div class="flex min-h-screen items-center justify-center" role="status">Loading…</div>
{:else if !authState.isAuthenticated}
  {@render children()}
{:else}
  <OfflineStatus />
  {@render children()}
  <Notifications />
{/if}
