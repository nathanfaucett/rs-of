<script lang="ts">
import type { StorageClient } from "@aicacia/storage-client";
import { createStorageCollection } from "@aicacia/storage-db-collection";
import { useLiveQuery } from "@tanstack/svelte-db";
import { onMount, untrack } from "svelte";
import { SecretSchema } from "$lib/models/secret";

let { client, userId, filesystemId }: { client: StorageClient; userId: string; filesystemId: string } = $props();
const collection = untrack(() => createStorageCollection({ client, userId, applicationId: "password-manager.aicacia.com", filesystemId, directory: "secrets", schema: SecretSchema, getKey: (secret) => secret.id }));
const query = useLiveQuery((q) => q.from({ secret: collection }).orderBy(({ secret }) => secret.favorite, "desc").orderBy(({ secret }) => secret.name, "asc"));
let error = $state<string | null>(null);
let selectedId = $state<string | null>(null);
let editing = $derived(query.data.find((secret) => secret.id === selectedId));

onMount(() => {
    let active = true;
    void collection.preload().catch((cause: unknown) => {
        if (active) error = cause instanceof Error ? cause.message : "Storage load failed";
    });
    return () => { active = false; void query.collection?.cleanup(); void collection.cleanup(); };
});
</script>

{#if error || query.isError}
    <p role="alert">{error ?? 'Storage connection failed. Select the filesystem again to reopen it.'}</p>
{:else if !query.isReady}
    <p role="status">Loading secrets…</p>
{:else}
    <div class="grid gap-6 md:grid-cols-2">
        <section class="card">
            <h2>Secrets</h2>
            <button class="btn primary" type="button" disabled>New secret</button>
            {#if query.data.length === 0}
                <p>No secrets yet.</p>
            {:else}
                <ul>
                    {#each query.data as value (value.id)}
                        <li>
                            <button class="btn secondary" type="button" onclick={() => { selectedId = value.id; }}>{value.favorite ? '★ ' : ''}{value.name}</button>
                            <button class="btn danger" type="button" disabled>Delete</button>
                        </li>
                    {/each}
                </ul>
            {/if}
        </section>
        {#if editing}
            <section class="card flex flex-col gap-3">
                <h2>View secret</h2>
                <label for="name">Name</label><input id="name" value={editing.name} readonly />
                <label for="secret">Secret</label><input id="secret" value={editing.secret} type="password" readonly />
                <label for="uri">Website</label><input id="uri" value={editing.uris[0]?.uri ?? ''} readonly />
                <label for="notes">Notes</label><textarea id="notes" value={editing.notes ?? ''} readonly></textarea>
                <label for="favorite"><input id="favorite" checked={editing.favorite} type="checkbox" disabled /> Favorite</label>
                <button class="btn primary" type="button" disabled>Save</button>
                <button class="btn secondary" type="button" onclick={() => { selectedId = null; }}>Clear</button>
            </section>
        {/if}
    </div>
{/if}
