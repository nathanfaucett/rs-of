<script lang="ts">
import type { StorageAccess } from "@of/sdk";
import { onDestroy, untrack } from "svelte";
import { createSecretWorkflow } from "./workflow.svelte";

let {
    client,
    userId,
    filesystemId,
}: { client: StorageAccess; userId: string; filesystemId: string } = $props();
const workflow = untrack(() => createSecretWorkflow({ client, userId, filesystemId }));
onDestroy(workflow.dispose);

let name = $derived(workflow.editor?.record?.name ?? "");
let secret = $derived(workflow.editor?.record?.secret ?? "");
let uri = $derived(workflow.editor?.record?.uris[0]?.uri ?? "");
let notes = $derived(workflow.editor?.record?.notes ?? "");
let favorite = $derived(workflow.editor?.record?.favorite ?? false);
</script>

<button class="btn secondary" type="button" disabled={!workflow.canRefresh} onclick={() => void workflow.refresh()}>Refresh</button>
<p>Refresh reloads Storage and closes unsaved form edits. No background synchronization.</p>
{#if workflow.refreshing}<p role="status">Refreshing secrets…</p>{/if}
{#if workflow.loadError}
    <p role="alert">{workflow.loadError}</p>
{:else if workflow.loading}
    <p role="status">Loading secrets…</p>
{:else}
    {#if workflow.error}<p role="alert">{workflow.error}</p>{/if}
    {#if workflow.busy && !workflow.refreshing}<p role="status">Saving changes…</p>{:else if workflow.notice}<p role="status">{workflow.notice}</p>{/if}
    <div class="grid gap-6 md:grid-cols-2">
        <section class="card">
            <h2>Secrets</h2>
            <button class="btn primary" type="button" disabled={!workflow.canEdit} onclick={() => workflow.edit()}>New secret</button>
            {#if workflow.records.length === 0}
                <p>No secrets yet.</p>
            {:else}
                <ul>
                    {#each workflow.records as value (value.id)}
                        <li>
                            <button class="btn secondary" type="button" disabled={!workflow.canEdit} onclick={() => workflow.edit(value)}>{value.favorite ? '★ ' : ''}{value.name}</button>
                            <button class="btn danger" type="button" disabled={!workflow.canEdit} onclick={() => confirm(`Delete ${value.name}?`) && void workflow.delete(value)}>Delete</button>
                        </li>
                    {/each}
                </ul>
            {/if}
        </section>
        {#if workflow.editor}
            <section class="card flex flex-col gap-3">
                <h2>{workflow.editor.record ? 'Edit secret' : 'New secret'}</h2>
                <label for="name">Name</label><input id="name" bind:value={name} disabled={!workflow.canEdit} />
                <label for="secret">Secret</label><input id="secret" bind:value={secret} type="password" disabled={!workflow.canEdit} />
                <label for="uri">Website</label><input id="uri" bind:value={uri} disabled={!workflow.canEdit} />
                <label for="notes">Notes</label><textarea id="notes" bind:value={notes} disabled={!workflow.canEdit}></textarea>
                <label for="favorite"><input id="favorite" bind:checked={favorite} type="checkbox" disabled={!workflow.canEdit} /> Favorite</label>
                <button class="btn primary" type="button" disabled={!workflow.canEdit} onclick={() => void workflow.save({ name, secret, uri, notes, favorite })}>Save</button>
                <button class="btn secondary" type="button" disabled={!workflow.canEdit} onclick={() => workflow.cancel()}>Clear</button>
            </section>
        {/if}
    </div>
{/if}
