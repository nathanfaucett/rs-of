<script lang="ts">
    import {
        type DeviceInfo,
        type DeviceRestriction,
        SelectionKind,
    } from "@aicacia/device-client";
    import * as v from "valibot";
    import { deviceApi } from "$lib/common/state/deviceClient.svelte";
    import Alert from "$lib/ui/Alert.svelte";
    import Card from "$lib/ui/Card.svelte";
    import Field from "$lib/ui/Field.svelte";
    import ResourceResidency from "./ResourceResidency.svelte";

    let { devices }: { devices: DeviceInfo[] } = $props();
    let deviceId = $state("");
    let applicationId = $state("");
    let resourceId = $state("");
    let kind = $state<SelectionKind>(SelectionKind.Database);
    let storageAccessToken = $state("");
    let restriction = $state.raw<DeviceRestriction | null>(null);
    let busy = $state(false);
    let error = $state("");
    let statusMessage = $state("");
    const uuid = v.pipe(v.string(), v.uuid());
    const mutationAcknowledgementText =
        "The owner API does not expose a User-readable selection directory. Mutation acknowledgements above are not presented as a current selection list.";
    const residencyScopeText =
        "Full/Passthrough Residency controls apply to whole filesystems, not databases. Choose Filesystem and enter its resource UUID above.";

    async function readRestriction() {
        busy = true;
        error = "";
        restriction = null;
        try {
            v.parse(uuid, deviceId);
            restriction = await deviceApi.getDeviceRestriction({ deviceId });
        } catch (cause) {
            error = String(cause);
        } finally {
            busy = false;
        }
    }
    async function updateRestriction(allowed: boolean) {
        if (
            !confirm(
                `${allowed ? "Allow" : "Restrict"} replication policy for Device ${deviceId}? This is not a resource grant.`,
            )
        )
            return;
        busy = true;
        error = "";
        statusMessage = "";
        try {
            v.parse(uuid, deviceId);
            await deviceApi.putDeviceRestriction({
                deviceId,
                deviceRestriction: { adminAllowed: allowed },
            });
            restriction = await deviceApi.getDeviceRestriction({ deviceId });
            statusMessage =
                "Management acknowledged the restriction. Local-copy retirement may still be pending; no resource is reported ready.";
        } catch (cause) {
            error = String(cause);
        } finally {
            busy = false;
        }
    }
    async function updateSelection(select: boolean) {
        busy = true;
        error = "";
        statusMessage = "";
        try {
            for (const id of [deviceId, applicationId, resourceId])
                v.parse(uuid, id);
            if (select) {
                if (!storageAccessToken.trim())
                    throw new Error(
                        "An independently authorized, resource-scoped Storage token is required",
                    );
                await deviceApi.putDeviceSelection({
                    deviceId,
                    selectionRequest: {
                        applicationId,
                        id: resourceId,
                        kind,
                        storageAccessToken,
                    },
                });
            } else {
                if (
                    !confirm(
                        `Deselect ${kind} ${resourceId} in application ${applicationId}? Local retirement may remain incomplete.`,
                    )
                )
                    return;
                await deviceApi.deleteDeviceResourceSelection({
                    deviceId,
                    applicationId,
                    kind,
                    resourceId,
                });
            }
            statusMessage = `Management acknowledged ${select ? "selection" : "deselection"}. This is policy state only, not Storage authorization, local-copy cleanup completion, or Residency readiness.`;
        } catch (cause) {
            error = String(cause);
        } finally {
            storageAccessToken = "";
            busy = false;
        }
    }
</script>

<Card title="Device selections and restrictions">
    <div class="flex flex-col gap-3">
        <p>
            These operations use current Management owner APIs. Each request
            independently enforces its exact User permission and Device/resource
            owner binding; signing in or joining does not grant access.
        </p>
        <Field label="Device">
            {#snippet children({
                id,
                "aria-describedby": describedby,
                "aria-invalid": invalid,
                required,
            })}
                <select
                    {id}
                    aria-describedby={describedby}
                    aria-invalid={invalid}
                    {required}
                    bind:value={deviceId}
                    disabled={busy}
                    onchange={() => {
                        restriction = null;
                        statusMessage = "";
                    }}
                >
                    <option value="">Select a Device</option>
                    {#each devices as device (device.id)}<option
                            value={device.id}
                            >{device.name} — {device.id}</option
                        >{/each}
                </select>
            {/snippet}
        </Field>
        <button
            class="btn btn-secondary btn-md"
            type="button"
            onclick={readRestriction}
            disabled={busy || !deviceId}>Read current restriction</button
        >
        {#if restriction}<p>
                Current administrative replication permission: {restriction.adminAllowed
                    ? "allowed"
                    : "restricted"}.
            </p>{/if}
        <div class="flex gap-2">
            <button
                class="btn btn-secondary btn-md"
                type="button"
                onclick={() => updateRestriction(false)}
                disabled={busy || !deviceId}>Restrict replication</button
            >
            <button
                class="btn btn-secondary btn-md"
                type="button"
                onclick={() => updateRestriction(true)}
                disabled={busy || !deviceId}>Allow replication policy</button
            >
        </div>
        <form
            class="flex flex-col gap-3"
            onsubmit={(event) => {
                event.preventDefault();
                void updateSelection(true);
            }}
        >
            <Field label="Application UUID" required>
                {#snippet children({
                    id,
                    "aria-describedby": describedby,
                    "aria-invalid": invalid,
                    required,
                })}
                    <input
                        {id}
                        aria-describedby={describedby}
                        aria-invalid={invalid}
                        {required}
                        bind:value={applicationId}
                        disabled={busy}
                    />
                {/snippet}
            </Field>
            <Field label="Resource UUID" required>
                {#snippet children({
                    id,
                    "aria-describedby": describedby,
                    "aria-invalid": invalid,
                    required,
                })}
                    <input
                        {id}
                        aria-describedby={describedby}
                        aria-invalid={invalid}
                        {required}
                        bind:value={resourceId}
                        disabled={busy}
                    />
                {/snippet}
            </Field>
            <Field label="Resource kind">
                {#snippet children({
                    id,
                    "aria-describedby": describedby,
                    "aria-invalid": invalid,
                    required,
                })}
                    <select
                        {id}
                        aria-describedby={describedby}
                        aria-invalid={invalid}
                        {required}
                        bind:value={kind}
                        disabled={busy}
                    >
                        <option value={SelectionKind.Database}>Database</option>
                        <option value={SelectionKind.Filesystem}
                            >Filesystem</option
                        >
                    </select>
                {/snippet}
            </Field>
            <Field label="Resource-scoped Storage access token">
                {#snippet children({
                    id,
                    "aria-describedby": describedby,
                    "aria-invalid": invalid,
                    required,
                })}
                    <input
                        {id}
                        aria-describedby={describedby}
                        aria-invalid={invalid}
                        {required}
                        type="password"
                        bind:value={storageAccessToken}
                        autocomplete="off"
                        disabled={busy}
                    />
                {/snippet}
            </Field>
            <p>
                The Storage token is transient request data, not a saved
                credential or substitute for the Management User permission. It
                is cleared after each attempt.
            </p>
            <div class="flex gap-2">
                <button
                    class="btn btn-primary btn-md"
                    type="submit"
                    disabled={busy || !deviceId}
                    >Select authorized resource</button
                >
                <button
                    class="btn btn-secondary btn-md"
                    type="button"
                    onclick={() => updateSelection(false)}
                    disabled={busy ||
                        !deviceId ||
                        !applicationId ||
                        !resourceId}>Deselect exact resource</button
                >
            </div>
        </form>
        {#if kind === SelectionKind.Filesystem}
            <ResourceResidency filesystemId={resourceId} />
        {:else}
            <p>{residencyScopeText}</p>
        {/if}
        <p>{mutationAcknowledgementText}</p>
        {#if statusMessage}<Alert tone="info">{statusMessage}</Alert>{/if}
        {#if error}<Alert tone="danger">{error}</Alert>{/if}
    </div>
</Card>
