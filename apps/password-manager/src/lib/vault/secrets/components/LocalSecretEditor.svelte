<script lang="ts">
import { onDestroy, tick, untrack } from "svelte";
import { goto } from "$app/navigation";
import SecretDetail from "./SecretDetail.svelte";
import CardFields from "./CardFields.svelte";
import IdentityFields from "./IdentityFields.svelte";
import SshKeyFields from "./SshKeyFields.svelte";
import FolderControls from "./FolderControls.svelte";
import GeneratorDialog from "../generator/GeneratorDialog.svelte";
import { Alert, Card, Field } from "$lib/ui";
import type { VaultHandle } from "../../vault";
import { createLocalSecretWorkflow } from "../workflow.svelte";
import type { Secret } from "../secret";
import { customFieldIssue, type SshKeyDraft } from "../secret-draft";
import { searchItems, type ItemTypeFilter, type ItemSort } from "../search";

interface Props {
    vault: VaultHandle;
    initialTrash?: boolean;
    // Filter state (controlled by parent)
    searchText?: string;
    itemType?: ItemTypeFilter;
    favoritesOnly?: boolean;
    sort?: ItemSort;
    onSearchTextChange?: (value: string) => void;
    onItemTypeChange?: (value: ItemTypeFilter) => void;
    onFavoritesOnlyChange?: (value: boolean) => void;
    onSortChange?: (value: ItemSort) => void;
    onClearFilters?: () => void;
    onFolderFilterChange?: (value: string) => void;
}

let {
    vault,
    initialTrash = false,
    searchText = "",
    itemType = "all",
    favoritesOnly = false,
    sort = "favorites",
    onSearchTextChange,
    onItemTypeChange,
    onFavoritesOnlyChange,
    onSortChange,
    onClearFilters,
    onFolderFilterChange
}: Props = $props();

const workflow = untrack(() => createLocalSecretWorkflow({ vault }));

$effect(() => { if (initialTrash) workflow.setTrashMode(true); });
onDestroy(retire);

let retired = $state(false);

function clearFilters() {
    searchText = "";
    itemType = "all";
    favoritesOnly = false;
    sort = "favorites";
    onSearchTextChange?.("");
    onItemTypeChange?.("all");
    onFavoritesOnlyChange?.(false);
    onSortChange?.("favorites");
    onClearFilters?.();
}

let results = $derived(retired ? [] : searchItems(workflow.records, {
    text: searchText, kind: itemType, favoritesOnly, sort,
}));


let pendingDelete = $state<Secret | null>(null);
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
let showNewSecretMenu = $state(false);
const newSecretId = `${uid}-new`;
const records = $derived(workflow.trash ? workflow.trashRecords : results);
const searchId = `${newSecretId}-search`;
let folderControls = $state<ReturnType<typeof FolderControls>>();

export function hasUnsavedChanges() { return workflow.dirty || workflow.busy || !!folderControls?.dirty(); }
export function folders() { return workflow.folders; }
export function folderFilter() { return workflow.folderFilter; }
export function setFolderFilter(id: string) { workflow.setFolderFilter(id); }
export function retire() {
    retired = true;
    clearFilters();
    sort = "favorites";
    results = [];
    pendingDelete = null;
    deleteTrigger = null;
    workflow.setTrashMode(false);
    folderControls?.retire();
    workflow.dispose();
}

function navigate(action: () => void) {
    if ((workflow.dirty || folderControls?.dirty()) && !window.confirm("Discard unsaved changes?")) return;
    pendingDelete = null;
    folderControls?.retire();
    action();
}

</script>

<svelte:window onbeforeunload={(event) => {
    if (hasUnsavedChanges()) { event.preventDefault(); event.returnValue = ""; }
}} />

<div class="flex flex-col gap-4">
    <Card title={workflow.trash ? "Trash" : "Secrets"} description={workflow.trash ? "Items stay here until you restore or permanently delete them. No automatic deletion." : "Stored locally in your encrypted vault."}>
        {#snippet actions()}
            <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canRefresh}
                onclick={() => navigate(() => void workflow.refresh())}>Refresh</button>
            <div class="relative" aria-haspopup="menu">
                <button id={newSecretId} class="btn btn-primary btn-md" type="button" disabled={!workflow.canEdit}
                    onclick={() => { showNewSecretMenu = !showNewSecretMenu; }} aria-expanded={showNewSecretMenu}>New secret</button>
                {#if showNewSecretMenu}
                <div class="absolute top-full left-0 mt-1 z-10 min-w-[160px] bg-surface border border-line rounded-lg shadow-lg py-1" role="menu" aria-labelledby={newSecretId}>
                    <button class="flex w-full items-center gap-2 px-3 py-2 text-sm text-ink hover:bg-surface-hover" role="menuitem" type="button" onclick={() => navigate(() => goto("/vault/new?kind=login"))}>New login</button>
                    <button class="flex w-full items-center gap-2 px-3 py-2 text-sm text-ink hover:bg-surface-hover" role="menuitem" type="button" onclick={() => navigate(() => goto("/vault/new?kind=secureNote"))}>New secure note</button>
                    <button class="flex w-full items-center gap-2 px-3 py-2 text-sm text-ink hover:bg-surface-hover" role="menuitem" type="button" onclick={() => navigate(() => goto("/vault/new?kind=card"))}>New card</button>
                    <button class="flex w-full items-center gap-2 px-3 py-2 text-sm text-ink hover:bg-surface-hover" role="menuitem" type="button" onclick={() => navigate(() => goto("/vault/new?kind=identity"))}>New identity</button>
                    <button class="flex w-full items-center gap-2 px-3 py-2 text-sm text-ink hover:bg-surface-hover" role="menuitem" type="button" onclick={() => navigate(() => goto("/vault/new?kind=sshKey"))}>New SSH key</button>
                </div>
                {/if}
            </div>
            {/snippet}
        {#if !retired && !workflow.trash}
            <fieldset class="mb-4 flex flex-wrap items-end gap-3 lg:hidden" disabled={workflow.loading || !!workflow.loadError}>
                <legend class="text-sm font-medium">Find items</legend>
                <Field id={`${searchId}-text`} label="Search items" description="Search names, usernames, websites, notes, and custom-field names.">
                    {#snippet children(control)}
                        <input {...control} type="search" autocomplete="off" spellcheck="false"
                            value={searchText}
                            oninput={(e) => { if (!retired) { searchText = e.currentTarget.value; onSearchTextChange?.(searchText); } }} />
                    {/snippet}
                </Field>
                <Field id={`${searchId}-folder`} label="Folder">
                    {#snippet children(control)}
                        <select {...control} value={workflow.folderFilter} onchange={(e) => { const value = e.currentTarget.value; onFolderFilterChange?.(value); }}>                            <option value="all">All folders</option>
                            <option value="unfiled">Unfiled</option>
                            {#each workflow.folders as folder (folder.id)}
                                <option value={folder.id}>{folder.name}</option>
                            {/each}
                        </select>
                    {/snippet}
                </Field>
                <Field id={`${searchId}-type`} label="Item type">
                    {#snippet children(control)}
                        <select {...control} value={itemType} onchange={(e) => { itemType = e.currentTarget.value as ItemTypeFilter; onItemTypeChange?.(itemType); }}>                            <option value="all">All types</option>
                            <option value="login">Login</option>
                            <option value="secureNote">Secure Note</option>
                            <option value="card">Card</option>
                            <option value="identity">Identity</option>
                            <option value="sshKey">SSH Key</option>
                        </select>
                    {/snippet}
                </Field>
                <label class="flex items-center gap-2 text-sm text-ink">
                    <input type="checkbox" checked={favoritesOnly} onchange={(e) => { favoritesOnly = e.currentTarget.checked; onFavoritesOnlyChange?.(favoritesOnly); }} /> Favorites only
                </label>
                <Field id={`${searchId}-sort`} label="Sort items">
                    {#snippet children(control)}
                        <select {...control} value={sort} onchange={(e) => { sort = e.currentTarget.value as ItemSort; onSortChange?.(sort); }}>                            <option value="favorites">Favorites first, then name</option>
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
            {#if !workflow.trash}<FolderControls bind:this={folderControls} {workflow} {navigate} />{/if}
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
                <p class="mb-0" role="status">{workflow.trash ? "Trash is empty." : searchText.trim() || itemType !== "all" || favoritesOnly ? "No items match. Clear filters to show all items." : workflow.folderFilter === "all" ? "No secrets yet." : "No items in this folder view."}</p>
            {:else}
                <p aria-live="polite" aria-atomic="true">{records.length} {records.length === 1 ? "item" : "items"} found.</p>
                <ul aria-label="Search results" class="flex list-none flex-col gap-2 p-0">
                    {#each records as value (value.id)}
                        <li class="flex items-center justify-between gap-3 rounded-control border border-line bg-surface-muted/40 px-3 py-2">
                            <div class="flex min-w-0 flex-col">
                                {#if workflow.trash}
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
                            {#if workflow.trash}
                                <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit}
                                    onclick={async () => { await workflow.restore(value); document.getElementById(navigationId)?.focus(); }}>Restore</button>
                            {/if}
                            <button class="btn btn-ghost btn-md shrink-0 text-danger-text hover:bg-danger-soft"
                                type="button" disabled={!workflow.canEdit} onclick={(event) => navigate(() => { workflow.open(null); deleteTrigger = event.currentTarget; pendingDelete = value; })}>{workflow.trash ? "Delete permanently" : "Delete"}</button>
                        </li>
                    {/each}
                </ul>
            {/if}
        {/if}
    </Card>

    {#if workflow.detail && !workflow.editor}
        {#key workflow.detail}
            <SecretDetail record={workflow.detail} canEdit={workflow.canEdit}
                onedit={() => navigate(() => goto(`/vault/edit/${workflow.detail.id}`))}
                onclose={() => navigate(() => { workflow.open(null); document.getElementById(newSecretId)?.focus(); })} />
        {/key}
    {/if}
</div>
