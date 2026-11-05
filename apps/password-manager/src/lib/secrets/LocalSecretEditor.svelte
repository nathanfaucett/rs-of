<script lang="ts">
import { onDestroy, untrack } from "svelte";
import { Alert, Card, Field } from "$lib/ui";
import type { VaultHandle } from "$lib/vault/vault";
import { createLocalSecretWorkflow } from "./local-workflow.svelte";
import type { Secret } from "./secret";

let { vault }: { vault: VaultHandle } = $props();
const workflow = untrack(() => createLocalSecretWorkflow({ vault }));
onDestroy(workflow.dispose);

let draft = $derived(workflow.editor?.draft);
let pendingDelete = $state<Secret | null>(null);
</script>

<div class="flex flex-col gap-4">
    <Card title="Secrets" description="Stored locally in your encrypted vault.">
        {#snippet actions()}
            <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canRefresh}
                onclick={() => void workflow.refresh()}>Refresh</button>
            <button class="btn btn-primary btn-md" type="button" disabled={!workflow.canEdit}
                onclick={() => workflow.edit()}>New secret</button>
        {/snippet}

        {#if workflow.refreshing}
            <p role="status">Refreshing secrets…</p>
        {/if}
        {#if workflow.loadError}
            <Alert tone="danger" title="Vault unavailable">{workflow.loadError}</Alert>
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
            {#if pendingDelete}
                <div class="flex flex-col gap-2 rounded-control border border-line p-3">
                    <p>Delete {pendingDelete.name}? This cannot be undone.</p>
                    <div class="flex gap-2">
                        <button class="btn btn-primary btn-md" type="button" disabled={!workflow.canEdit}
                            onclick={() => {
                                const value = pendingDelete;
                                pendingDelete = null;
                                if (value) void workflow.delete(value);
                            }}>Confirm delete</button>
                        <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit}
                            onclick={() => pendingDelete = null}>Cancel delete</button>
                    </div>
                </div>
            {/if}
            {#if workflow.records.length === 0}
                <p class="mb-0">No secrets yet.</p>
            {:else}
                <ul class="flex list-none flex-col gap-2 p-0">
                    {#each workflow.records as value (value.id)}
                        <li class="flex items-center justify-between gap-3 rounded-control border border-line bg-surface-muted/40 px-3 py-2">
                            <div class="flex min-w-0 flex-col">
                                <button class="btn btn-ghost btn-md w-full justify-start truncate px-0 text-left font-medium"
                                    type="button" disabled={!workflow.canEdit || value.kind !== "login"} onclick={() => workflow.edit(value)}>
                                    {value.favorite ? "★ " : ""}{value.name}
                                </button>
                                {#if (value.kind === "login" || value.kind === "identity") && value.username}<p class="truncate text-sm text-ink-subtle">{value.username}</p>{/if}
                                {#each value.kind === "login" ? value.uris : [] as uri (uri)}
                                    <p class="truncate text-xs text-ink-subtle">{uri.uri}</p>
                                {/each}
                            </div>
                            <button class="btn btn-ghost btn-md shrink-0 text-danger-text hover:bg-danger-soft"
                                type="button" disabled={!workflow.canEdit} onclick={() => pendingDelete = value}>Delete</button>
                        </li>
                    {/each}
                </ul>
            {/if}
        {/if}
    </Card>

    {#if workflow.editor && draft}
        <Card title={workflow.editor.record ? "Edit secret" : "New secret"}
            description="Changes are encrypted and stored when you save.">
            <form class="flex flex-col gap-4" onsubmit={(event) => {
                event.preventDefault();
                if (draft) void workflow.save(draft);
            }}>
                <Field id="name" label="Name" required>
                    {#snippet children(control)}
                        <input {...control} bind:value={draft.name} disabled={!workflow.canEdit} />
                    {/snippet}
                </Field>
                <Field id="username" label="Username">
                    {#snippet children(control)}
                        <input {...control} bind:value={draft.username} autocomplete="off" disabled={!workflow.canEdit} />
                    {/snippet}
                </Field>
                <Field id="secret" label="Password">
                    {#snippet children(control)}
                        <input {...control} bind:value={draft.secret} type={workflow.editor?.showPassword ? "text" : "password"} autocomplete="new-password" disabled={!workflow.canEdit} />
                    {/snippet}
                </Field>
                <button class="btn btn-secondary btn-md self-start" type="button"
                    aria-controls="secret" aria-pressed={workflow.editor.showPassword}
                    onclick={() => {
                        if (workflow.editor) workflow.editor.showPassword = !workflow.editor.showPassword;
                    }}>{workflow.editor.showPassword ? "Hide password" : "Show password"}</button>
                {#if draft.secret && draft.secret !== workflow.editor.record?.secret}
                    <Field id="confirm-secret" label="Confirm password" required>
                        {#snippet children(control)}
                            <input {...control} bind:value={workflow.editor.confirmPassword} type="password" autocomplete="new-password" disabled={!workflow.canEdit} />
                        {/snippet}
                    </Field>
                {/if}
                <Field id="totp" label="Authenticator key (TOTP)" description="Store a key or otpauth URI. Code generation is not available.">
                    {#snippet children(control)}
                        <input {...control} bind:value={draft.totp} type="password" autocomplete="off" disabled={!workflow.canEdit} />
                    {/snippet}
                </Field>

                <fieldset class="flex min-w-0 flex-col gap-3" disabled={!workflow.canEdit}>
                    <legend class="text-sm font-medium">Websites</legend>
                    {#each draft.uris as uri, i (uri)}
                        <div class="flex flex-wrap items-end gap-2">
                            <Field id={`uri-${i}`} label={`Website ${i + 1}`}>
                                {#snippet children(control)}
                                    <input {...control} bind:value={uri.uri} placeholder="https://example.com" />
                                {/snippet}
                            </Field>
                            <Field id={`match-${i}`} label={`Match rule ${i + 1}`}>
                                {#snippet children(control)}
                                    <select {...control} bind:value={uri.match}>
                                        <option value={null}>Default (inherited)</option>
                                        <option value="domain">Domain</option>
                                        <option value="host">Host</option>
                                        <option value="startsWith">Starts with</option>
                                        <option value="exact">Exact</option>
                                        <option value="regex">Regular expression</option>
                                        <option value="never">Never</option>
                                    </select>
                                {/snippet}
                            </Field>
                            <button class="btn btn-ghost btn-md text-danger-text" type="button" aria-label={`Remove website ${i + 1}`}
                                onclick={() => draft?.uris.splice(i, 1)}>Remove</button>
                        </div>
                    {/each}
                    <button class="btn btn-secondary btn-md self-start" type="button"
                        onclick={() => draft?.uris.push({ uri: "", match: "domain" })}>Add website</button>
                </fieldset>

                <fieldset class="flex min-w-0 flex-col gap-3" disabled={!workflow.canEdit}>
                    <legend class="text-sm font-medium">Custom fields</legend>
                    {#each draft.fields as field, i (field)}
                        <div class="flex flex-wrap items-end gap-2">
                            <Field id={`field-name-${i}`} label={`Field name ${i + 1}`} required>
                                {#snippet children(control)}<input {...control} bind:value={field.name} />{/snippet}
                            </Field>
                            <Field id={`field-type-${i}`} label={`Field type ${i + 1}`}>
                                {#snippet children(control)}
                                    <select {...control} bind:value={field.type}>
                                        <option value="text">Text</option>
                                        <option value="hidden">Hidden</option>
                                        <option value="boolean">Boolean</option>
                                    </select>
                                {/snippet}
                            </Field>
                            <Field id={`field-value-${i}`} label={`Field value ${i + 1}`}>
                                {#snippet children(control)}
                                    {#if field.type === "boolean"}
                                        <select {...control} bind:value={field.value}>
                                            <option value="">Not set</option>
                                            <option value="true">True</option>
                                            <option value="false">False</option>
                                        </select>
                                    {:else}
                                        <input {...control} bind:value={field.value} type={field.type === "hidden" ? "password" : "text"} autocomplete="off" />
                                    {/if}
                                {/snippet}
                            </Field>
                            <button class="btn btn-ghost btn-md text-danger-text" type="button" aria-label={`Remove custom field ${i + 1}`}
                                onclick={() => draft?.fields.splice(i, 1)}>Remove</button>
                        </div>
                    {/each}
                    <button class="btn btn-secondary btn-md self-start" type="button"
                        onclick={() => draft?.fields.push({ name: "", value: "", type: "text" })}>Add custom field</button>
                </fieldset>

                <Field id="notes" label="Notes">
                    {#snippet children(control)}
                        <textarea {...control} bind:value={draft.notes} rows="3" disabled={!workflow.canEdit}></textarea>
                    {/snippet}
                </Field>
                <label class="flex items-center gap-2 text-sm text-ink" for="favorite">
                    <input id="favorite" bind:checked={draft.favorite} type="checkbox" disabled={!workflow.canEdit} />
                    Favorite
                </label>
                {#if workflow.editor.record?.history.length}
                    <details>
                        <summary>Password history ({workflow.editor.record.history.length})</summary>
                        <ul class="flex list-none flex-col gap-2 p-0">
                            {#each workflow.editor.record.history as entry, i (entry)}
                                <li><Field id={`history-${i}`} label={`Password changed at ${entry.changedAt}`}>
                                    {#snippet children(control)}<input {...control} type="password" value={entry.secret} readonly />{/snippet}
                                </Field></li>
                            {/each}
                        </ul>
                    </details>
                {/if}
                <div class="flex gap-2">
                    <button class="btn btn-primary btn-md" type="submit" disabled={!workflow.canEdit}>Save</button>
                    <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit}
                        onclick={() => workflow.cancel()}>Cancel</button>
                </div>
            </form>
        </Card>
    {/if}
</div>
