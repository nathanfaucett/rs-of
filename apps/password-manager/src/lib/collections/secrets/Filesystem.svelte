<script lang="ts">
import type { StorageAccess } from "@of/sdk";
import {
    createStorageCollection,
    UnacknowledgedStorageWriteError,
} from "@aicacia/storage-db-collection";
import { useLiveQuery } from "@tanstack/svelte-db";
import { onMount, untrack } from "svelte";
import * as v from "valibot";
import { type Secret, SecretSchema } from "$lib/models/secret";

let {
    client,
    userId,
    filesystemId,
}: { client: StorageAccess; userId: string; filesystemId: string } = $props();
const collection = untrack(() =>
    createStorageCollection({
        client,
        userId,
        applicationId: "password-manager.aicacia.com",
        filesystemId,
        directory: "secrets",
        schema: SecretSchema,
        getKey: (secret) => secret.id,
    }),
);
const query = useLiveQuery((q) =>
    q
        .from({ secret: collection })
        .orderBy(({ secret }) => secret.favorite, "desc")
        .orderBy(({ secret }) => secret.name, "asc"),
);
let loadError = $state<string | null>(null);
let error = $state<string | null>(null);
let notice = $state("");
let busy = $state(false);
let refreshing = $state(false);
let editorOpen = $state(false);
let previous = $state.raw<Secret | null>(null);
let name = $state("");
let secret = $state("");
let uri = $state("");
let notes = $state("");
let favorite = $state(false);
let active = true;

onMount(() => {
    const unsubscribe = collection.on("status:change", ({ status }) => {
        if (active && status === "error")
            loadError =
                "Storage connection failed; a sent write may have persisted. Refresh and verify records before retrying.";
    });
    void collection.preload().catch((cause: unknown) => {
        if (active) loadError = message(cause);
    });
    return () => {
        active = false;
        unsubscribe();
        void query.collection?.cleanup();
        void collection.cleanup();
    };
});

async function refresh() {
    if (busy) return;
    busy = true;
    refreshing = true;
    error = null;
    notice = "";
    try {
        await collection.utils.refresh();
        if (!active) return;
        previous = null;
        editorOpen = false;
        loadError = null;
        notice = "Refreshed. Authoritative records reloaded.";
    } catch (cause) {
        if (active) loadError = message(cause);
    } finally {
        if (active) {
            busy = false;
            refreshing = false;
        }
    }
}
function edit(value: Secret | null) {
    previous = value ? v.parse(SecretSchema, value) : null;
    name = value?.name ?? "";
    secret = value?.secret ?? "";
    uri = value?.uris[0]?.uri ?? "";
    notes = value?.notes ?? "";
    favorite = value?.favorite ?? false;
    editorOpen = true;
    error = null;
    notice = "";
}
async function save() {
    if (busy) return;
    error = null;
    notice = "";
    const now = new Date().toISOString();
    const uris = previous?.uris.slice() ?? [];
    if (uri)
        uris.splice(0, uris.length ? 1 : 0, {
            uri,
            match: uris[0]?.match ?? "domain",
        });
    else if (uris.length) uris.shift();
    const result = v.safeParse(SecretSchema, {
        ...previous,
        id: previous?.id ?? crypto.randomUUID(),
        name,
        secret,
        uris,
        notes:
            notes === (previous?.notes ?? "")
                ? previous?.notes
                : notes || undefined,
        favorite,
        fields: previous?.fields ?? [],
        history:
            previous && previous.secret !== secret
                ? [
                      ...previous.history,
                      { secret: previous.secret, changedAt: now },
                  ]
                : (previous?.history ?? []),
        createdAt: previous?.createdAt ?? now,
        updatedAt: now,
    });
    if (!result.success) {
        error =
            "Invalid secret: Name and Secret are required; check the fields.";
        return;
    }
    busy = true;
    try {
        const value = result.output;
        const tx = previous
            ? collection.update(value.id, (draft) => {
                  Object.assign(draft, value);
              })
            : collection.insert(value);
        await tx.isPersisted.promise;
        if (!active) return;
        const authoritative = collection.get(value.id);
        if (authoritative) edit(authoritative);
        else {
            previous = null;
            editorOpen = false;
        }
        notice = "Saved. Authoritative records reloaded.";
    } catch (cause) {
        if (active) {
            error = message(cause);
            if (cause instanceof UnacknowledgedStorageWriteError) {
                previous = null;
                editorOpen = false;
            }
        }
    } finally {
        if (active) busy = false;
    }
}
async function remove(value: Secret) {
    if (busy || !confirm(`Delete ${value.name}?`)) return;
    error = null;
    notice = "";
    busy = true;
    try {
        await collection.delete(value.id).isPersisted.promise;
        if (!active) return;
        if (previous?.id === value.id) {
            previous = null;
            editorOpen = false;
        }
        notice = "Deleted. Authoritative records reloaded.";
    } catch (cause) {
        if (active) {
            error = message(cause);
            if (
                cause instanceof UnacknowledgedStorageWriteError &&
                previous?.id === value.id
            ) {
                previous = null;
                editorOpen = false;
            }
        }
    } finally {
        if (active) busy = false;
    }
}
function message(cause: unknown): string {
    return cause instanceof Error ? cause.message : "Storage request failed";
}
</script>

<button class="btn secondary" type="button" disabled={busy || (!query.isReady && !query.isError && !loadError)} onclick={() => void refresh()}>Refresh</button>
<p>Refresh reloads Storage and closes unsaved form edits. No background synchronization.</p>
{#if refreshing}<p role="status">Refreshing secrets…</p>{/if}
{#if loadError || query.isError}
    <p role="alert">{loadError ?? 'Storage connection failed. Refresh and verify records before retrying.'}</p>
{:else if !query.isReady}
    <p role="status">Loading secrets…</p>
{:else}
    {#if error}<p role="alert">{error}</p>{/if}
    {#if busy && !refreshing}<p role="status">Saving changes…</p>{:else if notice}<p role="status">{notice}</p>{/if}
    <div class="grid gap-6 md:grid-cols-2">
        <section class="card">
            <h2>Secrets</h2>
            <button class="btn primary" type="button" disabled={busy} onclick={() => edit(null)}>New secret</button>
            {#if query.data.length === 0}
                <p>No secrets yet.</p>
            {:else}
                <ul>
                    {#each query.data as value (value.id)}
                        <li>
                            <button class="btn secondary" type="button" disabled={busy} onclick={() => edit(value)}>{value.favorite ? '★ ' : ''}{value.name}</button>
                            <button class="btn danger" type="button" disabled={busy} onclick={() => void remove(value)}>Delete</button>
                        </li>
                    {/each}
                </ul>
            {/if}
        </section>
        {#if editorOpen}
            <section class="card flex flex-col gap-3">
                <h2>{previous ? 'Edit secret' : 'New secret'}</h2>
                <label for="name">Name</label><input id="name" bind:value={name} disabled={busy} />
                <label for="secret">Secret</label><input id="secret" bind:value={secret} type="password" disabled={busy} />
                <label for="uri">Website</label><input id="uri" bind:value={uri} disabled={busy} />
                <label for="notes">Notes</label><textarea id="notes" bind:value={notes} disabled={busy}></textarea>
                <label for="favorite"><input id="favorite" bind:checked={favorite} type="checkbox" disabled={busy} /> Favorite</label>
                <button class="btn primary" type="button" disabled={busy} onclick={() => void save()}>Save</button>
                <button class="btn secondary" type="button" disabled={busy} onclick={() => { previous = null; editorOpen = false; error = null; notice = ''; }}>Clear</button>
            </section>
        {/if}
    </div>
{/if}
