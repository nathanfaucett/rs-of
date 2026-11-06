<script lang="ts">
    import { onMount } from "svelte";
    import { checkStorageSyncCapability } from "../state/deviceClient.svelte";
    import {
        directions, ownerBlocker, saveSyncTarget, syncContext, syncTarget,
        validateSyncTarget, type SyncDirection,
    } from "../state/sync.svelte";
    import { Alert, Card } from "$lib/ui";

    let busy = $state(true);
    let setup = $state(false);
    let url = $state("");
    let error = $state("");
    let message = $state("");
    let direction = $state<SyncDirection>("Push");
    let selected = $state<string[]>([]);
    let context = $state.raw<Awaited<ReturnType<typeof syncContext>>>();
    let discovery = $state.raw<Awaited<ReturnType<Awaited<ReturnType<typeof syncContext>>["discover"]>>>();
    let cancellation: AbortController | undefined;

    onMount(() => {
        let active = true;
        void syncContext().then((value) => {
            if (!active) return;
            context = value;
            direction = value.stored.item.direction;
            selected = value.stored.item.deviceIds;
        }).catch(() => {
            if (active) error = "Sign in before checking sync.";
        }).finally(() => { if (active) busy = false; });
        return () => { active = false; cancellation?.abort(); };
    });

    async function check(save = false) {
        if (busy) return;
        busy = true;
        error = "";
        message = "Checking service capability…";
        discovery = undefined;
        cancellation?.abort();
        const controller = new AbortController();
        cancellation = controller;
        const timeout = setTimeout(() => controller.abort(), 15000);
        try {
            const destination = save ? validateSyncTarget(url) : syncTarget();
            if (!destination) {
                setup = true;
                message = "No sync service is configured.";
                return;
            }
            validateSyncTarget(destination);
            if (!await checkStorageSyncCapability(destination, controller.signal)) {
                setup = true;
                message = "The service is unreachable or does not advertise sync. No data was transferred.";
                return;
            }
            controller.signal.throwIfAborted();
            if (save) saveSyncTarget(destination);
            context = await syncContext();
            message = "Loading current-Application resources and trusted Devices…";
            discovery = await context.discover(controller.signal);
            selected = context.stored.item.deviceIds.filter((id) => discovery?.devices.some((device) => device.id === id));
            direction = context.stored.item.direction;
            setup = false;
            message = "Service capability found. Transfer is blocked; this is not a backup.";
        } catch {
            error = "Sync check failed. Use an HTTPS Storage API URL without credentials, query, or fragment. Check your session, access, and service connection. No data was transferred.";
            message = "";
        } finally {
            clearTimeout(timeout);
            if (cancellation === controller) busy = false;
        }
    }
    async function checkTargets() {
        if (busy || !context || !discovery) return;
        busy = true;
        error = "";
        const controller = new AbortController();
        cancellation = controller;
        const timeout = setTimeout(() => controller.abort(), 15000);
        try {
            context.assertCurrent();
            const current = await context.discover(controller.signal);
            const allowed = new Set(current.devices.map((device) => device.id));
            const retained = selected.filter((id) => allowed.has(id));
            discovery = current;
            selected = retained;
            if (!retained.length) throw new Error("No approved targets");
            const results = { ...context.stored.item.results };
            for (const id of retained) results[id] = {
                checkedAt: new Date().toISOString(), direction, status: "blocked",
            };
            context.stored.item = { deviceIds: retained, direction, results };
            message = `Blocked — no data transferred. ${ownerBlocker}`;
        } catch {
            error = "Targets or access changed. Check sync again. No data was transferred.";
        } finally {
            clearTimeout(timeout);
            busy = false;
        }
    }
</script>

<Card title="Sync">
    <p>{ownerBlocker}</p>
    <p>Push, Pull, and Bidirectional transfers are unavailable. A capability check or connection is not backup success.</p>
    <div class="flex gap-2">
        <button class="btn btn-primary btn-md" type="button" disabled={busy || !context} onclick={() => check()}>Sync</button>
        <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={() => { setup = true; url = syncTarget() ?? ""; }}>Configure sync</button>
    </div>
    {#if busy}<p role="status">{message || "Checking session…"}</p>{/if}
    {#if setup}
        <section aria-label="Set up sync" class="flex flex-col gap-3">
            <h3>Set up sync</h3>
            <label>Sync Storage API URL <input type="url" bind:value={url} disabled={busy} placeholder="https://node.example/storage" /></label>
            <p>The URL check sends no token. Resource discovery uses your existing source Storage API, not this override.</p>
            <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={() => check(true)}>Save and check</button>
            <button class="btn btn-secondary btn-md" type="button" disabled={busy} onclick={() => { saveSyncTarget(null); url = syncTarget() ?? ""; context = undefined; void syncContext().then((value) => { context = value; }).catch(() => { error = "Sign in before checking sync."; }); }}>Use configured Storage API</button>
            <a href="#device-pairing">Pair a Device</a>
            <p>Pairing does not grant resource access or restore a backup.</p>
        </section>
    {/if}
    {#if discovery}
        <h3>Resources in your signed-in Application namespace</h3>
        <p>These are source catalog records, not proof that this browser has a local copy.</p>
        <ul>
            {#each discovery.resources as resource (`${resource.kind}:${resource.id}`)}
                <li>{resource.kind}: {resource.name || resource.id} — {resource.id}</li>
            {:else}<li>No resources found.</li>{/each}
        </ul>
        <p>Resource Selections are unknown: the User API has no selection-read route. Saved choices below are local preferences, not policy grants.</p>
        <fieldset disabled={busy}>
            <legend>Trusted Device preferences</legend>
            {#each discovery.devices as device (device.id)}
                <label class="block"><input type="checkbox" bind:group={selected} value={device.id} /> {device.name}</label>
            {:else}<p>No approved Devices available.</p>{/each}
            <label>Direction <select bind:value={direction}>{#each directions as value (value)}<option>{value}</option>{/each}</select></label>
            <button class="btn btn-secondary btn-md" type="button" disabled={!selected.length} onclick={checkTargets}>Check selected targets</button>
        </fieldset>
    {/if}
    {#if context && Object.keys(context.stored.item.results).length}
        <h3>Last local checks — not last successful sync</h3>
        <ul>
            {#each Object.entries(context.stored.item.results) as [id, result] (id)}
                <li>{id}: Blocked — no data transferred. {result.direction}; {result.checkedAt}. Current trust and policy are not inferred.</li>
            {/each}
        </ul>
    {/if}
    {#if message && !busy}<p role="status">{message}</p>{/if}
    {#if error}<Alert tone="danger">{error}</Alert>{/if}
</Card>
