<script lang="ts">
import { goto } from "$app/navigation";
import { resolve } from "$app/paths";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import SyncStatus from "$lib/vault/components/SyncStatus.svelte";
import LocalSecretEditor from "$lib/vault/secrets/components/LocalSecretEditor.svelte";
import { createFilterState } from "$lib/vault/secrets/filter-state.svelte";
import FilterSidebar from "../layout/FilterSidebar.svelte";

let itemEditor = $state<ReturnType<typeof LocalSecretEditor>>();

let vaultData = $derived(vaultStateModule.vaultData);
let vaultStatus = $derived(vaultStateModule.state.status);

const filters = createFilterState();

// Redirect to vault selector if vault is not unlocked
$effect(() => {
    if (vaultStatus !== "loading" && vaultStatus !== "select-vault" && !vaultData) {
        void goto(resolve("/"));
    }
});
</script>

<svelte:head><title>Items | Password Manager</title></svelte:head>

<div class="p-3 pt-16 sm:px-6 lg:pt-6">
	{#if vaultData}
		<div class="mx-auto flex w-full max-w-[1400px] flex-col gap-4 lg:flex-row lg:gap-6">
			<div class="min-w-0 lg:w-64 lg:shrink-0">
				<SyncStatus />
				<FilterSidebar
					folders={itemEditor?.folders() ?? []}
					folderFilter={itemEditor?.folderFilter() ?? "all"}
					searchText={filters.searchText}
					itemType={filters.itemType}
					favoritesOnly={filters.favoritesOnly}
					sort={filters.sort}
					onFolderFilterChange={(id) => itemEditor?.setFolderFilter(id)}
					onItemTypeChange={(v) => filters.itemType = v}
					onFavoritesOnlyChange={(v) => filters.favoritesOnly = v}
					onSortChange={(v) => filters.sort = v}
					onClearFilters={() => itemEditor?.clearFilters()}
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
					onClearFilters={filters.clearFilters}
				/>
			</div>
		</div>
	{/if}
</div>
