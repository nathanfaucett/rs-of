<script lang="ts">
import { onMount } from "svelte";
import { goto } from "$app/navigation";
import LocalSecretEditor from "$lib/vault/secrets/components/LocalSecretEditor.svelte";
import FilterSidebar from "../layout/FilterSidebar.svelte";
import { Alert } from "$lib/ui";
import { notifications } from "$lib/shared/state/notifications.svelte";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import type { ItemTypeFilter, ItemSort } from "$lib/vault/secrets/search";
import type { Folder } from "$lib/vault/folder";

let error = $state("");
let notice = $state("");
let itemEditor = $state<ReturnType<typeof LocalSecretEditor>>();

let vaultData = $derived(vaultStateModule.vaultData);
let vaultStatus = $derived(vaultStateModule.state.status);

// Filter state (shared between FilterSidebar and LocalSecretEditor)
let searchText = $state("");
let itemType = $state<ItemTypeFilter>("all");
let favoritesOnly = $state(false);
let sort = $state<ItemSort>("favorites");

function clearFilters() {
    searchText = "";
    itemType = "all";
    favoritesOnly = false;
    sort = "favorites";
}

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
				<FilterSidebar
					folders={itemEditor?.folders?.() ?? []}
					folderFilter={itemEditor?.folderFilter?.() ?? "all"}
					searchText={searchText}
					itemType={itemType}
					favoritesOnly={favoritesOnly}
					sort={sort}
					onFolderFilterChange={(id) => itemEditor?.setFolderFilter?.(id)}
					onItemTypeChange={(v) => itemType = v}
					onFavoritesOnlyChange={(v) => favoritesOnly = v}
					onSortChange={(v) => sort = v}
					onClearFilters={clearFilters}
				/>
			</div>
			<div class="flex-1 min-w-0">
				<LocalSecretEditor
					bind:this={itemEditor}
					vault={vaultData.vault}
					searchText={searchText}
					itemType={itemType}
					favoritesOnly={favoritesOnly}
					sort={sort}
					onSearchTextChange={(v) => searchText = v}
					onItemTypeChange={(v) => itemType = v}
					onFavoritesOnlyChange={(v) => favoritesOnly = v}
					onSortChange={(v) => sort = v}
					onClearFilters={clearFilters}
				/>
			</div>
		</div>
	{/if}
</div>
