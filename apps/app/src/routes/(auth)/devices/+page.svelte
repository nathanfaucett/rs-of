<script lang="ts">
    import { toDataURL } from "qrcode";
    import { onMount } from "svelte";
    import SourceApproval from "$lib/common/components/SourceApproval.svelte";
    import DevicePolicy from "$lib/common/components/DevicePolicy.svelte";

    import {
        type DeviceInfo,
        listDevices,
        renameDevice,
        revokeDevice,
    } from "$lib/common/state/devices.svelte";
    import { deviceApi } from "$lib/common/state/deviceClient.svelte";
    import { notifications } from "$lib/common/state/notifications.svelte";

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

<div class="flex flex-col gap-4">
    <div class="flex items-center justify-between">
        <h1 class="mb-0 text-4xl">Devices</h1>
        <div class="flex gap-2">
            <button
                type="button"
                class="btn secondary"
                onclick={() => void loadDevices()}
                disabled={loading}>Refresh</button
            >
        </div>
    </div>

    <DevicePolicy {devices} />

    <SourceApproval
        endpoint={deviceEndpointId}
        qr={publicIdQr}
        oncopy={(value) => void onCopy(value, "Endpoint ID")}
    />

    {#if error}<div class="card border border-red-700 text-red-200">
            {error}
        </div>{/if}
    {#if initialLoading}<div class="card">Loading devices...</div>{/if}

    {#if empty}
        <div class="card secondary">
            <h2 class="mb-0 text-2xl">No devices yet</h2>
        </div>
    {/if}

    {#if hasDevices}
        <div class="card secondary overflow-x-auto p-0">
            <table class="min-w-full border-collapse text-left">
                <thead>
                    <tr class="border-b border-gray-300 dark:border-gray-700">
                        <th class="px-4 py-3">Name</th>
                        <th class="px-4 py-3">State</th>
                        <th class="px-4 py-3">Public key</th>
                        <th class="px-4 py-3">Updated</th>
                        <th class="px-4 py-3">Actions</th>
                    </tr>
                </thead>
                <tbody>
                    {#each devices as device (device.id)}
                        <tr
                            class="border-b border-gray-200 align-top last:border-b-0 dark:border-gray-800"
                        >
                            <td class="px-4 py-3">
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
                                            type="submit"
                                            class="btn secondary">Save</button
                                        >
                                    </form>
                                {:else}
                                    {device.name}
                                {/if}
                            </td>
                            <td class="px-4 py-3">{device.state}</td>
                            <td class="px-4 py-3"
                                ><code title={device.publicKey}
                                    >{shortValue(device.publicKey)}</code
                                ></td
                            >
                            <td class="px-4 py-3"
                                >{formatTimestamp(device.updatedAt)}</td
                            >
                            <td class="px-4 py-3">
                                {#if device.state !== "revoked"}
                                    <div class="flex gap-2">
                                        <button
                                            type="button"
                                            class="btn secondary"
                                            onclick={() =>
                                                onStartRename(device)}
                                            >Rename</button
                                        >
                                        {#if deviceEndpointId !== null && device.publicKey !== deviceEndpointId}
                                            <button
                                                type="button"
                                                class="btn secondary"
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
</div>
