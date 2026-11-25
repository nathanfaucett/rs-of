<script lang="ts">
import { Pencil, Trash2 } from "@lucide/svelte";
import { onDestroy, tick, untrack } from "svelte";
import { goto } from "$app/navigation";
import { resolve } from "$app/paths";
import Dialog from "$lib/shared/components/Dialog.svelte";
import { Alert, Card, Field, type FieldControl } from "$lib/ui";
import type { VaultHandleLike } from "$lib/vault/types";
import { type ItemSort, type ItemTypeFilter, searchItems } from "../search";
import type { Secret } from "../secret";
import { createLocalSecretWorkflow } from "../workflow.svelte";
import FolderControls from "./FolderControls.svelte";
import NewItemMenu from "./NewItemMenu.svelte";
import SecretDetail from "./SecretDetail.svelte";

interface Props {
    vault: VaultHandleLike;
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
    onClearFilters
}: Props = $props();

const workflow = untrack(() => createLocalSecretWorkflow({ vault }));

$effect(() => { if (initialTrash) workflow.setTrashMode(true); });
onDestroy(retire);

let retired = $state(false);

export function clearFilters() {
    setFolderFilter("all");
    onClearFilters?.();
}

const results = $derived(retired ? [] : searchItems(workflow.records, {
    text: searchText, kind: itemType, favoritesOnly, sort,
}));


let pendingDelete = $state<Secret | null>(null);
let deleteTrigger: HTMLButtonElement | null = null;
const uid = $props.id();

const navigationId = `${uid}-navigation`;
async function cancelDelete() {
    const trigger = deleteTrigger;
    pendingDelete = null;
    deleteTrigger = null;
    await tick();
    trigger?.focus();
}
const records = $derived(workflow.trash ? workflow.trashRecords : results);
const searchId = `${uid}-search`;
let detailTrigger: HTMLButtonElement | undefined;
const folderChoices = $derived.by(() => {
    const folders = workflow.folders;
    const totals = new Map<string, number>();
    const indices = new Map<string, number>();
    for (const { name } of folders) totals.set(name, (totals.get(name) ?? 0) + 1);
    return folders.map((folder) => {
        const { name } = folder;
        const index = (indices.get(name) ?? 0) + 1;
        indices.set(name, index);
        const label = (totals.get(name) ?? 0) > 1 ? `${name} (${index})` : name === "Unfiled" ? "Unfiled (folder)" : name;
        return { ...folder, label };
    });
});
let folderControls = $state<ReturnType<typeof FolderControls>>();

export function hasUnsavedChanges() { return workflow.dirty || workflow.busy || !!folderControls?.dirty(); }
export function folders() { return folderChoices; }
export function folderFilter() { return workflow.folderFilter; }
export function setFolderFilter(id: string) {
    if (id !== workflow.folderFilter) workflow.setFolderFilter(id);
}
export function retire() {
    retired = true;
    clearFilters();
    pendingDelete = null;
    deleteTrigger = null;
    workflow.setTrashMode(false);
    folderControls?.retire();
    workflow.dispose();
}

async function closeDetails() {
    const trigger = detailTrigger;
    detailTrigger = undefined;
    workflow.open(null);
    await tick();
    (trigger?.isConnected ? trigger : document.getElementById(`${searchId}-text`))?.focus();
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
    <Card title={workflow.trash ? "Trash" : "Items"} description={workflow.trash ? "Items stay here until you restore or permanently delete them. No automatic deletion." : "Stored locally in your encrypted vault."}>
        {#snippet actions()}
            <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canRefresh}
                onclick={() => navigate(() => void workflow.refresh())}>Refresh</button>
            <NewItemMenu disabled={!workflow.canEdit} onselect={(kind) => navigate(() => goto(resolve(`/vault/new?kind=${kind}`)))} />
            {/snippet}
        {#if !retired && !workflow.trash}
            <div class="mb-4">
                <Field id={`${searchId}-text`} label="Search items" description="Search names, usernames, websites, notes, and custom-field names.">
                    {#snippet children(control: FieldControl)}
                        <input {...control} type="search" autocomplete="off" spellcheck="false"
                            value={searchText} disabled={workflow.loading || !!workflow.loadError}
                            oninput={(e) => onSearchTextChange?.(e.currentTarget.value)} />
                    {/snippet}
                </Field>
            </div>
            <fieldset class="mb-4 flex min-w-0 flex-wrap items-end gap-3 [&>.field]:min-w-0 [&>.field]:max-w-full lg:hidden" disabled={workflow.loading || !!workflow.loadError}>
                <legend class="text-sm font-medium">Filters</legend>
                <Field id={`${searchId}-folder`} label="Folder">
                    {#snippet children(control: FieldControl)}
                        <select {...control} class="max-w-full" value={workflow.folderFilter} onchange={(e) => setFolderFilter(e.currentTarget.value)}>
                            <option value="all">All folders</option>
                            <option value="unfiled">Unfiled</option>
                            {#each folderChoices as folder (folder.id)}
                                <option value={folder.id}>{folder.label}</option>
                            {/each}
                        </select>
                    {/snippet}
                </Field>
                <Field id={`${searchId}-type`} label="Item type">
                    {#snippet children(control: FieldControl)}
                        <select {...control} value={itemType} onchange={(e) => onItemTypeChange?.(e.currentTarget.value as ItemTypeFilter)}>
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
                    <input type="checkbox" checked={favoritesOnly} onchange={(e) => onFavoritesOnlyChange?.(e.currentTarget.checked)} /> Favorites only
                </label>
                <Field id={`${searchId}-sort`} label="Sort items">
                    {#snippet children(control: FieldControl)}
                        <select {...control} value={sort} onchange={(e) => onSortChange?.(e.currentTarget.value as ItemSort)}>
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
            <p role="status">Refreshing items…</p>
        {/if}
        {#if workflow.loadError}
            <Alert tone="danger" title="Vault unavailable">{workflow.loadError}</Alert>
        {:else if workflow.loading}
            <p role="status">Loading items…</p>
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
                <Dialog title={pendingDelete.deletedAt ? "Permanently delete?" : "Move to Trash?"} open onclose={cancelDelete}>
                    <p class="text-ink">{pendingDelete.deletedAt ? `Permanently delete ${pendingDelete.name}? This cannot be undone. Existing backups can still contain this item.` : `Move ${pendingDelete.name} to Trash? You can restore it later.`}</p>
                    <div class="flex gap-2 justify-end">
                        <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit}
                            onclick={cancelDelete}>Cancel</button>
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
                    </div>
                </Dialog>
            {/if}
            {#if records.length === 0}
                <p class="mb-0" role="status">{workflow.trash ? "Trash is empty." : !workflow.hasItems ? "No items yet." : searchText.trim() || itemType !== "all" || favoritesOnly ? "No items match. Clear filters to show all items." : "No items in this folder view."}</p>
                {#if !workflow.trash && !workflow.hasItems}
                    <div class="mt-3 flex flex-wrap gap-2">
                        <a class="btn btn-primary btn-md" href={resolve("/vault/new?kind=login")}>New login</a>
                        <a class="btn btn-secondary btn-md" href={`${resolve("/settings")}#import-json`}>Import items</a>
                    </div>
                {/if}
            {:else}
                <p aria-live="polite" aria-atomic="true">{records.length} {records.length === 1 ? "item" : "items"} found.</p>
                <ul aria-label={workflow.trash ? "Search results" : "Items"} class="flex list-none flex-col gap-2 p-0">
                    {#each records as value (value.id)}
                        <li class="item-row rounded-control border border-line bg-surface-muted/40 px-3 py-2">
                            <div class="item-content">
                                {#if workflow.trash}
                                    <span class="font-medium">{value.name}</span>
                                    <span class="text-sm text-ink-subtle">Deleted: <time datetime={value.deletedAt}>{value.deletedAt}</time></span>
                                {:else}
                                <button class="btn btn-ghost btn-md h-auto min-h-9 w-full justify-start whitespace-normal px-0 text-left font-medium"
                                    type="button" disabled={!workflow.canEdit} onclick={(event) => navigate(() => {
                                        detailTrigger = event.currentTarget;
                                        workflow.open(value.id);
                                    })}>
                                    <span class="min-w-0 wrap-anywhere">{#if value.favorite}<span aria-hidden="true">★ </span>{/if}{value.name}</span>
                                </button>
                                {#if (value.kind === "login" || value.kind === "identity") && value.username}<p class="wrap-anywhere text-sm text-ink-subtle">{value.username}</p>{/if}
                                {#each value.kind === "login" ? value.uris : [] as uri (uri)}
                                    <p class="wrap-anywhere text-xs text-ink-subtle">{uri.uri}</p>
                                {/each}
                                {/if}
                            </div>
                            <div class="item-actions">
                                {#if workflow.trash}
                                    <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit}
                                        onclick={async () => { await workflow.restore(value); document.getElementById(navigationId)?.focus(); }}>Restore</button>
                                {:else}
                                    <button class="btn btn-ghost btn-sm p-1.5 text-muted-foreground hover:text-ink hover:bg-surface-hover rounded"
                                        type="button" disabled={!workflow.canEdit} aria-label="Edit"
                                        onclick={() => navigate(() => goto(resolve(`/vault/edit/${value.id}`)))}>
                                        <Pencil class="w-4 h-4" />
                                    </button>
                                    <button class="btn btn-ghost btn-sm p-1.5 text-muted-foreground hover:text-danger-text hover:bg-danger-soft rounded"
                                        type="button" disabled={!workflow.canEdit} aria-label="Delete"
                                        onclick={(event) => navigate(() => { workflow.open(null); deleteTrigger = event.currentTarget; pendingDelete = value; })}>
                                        <Trash2 class="w-4 h-4" />
                                    </button>
                                {/if}
                            </div>
                        </li>
                    {/each}
                </ul>
            {/if}
        {/if}
    </Card>

    {#if workflow.detail && !workflow.editor}
        {@const detail = workflow.detail}
        {#key detail.id}
            <SecretDetail record={detail} canEdit={workflow.canEdit}
                onedit={() => navigate(() => goto(resolve(`/vault/edit/${detail.id}`)))} onclose={closeDetails}>
                {#snippet folderAssignment()}
                    <form class="flex flex-wrap items-end gap-2" onsubmit={(event) => {
                        event.preventDefault();
                        const destination = new FormData(event.currentTarget).get("destination");
                        if (typeof destination === "string") navigate(() => { void workflow.moveItem(detail.id, destination || null); });
                    }}>
                        <label class="flex min-w-0 flex-col gap-1">Move to folder
                            <select name="destination" class="w-full" value={detail.folderId ?? ""} disabled={!workflow.canEdit}>
                                <option value="">Unfiled</option>
                                {#each folderChoices as folder (folder.id)}
                                    <option value={folder.id}>{folder.label}</option>
                                {/each}
                            </select>
                        </label>
                        <button class="btn btn-secondary btn-md" type="submit" disabled={!workflow.canEdit}>Move item</button>
                    </form>
                {/snippet}
            </SecretDetail>
        {/key}
    {/if}

<style>
    .item-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 1rem;
    }
    .item-content {
        flex: 1;
        min-width: 0;
    }
    .item-actions {
        display: flex;
        align-items: center;
        gap: 0.25rem;
        flex-shrink: 0;
    }
</style>
</div>
