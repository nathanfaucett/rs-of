<script lang="ts">
import { onMount } from "svelte";
import { goto } from "$app/navigation";
import LocalSecretEditor from "$lib/vault/secrets/components/LocalSecretEditor.svelte";
import FilterSidebar from "../layout/FilterSidebar.svelte";
import SyncStatus from "$lib/vault/components/SyncStatus.svelte";
import { Alert } from "$lib/ui";
import { notifications } from "$lib/shared/state/notifications.svelte";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import { createFilterState } from "$lib/vault/secrets/filter-state.svelte";

let error = $state("");
let notice = $state("");
let itemEditor = $state<ReturnType<typeof LocalSecretEditor>>();

let vaultData = $derived(vaultStateModule.vaultData);
let vaultStatus = $derived(vaultStateModule.state.status);

const filters = createFilterState();

onMount(() => {
    // Initialize vault-specific state
});

// Redirect to vault selector if vault is not unlocked
$effect(() => {
    if (vaultStatus !== "loading" && vaultStatus !== "select-vault" && !vaultData) {
        goto("/");
    }
});
</script>

<div class="flex min-h-0 grow overflow-auto p-6">
	{#if error}
		<Alert tone="danger">{error}</Alert>
	{/if}
	{#if notice}
		<p role="status">{notice}</p>
	{/if}
	{#if vaultData}
		<div class="flex gap-6 w-full max-w-[1400px] mx-auto">
			<div class="lg:w-64 flex-shrink-0">
				<SyncStatus />
				<FilterSidebar
					folders={itemEditor?.folders?.() ?? []}
					folderFilter={itemEditor?.folderFilter?.() ?? "all"}
					searchText={filters.searchText}
					itemType={filters.itemType}
					favoritesOnly={filters.favoritesOnly}
					sort={filters.sort}
					onFolderFilterChange={(id) => itemEditor?.setFolderFilter?.(id)}
					onItemTypeChange={(v) => filters.itemType = v}
					onFavoritesOnlyChange={(v) => filters.favoritesOnly = v}
					onSortChange={(v) => filters.sort = v}
					onClearFilters={filters.clearFilters}
				/>
			</div>
			<div class="flex-1 min-w-0">
				<LocalSecretEditor
				                    bind:this={itemEditor}
				                    vault={vaultData.vault}
				                    searchText={filters.searchText}
				                    itemType={filters.itemType}
				                    favoritesOnly={filters.favoritesOnly}
				                    sort={filters.sort}
				                    onSearchTextChange={(v) => filters.searchText = v}
				                    onItemTypeChange={(v) => filters.itemType = v}
				                    onFavoritesOnlyChange={(v) => filters.favoritesOnly = v}
				                    onSortChange={(v) => filters.sort = v}
				                    onFolderFilterChange={(v) => itemEditor?.setFolderFilter?.(v)}
				                    onClearFilters={filters.clearFilters}
				                />
			</div>
		</div>
	{/if}
</div>
