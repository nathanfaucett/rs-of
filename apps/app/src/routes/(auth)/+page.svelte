<script lang="ts">
    import { onMount } from "svelte";
    import { resolve } from "$app/paths";
    import { Card, Page, PageHeader } from "$lib/ui";
    import { idpApi } from "$lib/common/state/idpClient.svelte";
    import { deviceApi } from "$lib/common/state/deviceClient.svelte";
    import type { PageProps } from "./$types";

    let { data }: PageProps = $props();

    const userInfo = $derived(data.userInfo);
    const displayName = $derived(
        userInfo?.name ||
            userInfo?.preferredUsername ||
            userInfo?.givenName ||
            userInfo?.email ||
            "User",
    );

    let statsLoading = $state(true);
    let statsError = $state<string | null>(null);
    let userCount = $state(0);
    let deviceCount = $state(0);

    async function loadStats() {
        statsLoading = true;
        statsError = null;
        try {
            const [users, devices] = await Promise.all([
                idpApi.listUsers({ limit: 1000 }),
                deviceApi.listDevices(),
            ]);
            userCount = users.length;
            deviceCount = devices.length;
        } catch (cause) {
            console.error(cause);
            statsError =
                cause instanceof Error ? cause.message : "Failed to load stats";
        } finally {
            statsLoading = false;
        }
    }

    onMount(() => {
        void loadStats();
    });
</script>

<Page>
    <PageHeader
        title="Dashboard"
        description="Manage applications, devices, and user records."
    />

    <Card>
        <p class="mb-0">Signed in as <strong>{displayName}</strong></p>
    </Card>

    {#if statsError}
        <Card tone="danger">Failed to load stats: {statsError}</Card>
    {:else if statsLoading}
        <Card tone="subtle">Loading dashboard...</Card>
    {:else}
        <div class="grid grid-cols-2 gap-4 md:grid-cols-2 mb-6">
            <div
                class="rounded-card border border-line bg-surface p-5 text-center"
            >
                <p class="mb-0 text-3xl font-bold">{userCount}</p>
                <p class="mb-0 text-sm text-ink-muted">Users</p>
            </div>
            <div
                class="rounded-card border border-line bg-surface p-5 text-center"
            >
                <p class="mb-0 text-3xl font-bold">{deviceCount}</p>
                <p class="mb-0 text-sm text-ink-muted">Devices</p>
            </div>
        </div>
    {/if}

    <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
        <a
            class="rounded-card border border-line bg-surface p-5 no-underline shadow-sm transition-colors hover:bg-surface-hover"
            href={resolve("/applications")}
        >
            <h2 class="mb-1 text-lg">Applications</h2>
            <p class="mb-0 text-sm text-ink-muted">
                Manage application settings, roles, permissions, and clients.
            </p>
        </a>
        <a
            class="rounded-card border border-line bg-surface p-5 no-underline shadow-sm transition-colors hover:bg-surface-hover"
            href={resolve("/devices")}
        >
            <h2 class="mb-1 text-lg">Devices</h2>
            <p class="mb-0 text-sm text-ink-muted">
                Approve and revoke the devices that can synchronize your files.
            </p>
        </a>
        <a
            class="rounded-card border border-line bg-surface p-5 no-underline shadow-sm transition-colors hover:bg-surface-hover"
            href={resolve("/users")}
        >
            <h2 class="mb-1 text-lg">Users</h2>
            <p class="mb-0 text-sm text-ink-muted">
                Browse and manage user records.
            </p>
        </a>
    </div>
</Page>
