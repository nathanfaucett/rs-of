<script lang="ts">
    import { toDataURL } from "qrcode";
    import { onMount } from "svelte";
    import DevicePolicy from "$lib/common/components/DevicePolicy.svelte";
    import SourceApproval from "$lib/common/components/SourceApproval.svelte";
    import { deviceApi } from "$lib/common/state/deviceClient.svelte";
    import {
        type DeviceInfo,
        listDevices,
        renameDevice,
        revokeDevice,
    } from "$lib/common/state/devices.svelte";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Alert, Card, Page, PageHeader } from "$lib/ui";

    let devices = $state<DeviceInfo[]>([]);
    let loading = $state(false);
    let error = $state<string | null>(null);
    let deviceEndpointId = $state<string | null>(null);
    let editingId = $state<string | null>(null);
    let editingName = $state("");
    let publicIdQr = $state<string | null>(null);

    const initialLoading = $derived(!error && loading && devices.length === 0);
    const empty = $derived(!error && !loading && devices.length === 0);
    const hasDevices = $derived(!error && devices.length > 0);
    function formatTimestamp(value: number): string {
        return new Date(value * 1000).toLocaleString();
    }

    function shortValue(value: string): string {
        return value.length > 20
            ? `${value.slice(0, 10)}…${value.slice(-8)}`
            : value;
    }

    async function loadDevices() {
        if (loading) return;
        loading = true;
        error = null;
        try {
            devices = await listDevices();
        } catch (cause) {
            console.error(cause);
            error =
                cause instanceof Error
                    ? cause.message
                    : "Failed to load devices";
        } finally {
            loading = false;
        }
    }

    async function onCopy(value: string, label: string) {
        try {
            await navigator.clipboard.writeText(value);
            notifications.add(`${label} copied`, "success");
        } catch {
            notifications.add(`Could not copy ${label.toLowerCase()}`, "error");
        }
    }

    function onStartRename(device: DeviceInfo) {
        editingId = device.id;
        editingName = device.name;
    }

    async function onRename(device: DeviceInfo) {
        if (!editingName.trim()) {
            return;
        }
        try {
            await renameDevice(device.id, editingName.trim());
            editingId = null;
            notifications.add("Device renamed", "success");
            await loadDevices();
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to rename device", "error");
        }
    }

    async function onRevoke(device: DeviceInfo) {
        if (
            !confirm(
                `Revoke ${device.name}? This stops future synchronization.`,
            )
        ) {
            return;
        }
        try {
            await revokeDevice(device.id);
            notifications.add("Device revoked", "success");
            await loadDevices();
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to revoke device", "error");
        }
    }

    onMount(() => {
        void deviceApi
            .device()
            .then(async (device) => {
                deviceEndpointId = device.publicKey;
                publicIdQr = await toDataURL(device.publicKey);
            })
            .catch(console.error);
        void loadDevices();
        const timer = setInterval(() => {
            void loadDevices();
        }, 5000);
        return () => {
            clearInterval(timer);
        };
    });
</script>

<Page>
    <PageHeader title="Devices">
        {#snippet actions()}
            <button
                class="btn btn-secondary btn-md"
                type="button"
                onclick={() => void loadDevices()}
                disabled={loading}>Refresh</button
            >
        {/snippet}
    </PageHeader>

    <DevicePolicy {devices} />

    <SourceApproval
        endpoint={deviceEndpointId}
        qr={publicIdQr}
        oncopy={(value) => void onCopy(value, "Endpoint ID")}
    />

    {#if error}
        <Alert tone="danger">{error}</Alert>
    {/if}
    {#if initialLoading}
        <Card tone="subtle">Loading devices...</Card>
    {/if}

    {#if empty}
        <Card tone="subtle">
            <h2 class="mb-0 text-base">No devices yet</h2>
        </Card>
    {/if}

    {#if hasDevices}
        <div class="overflow-x-auto rounded-card border border-line bg-surface">
            <table class="table">
                <thead>
                    <tr>
                        <th>Name</th>
                        <th>State</th>
                        <th>Public key</th>
                        <th>Updated</th>
                        <th>Actions</th>
                    </tr>
                </thead>
                <tbody>
                    {#each devices as device (device.id)}
                        <tr>
                            <td>
                                {#if editingId === device.id}
                                    <form
                                        class="flex gap-2"
                                        onsubmit={(event) => {
                                            event.preventDefault();
                                            void onRename(device);
                                        }}
                                    >
                                        <input
                                            bind:value={editingName}
                                            type="text"
                                            aria-label="Device name"
                                        />
                                        <button
                                            class="btn btn-secondary btn-md"
                                            type="submit">Save</button
                                        >
                                    </form>
                                {:else}
                                    {device.name}
                                {/if}
                            </td>
                            <td>{device.state}</td>
                            <td>
                                <code title={device.publicKey}
                                    >{shortValue(device.publicKey)}</code
                                >
                            </td>
                            <td>{formatTimestamp(device.updatedAt)}</td>
                            <td>
                                {#if device.state !== "revoked"}
                                    <div class="flex gap-2">
                                        <button
                                            class="btn btn-secondary btn-md"
                                            type="button"
                                            onclick={() =>
                                                onStartRename(device)}
                                            >Rename</button
                                        >
                                        {#if deviceEndpointId !== null && device.publicKey !== deviceEndpointId}
                                            <button
                                                class="btn btn-secondary btn-md"
                                                type="button"
                                                onclick={() =>
                                                    void onRevoke(device)}
                                                >Revoke</button
                                            >
                                        {/if}
                                    </div>
                                {/if}
                            </td>
                        </tr>
                    {/each}
                </tbody>
            </table>
        </div>
    {/if}
</Page>
