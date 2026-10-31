<script lang="ts">
import type { SqlStorage } from "$lib/storage/sql";
import { onDestroy, untrack } from "svelte";
import { Alert, Card, Field } from "$lib/ui";
import { createSecretWorkflow } from "./workflow.svelte";

let { client, databaseId }: { client: SqlStorage; databaseId: string } =
    $props();
const workflow = untrack(() => createSecretWorkflow({ client, databaseId }));
onDestroy(workflow.dispose);

let name = $derived(workflow.editor?.record?.name ?? "");
let secret = $derived(workflow.editor?.record?.secret ?? "");
let uri = $derived(workflow.editor?.record?.uris[0]?.uri ?? "");
let notes = $derived(workflow.editor?.record?.notes ?? "");
let favorite = $derived(workflow.editor?.record?.favorite ?? false);
</script>

<div class="flex flex-col gap-4">
    <Card
        title="Secrets"
        description="Refresh reloads Storage and closes unsaved form edits. No background synchronization."
    >
        {#snippet actions()}
            <button
                class="btn btn-secondary btn-md"
                type="button"
                disabled={!workflow.canRefresh}
                onclick={() => void workflow.refresh()}>Refresh</button
            >
            <button
                class="btn btn-primary btn-md"
                type="button"
                disabled={!workflow.canEdit}
                onclick={() => workflow.edit()}>New secret</button
            >
        {/snippet}

        {#if workflow.refreshing}
            <p role="status">Refreshing secrets…</p>
        {/if}

        {#if workflow.loadError}
            <Alert tone="danger" title="Storage unavailable">{workflow.loadError}</Alert>
        {:else if workflow.loading}
            <p role="status">Loading secrets…</p>
        {:else}
            {#if workflow.error}
                <Alert tone="danger" title="Action failed">{workflow.error}</Alert>
            {/if}
            {#if workflow.busy && !workflow.refreshing}
                <p role="status">Saving changes…</p>
            {:else if workflow.notice}
                <Alert tone="success">{workflow.notice}</Alert>
            {/if}

            {#if workflow.records.length === 0}
                <p class="mb-0">No secrets yet.</p>
            {:else}
                <ul class="flex list-none flex-col gap-2 p-0">
                    {#each workflow.records as value (value.id)}
                        <li
                            class="flex items-center justify-between gap-3 rounded-control border border-line bg-surface-muted/40 px-3 py-2"
                        >
                            <div class="flex min-w-0 flex-col">
                                <button
                                    class="btn btn-ghost btn-md w-full justify-start truncate px-0 text-left font-medium"
                                    type="button"
                                    disabled={!workflow.canEdit}
                                    onclick={() => workflow.edit(value)}
                                    ><span class="text-accent"
                                        >{value.favorite ? "★" : ""}</span
                                    >{value.favorite ? " " : ""}{value.name}</button
                                >
                                {#if value.uris[0]}
                                    <p class="truncate text-xs text-ink-subtle">
                                        {value.uris[0].uri}
                                    </p>
                                {/if}
                            </div>
                            <button
                                class="btn btn-ghost btn-md shrink-0 text-danger-text hover:bg-danger-soft"
                                type="button"
                                disabled={!workflow.canEdit}
                                onclick={() =>
                                    confirm(`Delete ${value.name}?`) &&
                                    void workflow.delete(value)}
                                >Delete</button
                            >
                        </li>
                    {/each}
                </ul>
            {/if}
        {/if}
    </Card>

    {#if workflow.editor}
        <Card
            title={workflow.editor.record ? 'Edit secret' : 'New secret'}
            description="Changes are written to Storage when you save."
        >
            <div class="flex flex-col gap-4">
                <Field id="name" label="Name" required>
                    {#snippet children(control)}
                        <input
                            {...control}
                            bind:value={name}
                            disabled={!workflow.canEdit}
                        />
                    {/snippet}
                </Field>

                <Field id="secret" label="Secret" required>
                    {#snippet children(control)}
                        <input
                            {...control}
                            bind:value={secret}
                            type="password"
                            disabled={!workflow.canEdit}
                        />
                    {/snippet}
                </Field>

                <Field
                    id="uri"
                    label="Website"
                    description="The first website associated with this secret."
                >
                    {#snippet children(control)}
                        <input
                            {...control}
                            bind:value={uri}
                            placeholder="https://example.com"
                            disabled={!workflow.canEdit}
                        />
                    {/snippet}
                </Field>

                <Field id="notes" label="Notes">
                    {#snippet children(control)}
                        <textarea
                            {...control}
                            bind:value={notes}
                            rows="3"
                            disabled={!workflow.canEdit}
                        ></textarea>
                    {/snippet}
                </Field>

                <label class="flex items-center gap-2 text-sm text-ink" for="favorite">
                    <input
                        id="favorite"
                        bind:checked={favorite}
                        type="checkbox"
                        disabled={!workflow.canEdit}
                    />
                    Favorite
                </label>

                <div class="flex gap-2">
                    <button
                        class="btn btn-primary btn-md"
                        type="button"
                        disabled={!workflow.canEdit}
                        onclick={() =>
                            void workflow.save({ name, secret, uri, notes, favorite })}
                        >Save</button
                    >
                    <button
                        class="btn btn-secondary btn-md"
                        type="button"
                        disabled={!workflow.canEdit}
                        onclick={() => workflow.cancel()}>Clear</button
                    >
                </div>
            </div>
        </Card>
    {/if}
</div>
