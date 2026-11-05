<script lang="ts">
import { onDestroy, tick, untrack } from "svelte";
import SecretDetail from "./SecretDetail.svelte";
import CardFields from "./CardFields.svelte";
import IdentityFields from "./IdentityFields.svelte";
import FolderControls from "./FolderControls.svelte";
import PasswordGenerator from "$lib/generator/PasswordGenerator.svelte";
import { Alert, Card, Field } from "$lib/ui";
import type { VaultHandle } from "$lib/vault/vault";
import { createLocalSecretWorkflow } from "./local-workflow.svelte";
import type { Secret } from "./secret";
import { searchItems, type ItemTypeFilter, type ItemSort } from "./search";

let { vault }: { vault: VaultHandle } = $props();
const workflow = untrack(() => createLocalSecretWorkflow({ vault }));
onDestroy(retire);

let retired = $state(false);
let searchText = $state("");
let itemType = $state<ItemTypeFilter>("all");
let favoritesOnly = $state(false);
let sort = $state<ItemSort>("favorites");
let results = $derived(retired ? [] : searchItems(workflow.records, {
    text: searchText, kind: itemType, favoritesOnly, sort,
}));


function clearFilters() {
    searchText = "";
    itemType = "all";
    favoritesOnly = false;
}


let pendingDelete = $state<Secret | null>(null);
let trash = $state(false);
let deleteTrigger: HTMLButtonElement | null = null;
const uid = $props.id();
const confirmationId = `${uid}-confirmation`;
const navigationId = `${uid}-navigation`;
async function cancelDelete() {
    const trigger = deleteTrigger;
    pendingDelete = null;
    deleteTrigger = null;
    await tick();
    trigger?.focus();
}
let showGenerator = $state(false);
const newSecretId = `${uid}-new`;
const records = $derived(trash ? workflow.trashRecords : results);
const searchId = `${newSecretId}-search`;
let folderControls = $state<ReturnType<typeof FolderControls>>();

export function hasUnsavedChanges() { return workflow.dirty || workflow.busy || !!folderControls?.dirty(); }
export function retire() {
    showGenerator = false;
    retired = true;
    clearFilters();
    sort = "favorites";
    results = [];
    pendingDelete = null;
    deleteTrigger = null;
    trash = false;
    folderControls?.retire();
    workflow.dispose();
}

function navigate(action: () => void) {
    if ((workflow.dirty || folderControls?.dirty()) && !window.confirm("Discard unsaved changes?")) return;
    showGenerator = false;
    pendingDelete = null;
    folderControls?.retire();
    action();
}

</script>

<svelte:window onbeforeunload={(event) => {
    if (hasUnsavedChanges()) { event.preventDefault(); event.returnValue = ""; }
}} />

<div class="flex flex-col gap-4">
    <nav aria-label="Item views" class="flex gap-2">
        <button id={navigationId} class="btn btn-secondary btn-md" type="button" aria-pressed={!trash} disabled={!workflow.canEdit}
            onclick={() => navigate(() => { workflow.open(null); trash = false; })}>All items</button>
        <button class="btn btn-secondary btn-md" type="button" aria-pressed={trash} disabled={!workflow.canEdit}
            onclick={() => navigate(() => { workflow.open(null); trash = true; })}>Trash</button>
    </nav>
    <Card title={trash ? "Trash" : "Secrets"} description={trash ? "Items stay here until you restore or permanently delete them. No automatic deletion." : "Stored locally in your encrypted vault."}>
        {#snippet actions()}
            <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit}
                aria-expanded={showGenerator} onclick={() => showGenerator = !showGenerator}>Password generator</button>
            <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canRefresh}
                onclick={() => navigate(() => void workflow.refresh())}>Refresh</button>
            <button id={newSecretId} class="btn btn-primary btn-md" type="button" disabled={!workflow.canEdit}
                onclick={() => navigate(() => { trash = false; workflow.edit(); })}>New secret</button>
            <button class="btn btn-primary btn-md" type="button" disabled={!workflow.canEdit}
                onclick={() => navigate(() => { trash = false; workflow.edit(null, "secureNote"); })}>New secure note</button>
            <button class="btn btn-primary btn-md" type="button" disabled={!workflow.canEdit}
                onclick={() => navigate(() => { trash = false; workflow.edit(null, "card"); })}>New card</button>
                            <button class="btn btn-primary btn-md" type="button" disabled={!workflow.canEdit}
                                onclick={() => navigate(() => { trash = false; workflow.edit(null, "identity"); })}>New identity</button>
        {/snippet}

        {#if !retired && !trash}
            <fieldset class="mb-4 flex flex-wrap items-end gap-3" disabled={workflow.loading || !!workflow.loadError}>
                <legend class="text-sm font-medium">Find items</legend>
                <Field id={`${searchId}-text`} label="Search items" description="Search names, usernames, websites, notes, and custom-field names.">
                    {#snippet children(control)}
                        <input {...control} type="search" autocomplete="off" spellcheck="false"
                            bind:value={() => searchText, (value) => { if (!retired) searchText = value; }} />
                    {/snippet}
                </Field>
                <Field id={`${searchId}-type`} label="Item type">
                    {#snippet children(control)}
                        <select {...control} bind:value={itemType}>
                            <option value="all">All types</option>
                            <option value="login">Login</option>
                            <option value="secureNote">Secure Note</option>
                            <option value="card">Card</option>
                            <option value="identity">Identity</option>
                            <option value="sshKey">SSH Key</option>
                        </select>
                    {/snippet}
                </Field>
                <label class="flex items-center gap-2 text-sm text-ink">
                    <input type="checkbox" bind:checked={favoritesOnly} /> Favorites only
                </label>
                <Field id={`${searchId}-sort`} label="Sort items">
                    {#snippet children(control)}
                        <select {...control} bind:value={sort}>
                            <option value="favorites">Favorites first, then name</option>
                            <option value="nameAsc">Name (A–Z)</option>
                            <option value="nameDesc">Name (Z–A)</option>
                            <option value="updatedDesc">Recently updated</option>
                            <option value="createdDesc">Recently created</option>
                        </select>
                    {/snippet}
                </Field>
                <button class="btn btn-secondary btn-md" type="button" onclick={clearFilters}>Clear filters</button>
            </fieldset>
        {/if}
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
            {#if !trash}<FolderControls bind:this={folderControls} {workflow} {navigate} />{/if}
            {#if pendingDelete}
                <dialog role="alertdialog" aria-labelledby={confirmationId} oncancel={(event) => { event.preventDefault(); cancelDelete(); }}
                    {@attach (element) => { element.showModal(); element.querySelector<HTMLButtonElement>('[data-cancel]')?.focus(); }}
                    class="rounded-control border border-line bg-surface p-4 text-ink">
                    <p id={confirmationId}>{pendingDelete.deletedAt ? `Permanently delete ${pendingDelete.name}? This cannot be undone. Existing backups can still contain this item.` : `Move ${pendingDelete.name} to Trash? You can restore it later.`}</p>
                    <div class="flex gap-2">
                        <button class="btn btn-primary btn-md" type="button" disabled={!workflow.canEdit}
                            onclick={async () => {
                                const value = pendingDelete;
                                pendingDelete = null;
                                deleteTrigger = null;
                                if (value) {
                                    if (value.deletedAt) await workflow.deletePermanently(value);
                                    else await workflow.delete(value);
                                }
                                document.getElementById(navigationId)?.focus();
                            }}>{pendingDelete.deletedAt ? "Confirm permanent delete" : "Confirm delete"}</button>
                        <button data-cancel class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit}
                            onclick={cancelDelete}>Cancel delete</button>
                    </div>
                </dialog>
            {/if}
            {#if records.length === 0}
                <p class="mb-0" role="status">{trash ? "Trash is empty." : searchText.trim() || itemType !== "all" || favoritesOnly ? "No items match. Clear filters to show all items." : workflow.folderFilter === "all" ? "No secrets yet." : "No items in this folder view."}</p>
            {:else}
                <p aria-live="polite" aria-atomic="true">{records.length} {records.length === 1 ? "item" : "items"} found.</p>
                <ul aria-label="Search results" class="flex list-none flex-col gap-2 p-0">
                    {#each records as value (value.id)}
                        <li class="flex items-center justify-between gap-3 rounded-control border border-line bg-surface-muted/40 px-3 py-2">
                            <div class="flex min-w-0 flex-col">
                                {#if trash}
                                    <span class="font-medium">{value.name}</span>
                                    <span class="text-sm text-ink-subtle">Deleted: <time datetime={value.deletedAt}>{value.deletedAt}</time></span>
                                {:else}
                                <button class="btn btn-ghost btn-md w-full justify-start truncate px-0 text-left font-medium"
                                    type="button" disabled={!workflow.canEdit} onclick={() => navigate(() => workflow.open(value.id))}>
                                    {value.favorite ? "★ " : ""}{value.name}
                                </button>
                                {#if (value.kind === "login" || value.kind === "identity") && value.username}<p class="truncate text-sm text-ink-subtle">{value.username}</p>{/if}
                                {#each value.kind === "login" ? value.uris : [] as uri (uri)}
                                    <p class="truncate text-xs text-ink-subtle">{uri.uri}</p>
                                {/each}
                                {/if}
                            </div>
                            {#if trash}
                                <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit}
                                    onclick={async () => { await workflow.restore(value); document.getElementById(navigationId)?.focus(); }}>Restore</button>
                            {/if}
                            <button class="btn btn-ghost btn-md shrink-0 text-danger-text hover:bg-danger-soft"
                                type="button" disabled={!workflow.canEdit} onclick={(event) => navigate(() => { workflow.open(null); deleteTrigger = event.currentTarget; pendingDelete = value; })}>{trash ? "Delete permanently" : "Delete"}</button>
                        </li>
                    {/each}
                </ul>
            {/if}
        {/if}
    </Card>

    {#if showGenerator && workflow.canEdit}
        {#key workflow.editor ?? workflow.detail}
            {@const editor = workflow.editor}
            <PasswordGenerator onuse={editor?.draft.kind === "login" ? (value) => {
                if (workflow.editor !== editor || !workflow.canEdit || editor.draft.kind !== "login") return;
                editor.draft.secret = value;
                editor.confirmPassword = value;
                editor.showPassword = false;
                showGenerator = false;
                document.getElementById("secret")?.focus();
            } : undefined} />
        {/key}
    {/if}

    {#if workflow.detail && !workflow.editor}
        {#key workflow.detail}
            <SecretDetail record={workflow.detail} canEdit={workflow.canEdit}
                onedit={() => navigate(() => workflow.edit(workflow.detail))}
                onclose={() => navigate(() => { workflow.open(null); document.getElementById(newSecretId)?.focus(); })} />
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
                {#if draft.kind === "login"}
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
                    {#if draft.secret && draft.secret !== (editor.record?.kind === "login" ? editor.record.secret : undefined)}
                        <Field id="confirm-secret" label="Confirm password" required>
                            {#snippet children(control)}
                                <input {...control} bind:value={editor.confirmPassword} type="password" autocomplete="new-password" disabled={!workflow.canEdit} />
                            {/snippet}
                        </Field>
                    {/if}
                    <Field id="totp" label="Authenticator key (TOTP)" description="Store a Base32 key or otpauth TOTP URI. View local codes in item details.">
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
                {:else if draft.kind === "card"}
                    {#key draft}<CardFields {draft} disabled={!workflow.canEdit} />{/key}
                {:else if draft.kind === "identity"}
                    <IdentityFields {draft} disabled={!workflow.canEdit} />
                {/if}

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
