<script lang="ts">
    import { isTauri } from "@tauri-apps/api/core";
    import { resolve } from "$app/paths";
    import NativeSetupRecovery from "$lib/common/components/NativeSetupRecovery.svelte";
    import { usesRemoteApis } from "$lib/common/state/setupClient.svelte";
</script>

<main class="mx-auto flex max-w-lg flex-col gap-4 px-4 py-8">
    <h1>Maintenance</h1>
    <a href={resolve("/setup")}>Return to setup and connection status</a>
    {#if isTauri() && !usesRemoteApis()}
        <NativeSetupRecovery />
    {:else}
        <p>Local maintenance requires the desktop with bundled services selected. Remote owners require their own protected host maintenance; this page cannot reset them.</p>
        <a href={resolve("/config")}>Connection settings</a>
    {/if}
</main>
