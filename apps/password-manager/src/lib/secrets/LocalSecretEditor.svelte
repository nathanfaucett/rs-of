<script lang="ts">
import { onDestroy, untrack } from "svelte";
import SecretDetail from "./SecretDetail.svelte";
import { Alert, Card, Field } from "$lib/ui";
import type { VaultHandle } from "$lib/vault/vault";
import { createLocalSecretWorkflow } from "./local-workflow.svelte";
import type { Secret } from "./secret";

let { vault }: { vault: VaultHandle } = $props();
const workflow = untrack(() => createLocalSecretWorkflow({ vault }));
onDestroy(workflow.dispose);


let pendingDelete = $state<Secret | null>(null);
const newSecretId = $props.id();

export function hasUnsavedChanges() { return workflow.dirty || workflow.busy; }
export function retire() { pendingDelete = null; workflow.dispose(); }

function navigate(action: () => void) {
    if (workflow.dirty && !window.confirm("Discard unsaved changes?")) return;
    pendingDelete = null;
    action();
}

</script>

<svelte:window onbeforeunload={(event) => {
    if (hasUnsavedChanges()) { event.preventDefault(); event.returnValue = ""; }
}} />

<div class="flex flex-col gap-4">
    <Card title="Secrets" description="Stored locally in your encrypted vault.">
        {#snippet actions()}
            <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canRefresh}
                onclick={() => navigate(() => void workflow.refresh())}>Refresh</button>
            <button id={newSecretId} class="btn btn-primary btn-md" type="button" disabled={!workflow.canEdit}
                onclick={() => navigate(() => workflow.edit())}>New secret</button>
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
                            {@attach (element) => { element.focus(); }}
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
                                    type="button" disabled={!workflow.canEdit} onclick={() => navigate(() => workflow.open(value.id))}>
                                    {value.favorite ? "★ " : ""}{value.name}
                                </button>
                                {#if (value.kind === "login" || value.kind === "identity") && value.username}<p class="truncate text-sm text-ink-subtle">{value.username}</p>{/if}
                                {#each value.kind === "login" ? value.uris : [] as uri (uri)}
                                    <p class="truncate text-xs text-ink-subtle">{uri.uri}</p>
                                {/each}
                            </div>
                            <button class="btn btn-ghost btn-md shrink-0 text-danger-text hover:bg-danger-soft"
                                type="button" disabled={!workflow.canEdit} onclick={() => navigate(() => { workflow.cancel(); pendingDelete = value; })}>Delete</button>
                        </li>
                    {/each}
                </ul>
            {/if}
        {/if}
    </Card>

    {#if workflow.detail && !workflow.editor}
        {#key workflow.detail}
            <SecretDetail record={workflow.detail} canEdit={workflow.canEdit}
                onedit={() => workflow.edit(workflow.detail)}
                onclose={() => { workflow.open(null); document.getElementById(newSecretId)?.focus(); }} />
        {/key}
    {/if}

    {#if workflow.editor}
        {@const editor = workflow.editor}
        {@const draft = editor.draft}
        <Card title={editor.record ? "Edit secret" : "New secret"}
            description="Changes are encrypted and stored when you save.">
            <form class="flex flex-col gap-4" onsubmit={(event) => {
                event.preventDefault();
                if (draft) void workflow.save(draft);
            }}>
                <Field id="name" label="Name" required>
                    {#snippet children(control)}
                        <input {...control} {@attach (element) => { element.focus(); }} bind:value={draft.name} disabled={!workflow.canEdit} />
                    {/snippet}
                </Field>
                <Field id="username" label="Username">
                    {#snippet children(control)}
                        <input {...control} bind:value={draft.username} autocomplete="off" disabled={!workflow.canEdit} />
                    {/snippet}
                </Field>
                <Field id="secret" label="Password">
                    {#snippet children(control)}
                        <input {...control} bind:value={draft.secret} type={editor?.showPassword ? "text" : "password"} autocomplete="new-password" disabled={!workflow.canEdit} />
                    {/snippet}
                </Field>
                <button class="btn btn-secondary btn-md self-start" type="button"
                    aria-controls="secret" aria-pressed={editor.showPassword}
                    onclick={() => {
                        if (editor) editor.showPassword = !editor.showPassword;
                    }}>{editor.showPassword ? "Hide password" : "Show password"}</button>
                {#if draft.secret && draft.secret !== editor.record?.secret}
                    <Field id="confirm-secret" label="Confirm password" required>
                        {#snippet children(control)}
                            <input {...control} bind:value={editor.confirmPassword} type="password" autocomplete="new-password" disabled={!workflow.canEdit} />
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

                <div class="flex gap-2">
                    <button class="btn btn-primary btn-md" type="submit" disabled={!workflow.canEdit}>Save</button>
                    <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit}
                        onclick={() => { workflow.cancel(); if (!workflow.detail) document.getElementById(newSecretId)?.focus(); }}>Cancel</button>
                </div>
            </form>
        </Card>
    {/if}
</div>
