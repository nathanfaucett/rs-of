<script lang="ts">
    import { onMount } from "svelte";
    import DevicePolicy from "$lib/common/components/DevicePolicy.svelte";
    import ListDiscovery from "$lib/common/components/ListDiscovery.svelte";
    import Modal from "$lib/common/components/Modal.svelte";
    import { deviceApi } from "$lib/common/state/deviceClient.svelte";
    import {
        approveDeviceEnrollment,
        type DeviceEnrollment,
        type DeviceInfo,
        enrollDevice,
        listDevices,
        listPendingEnrollments,
        renameDevice,
        revokeDevice,
    } from "$lib/common/state/devices.svelte";
    import { discoverRows, type ListOrder } from "$lib/common/state/listDiscovery";
    import {
        managementPolicyError,
        managementPolicyStatus,
    } from "$lib/common/state/managementPolicyError";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Alert, Card, Field, Page, PageHeader } from "$lib/ui";

    let devices = $state<DeviceInfo[]>([]);
    let pendingEnrollments = $state<DeviceEnrollment[]>([]);
    let loading = $state(false);
    let error = $state<string | null>(null);
    let deviceEndpointId = $state<string | null>(null);
    let editingId = $state<string | null>(null);
    let editingName = $state("");
    let renaming = $state(false);
    let renameError = $state<string | null>(null);
    let enrolling = $state(false);
    let enrollmentError = $state<string | null>(null);
    let newDeviceName = $state("");
    let newDeviceAddress = $state("");
    let newDevicePublicKey = $state("");
    let enrollmentFormModalRef = $state<Modal>();

    const initialLoading = $derived(!error && loading && devices.length === 0);
    const empty = $derived(!error && !loading && devices.length === 0);
    const hasDevices = $derived(!error && devices.length > 0);
    let search = $state("");
    let order = $state<ListOrder>("default");
    const visibleRows = $derived(
        discoverRows(devices, search, order, (row) => [row.id, row.state, row.publicKey]),
    );

    function formatTimestamp(value: number | null | undefined): string {
        // Initial Device provisioning uses 1 when it has no update time.
        if (value == null || value <= 1) return "Unknown";
        const date = new Date(value * 1000);
        return Number.isNaN(date.getTime()) ? "Unknown" : date.toLocaleString();
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
            pendingEnrollments = await listPendingEnrollments();
        } catch (cause) {
            error = managementPolicyError(cause, "load Devices");
        } finally {
            loading = false;
        }
    }

    async function onApproveEnrollment(enrollment: DeviceEnrollment) {
        try {
            await approveDeviceEnrollment(enrollment.id);
            notifications.add("Device enrollment approved", "success");
            await loadDevices();
        } catch (cause) {
            console.error(cause);
            notifications.add("Failed to approve enrollment", "error");
        }
    }

    function onStartRename(device: DeviceInfo) {
        editingId = device.id;
        editingName = device.name;
        renameError = null;
    }

    async function onRename(device: DeviceInfo) {
        if (renaming) return;
        const name = editingName.trim();
        if (!name || new TextEncoder().encode(name).length > 512) {
            renameError = "Device name must be 1 to 512 bytes after trimming.";
            return;
        }
        renameError = null;
        renaming = true;
        try {
            const renamed = await renameDevice(device.id, name);
            devices = devices.map((record) => record.id === renamed.id ? renamed : record);
            editingId = null;
            notifications.add("Device renamed", "success");
            await loadDevices();
        } catch (cause) {
            renameError = managementPolicyStatus(cause) === 403
                ? "Only the Device owner with rename permission can rename it. Ask an administrator for access."
                : managementPolicyError(cause, "rename the Device");
        } finally {
            renaming = false;
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

    function openEnrollmentForm() {
        enrollmentFormModalRef?.show();
    }

    async function onEnrollDevice() {
        if (enrolling) return;
        const name = newDeviceName.trim();
        const address = newDeviceAddress.trim();
        const publicKey = newDevicePublicKey.trim();
        const encoder = new TextEncoder();
        for (const [label, value] of [
            ["Device name", name],
            ["Device address", address],
        ]) {
            if (!value || encoder.encode(value).length > 512) {
                enrollmentError = `${label} must be 1 to 512 bytes after trimming.`;
                return;
            }
        }
        if (!/^[0-9a-f]{64}$/.test(publicKey)) {
            enrollmentError = "Enter the Device's Iroh endpoint public key as 64 lowercase hexadecimal characters.";
            return;
        }
        enrollmentError = null;
        enrolling = true;
        try {
            await enrollDevice(address, name, publicKey);
            newDeviceName = "";
            newDeviceAddress = "";
            newDevicePublicKey = "";
            enrollmentFormModalRef?.close();
            notifications.add("Device enrolled", "success");
            await loadDevices();
        } catch (cause) {
            enrollmentError = managementPolicyStatus(cause) === 403
                ? "Only the first Device can enroll here. For another Device, use pairing. If this is the first Device, ask an administrator for enrollment access."
                : managementPolicyError(cause, "enroll the Device");
        } finally {
            enrolling = false;
        }
    }

    onMount(() => {
        void deviceApi
            .device()
            .then((device) => {
                deviceEndpointId = device.publicKey;
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
                class="btn btn-primary btn-md"
                type="button"
                onclick={openEnrollmentForm}
            >
                Add device
            </button>
            <button
                class="btn btn-secondary btn-md"
                type="button"
                onclick={() => void loadDevices()}
                disabled={loading}>Refresh</button
            >
        {/snippet}
    </PageHeader>

    <DevicePolicy {devices} />

    {#if error}
        <Alert tone="danger">{error}</Alert>
    {/if}
    {#if initialLoading}
        <Card tone="subtle">Loading devices...</Card>
    {/if}

    <ListDiscovery
        listScope="loaded devices"
        fields="Name, ID, state, or public key. Does not include pending enrollments."
        bind:search
        bind:order
    />

    {#if empty}
        <Card tone="subtle">
            <h2 class="mb-0 text-base">No devices yet</h2>
        </Card>
    {/if}

    {#if hasDevices && visibleRows.length === 0}
        <Card tone="subtle">
            <p class="mb-0" role="status">No matching devices.</p>
        </Card>
    {:else if hasDevices}
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
                    {#each visibleRows as device (device.id)}
                        <tr>
                            <td data-label="Name">
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
                                            disabled={renaming}
                                            required
                                        />
                                        <button
                                            class="btn btn-secondary btn-md"
                                            type="submit"
                                            disabled={renaming}>Save</button
                                        >
                                        <button
                                            class="btn btn-secondary btn-md"
                                            type="button"
                                            disabled={renaming}
                                            onclick={() => { editingId = null; }}
                                            >Cancel</button
                                        >
                                    </form>
                                    {#if renameError}<Alert tone="danger">{renameError}</Alert>{/if}
                                {:else}
                                    {device.name}
                                {/if}
                            </td>
                            <td data-label="State">{device.state}</td>
                            <td data-label="Public key">
                                <code title={device.publicKey}
                                    >{shortValue(device.publicKey)}</code
                                >
                            </td>
                            <td data-label="Updated">{formatTimestamp(device.updatedAt)}</td>
                            <td data-label="Actions">
                                {#if device.state !== "revoked"}
                                    <div class="flex flex-wrap gap-2">
                                        <button
                                            class="btn btn-secondary btn-md"
                                            type="button"
                                            disabled={renaming || editingId !== null}
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

    {#if pendingEnrollments.length > 0}
        <Card tone="subtle">
            <h2 class="mb-3 text-base">Pending Enrollments</h2>
            <div class="overflow-x-auto">
                <table class="table">
                    <thead>
                        <tr>
                            <th>Device ID</th>
                            <th>State</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        {#each pendingEnrollments as enrollment (enrollment.id)}
                            <tr>
                                <td data-label="Device ID">
                                    <code title={enrollment.id}
                                        >{shortValue(enrollment.id)}</code
                                    >
                                </td>
                                <td data-label="State">{enrollment.state}</td>
                                <td data-label="Actions">
                                    <button
                                        class="btn btn-primary btn-md"
                                        type="button"
                                        onclick={() =>
                                            void onApproveEnrollment(
                                                enrollment,
                                            )}>Approve</button
                                    >
                                </td>
                            </tr>
                        {/each}
                    </tbody>
                </table>
            </div>
        </Card>
    {/if}

    <Modal bind:this={enrollmentFormModalRef} title="Enroll New Device">
        <form
            class="flex flex-col gap-4"
            onsubmit={(e) => {
                e.preventDefault();
                void onEnrollDevice();
            }}
        >
            <p>This form enrolls the first Device only. Use pairing to add another Device.</p>
            <Field label="Device name" required>
                {#snippet children(control)}
                    <input bind:value={newDeviceName} type="text" disabled={enrolling} {...control} />
                {/snippet}
            </Field>
            <Field
                label="Device address"
                description="Enter the endpoint address, for example 127.0.0.1:23456. The Device service stores it as text, up to 512 bytes."
                required
            >
                {#snippet children(control)}
                    <input bind:value={newDeviceAddress} type="text" disabled={enrolling} {...control} />
                {/snippet}
            </Field>
            <Field
                label="Endpoint public key"
                description="Copy the 64-character lowercase hexadecimal Iroh public key from the Device. Keep its private key on that Device."
                required
            >
                {#snippet children(control)}
                    <input bind:value={newDevicePublicKey} type="text" disabled={enrolling} spellcheck={false} {...control} />
                {/snippet}
            </Field>
            {#if enrollmentError}
                <Alert tone="danger">{enrollmentError}</Alert>
            {/if}
            <div class="flex justify-end gap-2">
                <button
                    class="btn btn-secondary btn-md"
                    type="button"
                    onclick={() => enrollmentFormModalRef?.close()}
                >
                    Cancel
                </button>
                <button
                    class="btn btn-primary btn-md"
                    type="submit"
                    disabled={enrolling}
                >
                    {enrolling ? "Enrolling..." : "Enroll device"}
                </button>
            </div>
        </form>
    </Modal>
</Page>
