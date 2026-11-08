<script lang="ts">
import { FolderKanban, Grid, Star, X, Funnel, Key, IdCard, CreditCard, HardDrive } from "@lucide/svelte";
import type { Folder } from "$lib/vault/folder";
import type { ItemTypeFilter, ItemSort } from "$lib/vault/secrets/search";

interface Props {
	folders: Folder[];
	folderFilter: string;
	searchText: string;
	itemType: ItemTypeFilter;
	favoritesOnly: boolean;
	sort: ItemSort;
	onFolderFilterChange: (id: string) => void;
	onItemTypeChange: (v: ItemTypeFilter) => void;
	onFavoritesOnlyChange: (v: boolean) => void;
	onSortChange: (v: ItemSort) => void;
	onClearFilters: () => void;
}

let {
	folders,
	folderFilter,
	searchText,
	itemType,
	favoritesOnly,
	sort,
	onFolderFilterChange,
	onItemTypeChange,
	onFavoritesOnlyChange,
	onSortChange,
	onClearFilters
}: Props = $props();

const typeFilters = $derived<{ value: ItemTypeFilter; label: string; icon: typeof Grid }[]>([
	{ value: "all", label: "All types", icon: Grid },
	{ value: "login", label: "Logins", icon: Key },
	{ value: "secureNote", label: "Secure notes", icon: HardDrive },
	{ value: "card", label: "Cards", icon: CreditCard },
	{ value: "identity", label: "Identities", icon: IdCard },
	{ value: "sshKey", label: "SSH keys", icon: HardDrive }
]);

const sortOptions = $derived([
	{ value: "favorites", label: "Favorites first" },
	{ value: "nameAsc", label: "Name (A–Z)" },
	{ value: "nameDesc", label: "Name (Z–A)" },
	{ value: "updatedDesc", label: "Recently updated" },
	{ value: "createdDesc", label: "Recently created" }
]);

const hasActiveFilters = $derived(searchText || itemType !== "all" || favoritesOnly);

function getFolderCount(folderId: string): number {
	return folders.length; // placeholder - actual count would come from workflow
}
</script>

<nav class="flex flex-col h-full bg-surface border-r border-line">
	<div class="flex h-14 items-center px-3 border-b border-line shrink-0">
		<div class="flex items-center gap-2">
			<Funnel class="w-5 h-5 text-muted-foreground" />
			<h2 class="text-base font-semibold text-ink truncate">Filters</h2>
		</div>
		{#if hasActiveFilters}
			<button class="btn btn-ghost btn-sm" onclick={onClearFilters}>
				<X class="w-3 h-3 mr-1" />Clear
			</button>
		{/if}
	</div>
	<div class="flex-1 overflow-y-auto p-2 space-y-3">
		<!-- Folders -->
		<div class="border border-line rounded-lg bg-surface-muted/30">
			<div class="px-3 py-2 text-sm font-medium text-ink">Folders</div>
			<div class="p-2 space-y-1 max-h-48 overflow-y-auto">
				<button
					class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover"
					onclick={() => onFolderFilterChange("all")}
					aria-pressed={folderFilter === "all"}
				>
					<span class="flex w-5 h-5 items-center justify-center">
						<FolderKanban class="w-4 h-4" />
					</span>
					<span class="truncate">All items</span>
					{#if folderFilter === "all"}
						<span class="ml-auto w-4 h-4 text-primary">✓</span>
					{/if}
				</button>
				<button
					class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover"
					onclick={() => onFolderFilterChange("unfiled")}
					aria-pressed={folderFilter === "unfiled"}
				>
					<span class="flex w-5 h-5 items-center justify-center text-muted-foreground">
						<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
							<path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
							<circle cx="12" cy="7" r="4" />
						</svg>
					</span>
					<span class="truncate">Unfiled</span>
					{#if folderFilter === "unfiled"}
						<span class="ml-auto w-4 h-4 text-primary">✓</span>
					{/if}
				</button>
				{#each folders as folder (folder.id)}
					<button
						class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover"
						onclick={() => onFolderFilterChange(folder.id)}
						aria-pressed={folderFilter === folder.id}
					>
						<span class="flex w-5 h-5 items-center justify-center">
							<FolderKanban class="w-4 h-4" />
						</span>
						<span class="truncate">{folder.name}</span>
						{#if folderFilter === folder.id}
							<span class="ml-auto w-4 h-4 text-primary">✓</span>
						{/if}
					</button>
				{/each}
				{#if folders.length === 0}
					<p class="px-2 py-2 text-xs text-muted-foreground text-center">No folders yet</p>
				{/if}
			</div>
		</div>

		<!-- Item Types -->
		<div class="border border-line rounded-lg bg-surface-muted/30">
			<div class="px-3 py-2 text-sm font-medium text-ink">Item types</div>
			<div class="p-2 space-y-1">
				{#each typeFilters as filter (filter.value)}
					<label class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover cursor-pointer">
						<input
							type="radio"
							name="item-type"
							value={filter.value}
							checked={itemType === filter.value}
							onchange={() => onItemTypeChange(filter.value)}
							class="sr-only"
						/>
						<span class="flex w-5 h-5 items-center justify-center">
							<filter.icon class="w-4 h-4" />
						</span>
						<span class="truncate">{filter.label}</span>
					</label>
				{/each}
			</div>
		</div>

		<!-- Favorites Only -->
		<div class="border border-line rounded-lg bg-surface-muted/30">
			<div class="p-3">
				<label class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover cursor-pointer">
					<input
						type="checkbox"
						checked={favoritesOnly}
						onchange={(e) => onFavoritesOnlyChange(e.currentTarget.checked)}
						class="sr-only peer"
					/>
					<span class="w-4 h-4 flex-shrink-0" class:fill-primary={favoritesOnly} class:text-primary={favoritesOnly}><Star class="w-4 h-4" /></span>
					<span class="truncate">Favorites only</span>
				</label>
			</div>
		</div>

		<!-- Sort -->
		<div class="border border-line rounded-lg bg-surface-muted/30">
			<div class="p-3">
				<label class="flex w-full flex-col gap-1.5">
					<span class="text-xs font-medium text-muted-foreground">Order by</span>
					<select
						value={sort}
						onchange={(e) => onSortChange(e.currentTarget.value as ItemSort)}
						class="w-full px-3 py-2 text-sm text-ink bg-surface border border-line rounded-lg focus:outline-none focus:ring-2 focus:ring-primary"
					>
						{#each sortOptions as option (option.value)}
							<option value={option.value}>{option.label}</option>
						{/each}
					</select>
				</label>
			</div>
		</div>
	</div>
</nav>
