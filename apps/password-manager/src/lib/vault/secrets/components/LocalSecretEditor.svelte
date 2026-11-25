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
import SecretDetail from "./SecretDetail.svelte";

interface Props {
    vault: VaultHandleLike;
    // Filter state (controlled by parent)
    searchText?: string;
    itemType?: ItemTypeFilter;
    favoritesOnly?: boolean;
    sort?: ItemSort;
    onSearchTextChange?: (value: string) => void;
    onItemTypeChange?: (value: ItemTypeFilter) => void;
    onFavoritesOnlyChange?: (value: boolean) => void;
    onSortChange?: (value: ItemSort) => void;
    onFolderFilterChange?: (value: string) => void;
    onClearFilters?: () => void;
}

let {
    vault,
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
    <Card title="Secrets" description="Stored locally in your encrypted vault.">
        {#snippet actions()}
            <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canRefresh}
                onclick={() => navigate(() => void workflow.refresh())}>Refresh</button>
            <div class="relative" aria-haspopup="menu">
                <button id={newSecretId} class="btn btn-primary btn-md" type="button" disabled={!workflow.canEdit}
                    onclick={() => { showNewSecretMenu = !showNewSecretMenu; }} aria-expanded={showNewSecretMenu}>New secret</button>
                {#if showNewSecretMenu}
                <div class="absolute top-full left-0 mt-1 z-10 min-w-[160px] bg-surface border border-line rounded-lg shadow-lg py-1" role="menu" aria-labelledby={newSecretId}>
                    <button class="flex w-full items-center gap-2 px-3 py-2 text-sm text-ink hover:bg-surface-hover" role="menuitem" type="button" onclick={() => navigate(() => goto(resolve("/vault/new?kind=login")))}>New login</button>
                    <button class="flex w-full items-center gap-2 px-3 py-2 text-sm text-ink hover:bg-surface-hover" role="menuitem" type="button" onclick={() => navigate(() => goto(resolve("/vault/new?kind=secureNote")))}>New secure note</button>
                    <button class="flex w-full items-center gap-2 px-3 py-2 text-sm text-ink hover:bg-surface-hover" role="menuitem" type="button" onclick={() => navigate(() => goto(resolve("/vault/new?kind=card")))}>New card</button>
                    <button class="flex w-full items-center gap-2 px-3 py-2 text-sm text-ink hover:bg-surface-hover" role="menuitem" type="button" onclick={() => navigate(() => goto(resolve("/vault/new?kind=identity")))}>New identity</button>
                    <button class="flex w-full items-center gap-2 px-3 py-2 text-sm text-ink hover:bg-surface-hover" role="menuitem" type="button" onclick={() => navigate(() => goto(resolve("/vault/new?kind=sshKey")))}>New SSH key</button>
                </div>
                {/if}
            </div>
            {/snippet}
        {#if !retired}
            <fieldset class="mb-4 flex flex-wrap items-end gap-3 lg:hidden" disabled={workflow.loading || !!workflow.loadError}>
                <legend class="text-sm font-medium">Find items</legend>
                <Field id={`${searchId}-text`} label="Search items" description="Search names, usernames, websites, notes, and custom-field names.">
                    {#snippet children(control: FieldControl)}
                        <input {...control} type="search" autocomplete="off" spellcheck="false"
                            value={searchText}
                            oninput={(e) => { if (!retired) { searchText = e.currentTarget.value; onSearchTextChange?.(searchText); } }} />
                    {/snippet}
                </Field>
                <Field id={`${searchId}-folder`} label="Folder">
                    {#snippet children(control: FieldControl)}
                        <select {...control} value={workflow.folderFilter} onchange={(e) => { const value = e.currentTarget.value; onFolderFilterChange?.(value); }}>                            <option value="all">All folders</option>
                            <option value="unfiled">Unfiled</option>
                            {#each workflow.folders as folder (folder.id)}
                                <option value={folder.id}>{folder.name}</option>
                            {/each}
                        </select>
                    {/snippet}
                </Field>
                <Field id={`${searchId}-type`} label="Item type">
                    {#snippet children(control: FieldControl)}
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
                    {#snippet children(control: FieldControl)}
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
            <FolderControls bind:this={folderControls} {workflow} {navigate} />
            {#if pendingDelete}
                <Dialog title="Move to Trash?" open onclose={cancelDelete}>
                    <p class="text-ink">Move {pendingDelete.name} to Trash? You can restore it later.</p>
                    <div class="flex gap-2 justify-end">
                        <button class="btn btn-secondary btn-md" type="button" disabled={!workflow.canEdit}
                            onclick={cancelDelete}>Cancel</button>
                        <button class="btn btn-primary btn-md" type="button" disabled={!workflow.canEdit}
                            onclick={async () => {
                                const value = pendingDelete;
                                pendingDelete = null;
                                deleteTrigger = null;
                                if (value) await workflow.delete(value);
                                document.getElementById(navigationId)?.focus();
                            }}>Confirm delete</button>
                    </div>
                </Dialog>
            {/if}
            {#if results.length === 0}
                <p class="mb-0" role="status">{searchText.trim() || itemType !== "all" || favoritesOnly ? "No items match. Clear filters to show all items." : workflow.folderFilter === "all" ? "No secrets yet." : "No items in this folder view."}</p>
            {:else}
                <p aria-live="polite" aria-atomic="true">{results.length} {results.length === 1 ? "item" : "items"} found.</p>
                <ul aria-label="Search results" class="flex list-none flex-col gap-2 p-0">
                    {#each results as value (value.id)}
                        <li class="item-row rounded-control border border-line bg-surface-muted/40 px-3 py-2">
                            <div class="item-content">
                                <button class="btn btn-ghost btn-md w-full justify-start truncate px-0 text-left font-medium"
                                    type="button" disabled={!workflow.canEdit} onclick={() => navigate(() => goto(resolve(`/vault/edit/${value.id}`)))}>
                                    {value.favorite ? "★ " : ""}{value.name}
                                </button>
                                {#if (value.kind === "login" || value.kind === "identity") && value.username}<p class="truncate text-sm text-ink-subtle">{value.username}</p>{/if}
                                {#each value.kind === "login" ? value.uris : [] as uri (uri)}
                                    <p class="truncate text-xs text-ink-subtle">{uri.uri}</p>
                                {/each}
                            </div>
                            <div class="item-actions">
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
                            </div>
                        </li>
                    {/each}
                </ul>
            {/if}
        {/if}
    </Card>

    {#if workflow.detail && !workflow.editor}
        {@const detail = workflow.detail}
        {#key detail}
            <SecretDetail record={detail} canEdit={workflow.canEdit}
                onedit={() => navigate(() => goto(resolve(`/vault/edit/${detail.id}`)))}
                onclose={() => navigate(() => { workflow.open(null); document.getElementById(newSecretId)?.focus(); })} />
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
