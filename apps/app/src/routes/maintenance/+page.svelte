<script lang="ts">
    import { isTauri } from "@tauri-apps/api/core";
    import { resolve } from "$app/paths";
    import NativeSetupRecovery from "$lib/common/components/NativeSetupRecovery.svelte";
    import { usesRemoteApis } from "$lib/common/state/setupClient.svelte";
    import { Alert, Page, PageHeader } from "$lib/ui";
</script>

<Page>
    <PageHeader title="Maintenance" />
    <a href={resolve("/setup")} class="btn btn-primary btn-md"
        >Return to setup and connection status</a
    >
    {#if isTauri() && !usesRemoteApis()}
        <NativeSetupRecovery />
    {:else}
        <Alert tone="info"
            >Local maintenance requires the desktop with bundled services
            selected. Remote owners require their own protected host
            maintenance; this page cannot reset them.</Alert
        >
        <a href={resolve("/config")}>Connection settings</a>
    {/if}
</Page>
