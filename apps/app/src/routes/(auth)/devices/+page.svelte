<script lang="ts">
    import { toDataURL } from "qrcode";
    import { onMount } from "svelte";
    import Modal from "$lib/common/components/Modal.svelte";
    import SyncPanel from "$lib/common/components/SyncPanel.svelte";
    import DevicePolicy from "$lib/common/components/DevicePolicy.svelte";
    import SourceApproval from "$lib/common/components/SourceApproval.svelte";
    import {
        deviceApi,
        getDeviceApiUrl,
    } from "$lib/common/state/deviceClient.svelte";
    import {
        type DeviceEnrollment,
        type DeviceInfo,
        listDevices,
        listPendingEnrollments,
        approveDeviceEnrollment,
        renameDevice,
        revokeDevice,
        enrollDevice,
    } from "$lib/common/state/devices.svelte";
    import { notifications } from "$lib/common/state/notifications.svelte";
    import { Alert, Card, Page, PageHeader, Field } from "$lib/ui";

    let devices = $state<DeviceInfo[]>([]);
    let pendingEnrollments = $state<DeviceEnrollment[]>([]);
    let loading = $state(false);
    let error = $state<string | null>(null);
    let deviceEndpointId = $state<string | null>(null);
    let editingId = $state<string | null>(null);
    let editingName = $state("");
    let publicIdQr = $state<string | null>(null);
    let enrolling = $state(false);
    let enrollmentError = $state<string | null>(null);
    let enrollmentResultModal = $state<{
        enrollment: DeviceEnrollment;
        deviceUri: string;
        qrCode: string;
    } | null>(null);
    let newDeviceName = $state("");
    let newDeviceAddress = $state("");
    let generatingKeys = $state(false);

    let enrollmentFormModalRef = $state<Modal>();
    let enrollmentResultModalRef = $state<Modal>();

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
            pendingEnrollments = await listPendingEnrollments();
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

    async function generateKeypair(): Promise<{ publicKey: string }> {
        const keyPair = await crypto.subtle.generateKey(
            { name: "ECDSA", namedCurve: "P-256" },
            true,
            ["sign", "verify"],
        );
        const publicKeyJwk = await crypto.subtle.exportKey(
            "jwk",
            keyPair.publicKey,
        );
        const publicKey = JSON.stringify(publicKeyJwk);
        return { publicKey };
    }

    function openEnrollmentForm() {
        enrollmentError = null;
        newDeviceName = "";
        newDeviceAddress = "";
        enrollmentFormModalRef.show();
    }

    async function onEnrollDevice() {
        if (!newDeviceName.trim() || !newDeviceAddress.trim()) {
            enrollmentError = "Device name and address are required";
            return;
        }
        enrollmentError = null;
        enrolling = true;
        generatingKeys = true;
        try {
            const { publicKey } = await generateKeypair();
            generatingKeys = false;
            const enrollment = await enrollDevice(
                newDeviceAddress.trim(),
                newDeviceName.trim(),
                publicKey,
            );
            const deviceApiUrl = getDeviceApiUrl();
            const deviceUri = `${deviceApiUrl}#${publicKey}`;
            const qrCode = await toDataURL(deviceUri);
            enrollmentResultModal = { enrollment, deviceUri, qrCode };
            enrollmentFormModalRef.close();
            enrollmentResultModalRef.show();
            await loadDevices();
        } catch (cause) {
            console.error(cause);
            enrollmentError =
                cause instanceof Error
                    ? cause.message
                    : "Failed to enroll device";
        } finally {
            enrolling = false;
            generatingKeys = false;
        }
    }

    function onEnrollmentResultClose() {
        enrollmentResultModal = null;
    }

    onMount(() => {
        void deviceApi
            .device()
            .then(async (device) => {
                deviceEndpointId = device.publicKey;
                publicIdQr = await toDataURL(
                    `${getDeviceApiUrl()}#${device.publicKey}`,
                );
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

    <SyncPanel />

    <DevicePolicy {devices} />

    <div id="device-pairing"></div>
    <SourceApproval
        endpoint={deviceEndpointId}
        qr={publicIdQr}
        oncopy={(value) => void onCopy(value, "Trusted peer")}
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
                                <td>
                                    <code title={enrollment.id}
                                        >{shortValue(enrollment.id)}</code
                                    >
                                </td>
                                <td>{enrollment.state}</td>
                                <td>
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
            <Field
                label="Device name"
                required
                issues={enrollmentError ? [{ message: enrollmentError }] : []}
            >
                {#snippet children(control)}
                    <input
                        bind:value={newDeviceName}
                        type="text"
                        placeholder="My Device"
                        disabled={enrolling}
                        aria-invalid={!!enrollmentError}
                        {...control}
                    />
                {/snippet}
            </Field>
            <Field
                label="Device address"
                required
                issues={enrollmentError ? [{ message: enrollmentError }] : []}
            >
                {#snippet children(control)}
                    <input
                        bind:value={newDeviceAddress}
                        type="text"
                        placeholder="https://device.example.com"
                        disabled={enrolling}
                        aria-invalid={!!enrollmentError}
                        {...control}
                    />
                {/snippet}
            </Field>
            {#if enrollmentError}
                <Alert tone="danger">{enrollmentError}</Alert>
            {/if}
            <div class="flex justify-end gap-2">
                <button
                    class="btn btn-secondary btn-md"
                    type="button"
                    formmethod="dialog"
                    onclick={() => enrollmentFormModalRef.close()}
                >
                    Cancel
                </button>
                <button
                    class="btn btn-primary btn-md"
                    type="submit"
                    disabled={enrolling || generatingKeys}
                >
                    {generatingKeys
                        ? "Generating keys..."
                        : enrolling
                          ? "Enrolling..."
                          : "Enroll device"}
                </button>
            </div>
        </form>
    </Modal>

    {#if enrollmentResultModal}
        <Modal bind:this={enrollmentResultModalRef} title="Device Enrolled">
            <div class="flex flex-col gap-4">
                <p class="mb-0 text-sm text-ink-muted">
                    Device enrolled successfully. Share the device URI or scan
                    the QR code to complete setup.
                </p>
                <div class="grid gap-3 md:grid-cols-2">
                    <div class="flex flex-col gap-2">
                        <label for="device-uri" class="text-sm font-medium"
                            >Device URI</label
                        >
                        <div class="flex gap-2">
                            <input
                                id="device-uri"
                                type="text"
                                value={enrollmentResultModal.deviceUri}
                                readonly
                                class="flex-1 font-mono text-sm"
                            />
                            <button
                                class="btn btn-secondary btn-md"
                                type="button"
                                onclick={() =>
                                    void onCopy(
                                        enrollmentResultModal.deviceUri,
                                        "Device URI",
                                    )}
                            >
                                Copy
                            </button>
                        </div>
                    </div>
                    <div class="flex flex-col gap-2">
                        <span id="qr-code-label" class="text-sm font-medium"
                            >QR Code</span
                        >
                        <img
                            src={enrollmentResultModal.qrCode}
                            alt="Device URI QR code"
                            class="w-48 h-48 mx-auto"
                            aria-labelledby="qr-code-label"
                        />
                    </div>
                </div>
                {#if enrollmentResultModal.enrollment.approvalCode}
                    <div class="flex flex-col gap-2">
                        <label for="approval-code" class="text-sm font-medium"
                            >Approval Code</label
                        >
                        <div class="flex gap-2">
                            <input
                                id="approval-code"
                                type="text"
                                value={enrollmentResultModal.enrollment
                                    .approvalCode}
                                readonly
                                class="flex-1 font-mono text-sm"
                            />
                            <button
                                class="btn btn-secondary btn-md"
                                type="button"
                                onclick={() =>
                                    void onCopy(
                                        enrollmentResultModal.enrollment
                                            .approvalCode!,
                                        "Approval code",
                                    )}
                            >
                                Copy
                            </button>
                        </div>
                        <p class="mb-0 text-sm text-ink-muted">
                            Provide this approval code to the device to complete
                            enrollment.
                        </p>
                    </div>
                {/if}
                <form method="dialog" class="flex justify-end">
                    <button class="btn btn-primary btn-md" type="submit">
                        Done
                    </button>
                </form>
            </div>
        </Modal>
    {/if}
</Page>
