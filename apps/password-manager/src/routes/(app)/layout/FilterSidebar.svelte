<script lang="ts">
	import { onMount } from "svelte";
	import { browser } from "$app/environment";
	import { ChevronDown, ChevronUp, FolderKanban, Grid, Star, X, Funnel, ChevronLeft, ChevronRight } from "@lucide/svelte";
	import type { Folder } from "$lib/vault/folder";
	import type { ItemTypeFilter, ItemSort } from "$lib/vault/secrets/search";

	interface Props {
		folders: Folder[];
		folderFilter: string;
		searchText: string;
		itemType: ItemTypeFilter;
		favoritesOnly: boolean;
		sort: ItemSort;
		onFolderFilterChange: (folderId: string) => void;
		onItemTypeChange: (type: ItemTypeFilter) => void;
		onFavoritesOnlyChange: (value: boolean) => void;
		onSortChange: (sort: ItemSort) => void;
		onClearFilters: () => void;
		inline?: boolean;
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
		onClearFilters,
		inline = false
	}: Props = $props();

	let collapsed = $state(false);
	let drawerOpen = $state(false);
	let foldersOpen = $state(true);
	let typesOpen = $state(true);
	let sortOpen = $state(false);

	const typeFilters: { value: ItemTypeFilter; label: string; icon: any }[] = [
		{ value: "all", label: "All types", icon: Grid },
		{ value: "login", label: "Logins", icon: FolderKanban },
		{ value: "secureNote", label: "Secure Notes", icon: FolderKanban },
		{ value: "card", label: "Cards", icon: FolderKanban },
		{ value: "identity", label: "Identities", icon: FolderKanban },
		{ value: "sshKey", label: "SSH Keys", icon: FolderKanban },
	];

	const sortOptions: { value: ItemSort; label: string }[] = [
		{ value: "favorites", label: "Favorites first" },
		{ value: "nameAsc", label: "Name (A-Z)" },
		{ value: "nameDesc", label: "Name (Z-A)" },
		{ value: "updatedDesc", label: "Recently updated" },
		{ value: "createdDesc", label: "Recently created" },
	];

	const hasActiveFilters = searchText || itemType !== "all" || favoritesOnly;

	function initCollapsed() {
		if (!browser) return;
		const stored = localStorage.getItem("pm:vault:filter:collapsed");
		if (stored !== null) {
			collapsed = stored === "true";
		} else {
			collapsed = window.innerWidth < 1024;
		}
	}

	function toggleCollapse() {
		collapsed = !collapsed;
		if (browser) {
			localStorage.setItem("pm:vault:filter:collapsed", String(collapsed));
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === "Escape" && drawerOpen) {
			event.preventDefault();
			closeDrawer();
		}
	}

	function openDrawer() {
		drawerOpen = true;
	}

	function closeDrawer() {
		drawerOpen = false;
	}

	function handleOverlayClick() {
		closeDrawer();
	}

	function handleOverlayKeydown(event: KeyboardEvent) {
		if (event.key === "Escape") {
			closeDrawer();
		}
	}

	$effect(() => {
		if (!browser) return;
		const handleResize = () => {
			if (window.innerWidth >= 1024 && drawerOpen) {
				closeDrawer();
			}
		};
		window.addEventListener("resize", handleResize);
		return () => window.removeEventListener("resize", handleResize);
	});

	onMount(() => {
		initCollapsed();
	});
</script>

{#snippet sectionHeader(title, open, onToggle, count = null)}
	<button
		type="button"
		class="flex w-full items-center justify-between gap-2 px-2 py-1.5 text-sm font-medium text-ink hover:text-ink transition-colors"
		onclick={onToggle}
		aria-expanded={open}
	>
		<div class="flex items-center gap-2">
			{#if open}
				<ChevronDown class="w-4 h-4 text-muted-foreground" />
			{:else}
				<ChevronRight class="w-4 h-4 text-muted-foreground" />
			{/if}
			<span class="truncate">{title}</span>
		</div>
		{#if count !== null && count > 0}
			<span class="flex h-5 min-w-5 items-center justify-center px-1.5 text-xs font-medium text-muted-foreground bg-surface-muted rounded-full">{count}</span>
		{/if}
	</button>
{/snippet}

{#if inline}
	<!-- Inline mode: part of page content flow -->
	<nav
		aria-label="Vault filters"
		class="flex-shrink-0 w-64 lg:w-64 flex flex-col bg-surface border-r border-line transition-all duration-200 ease-out"
		style:--filter-width={"240px"}
	>
		<div class="flex h-14 items-center px-3 border-b border-line shrink-0">
			<div class="flex items-center gap-2">
				<Funnel class="w-5 h-5 text-muted-foreground" />
				<h2 class="text-base font-semibold text-ink truncate">Filters</h2>
			</div>
			{#if hasActiveFilters}
				<button
					type="button"
					class="ml-auto px-2 py-1 text-xs font-medium text-muted-foreground hover:text-ink hover:bg-surface-hover rounded transition-colors"
					onclick={onClearFilters}
					aria-label="Clear all filters"
				>
					<X class="w-3 h-3 mr-1" />
					Clear
				</button>
			{/if}
		</div>

		<div class="flex-1 overflow-y-auto p-2 space-y-3">
			<!-- Folders Section -->
			<div class="border border-line rounded-lg bg-surface-muted/30">
				<div class="px-3 py-2 border-b border-line">
					{@render sectionHeader("Folders", foldersOpen, () => foldersOpen = !foldersOpen, folders.length)}
				</div>
				{#if foldersOpen}
					<div class="p-2 space-y-1 max-h-48 overflow-y-auto">
						<button
							type="button"
							class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover
								{folderFilter === 'all' ? 'bg-accent-soft text-accent-text font-medium' : ''}"
							onclick={() => onFolderFilterChange("all")}
						>
							<span class="flex w-5 h-5 items-center justify-center">
								<FolderKanban class="w-4 h-4" />
							</span>
							<span class="truncate">All folders</span>
						</button>
						<button
							type="button"
							class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover
								{folderFilter === 'unfiled' ? 'bg-accent-soft text-accent-text font-medium' : ''}"
							onclick={() => onFolderFilterChange("unfiled")}
						>
							<span class="flex w-5 h-5 items-center justify-center text-muted-foreground">
								<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path><circle cx="12" cy="7" r="4"></circle></svg>
							</span>
							<span class="truncate">Unfiled</span>
						</button>
						{#each folders as folder (folder.id)}
							<button
								type="button"
								class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover
									{folderFilter === folder.id ? 'bg-accent-soft text-accent-text font-medium' : ''}"
								onclick={() => onFolderFilterChange(folder.id)}
							>
								<span class="flex w-5 h-5 items-center justify-center">
									<FolderKanban class="w-4 h-4" />
								</span>
								<span class="truncate">{folder.name}</span>
							</button>
						{/each}
						{#if folders.length === 0}
							<p class="px-2 py-2 text-xs text-muted-foreground text-center">No folders yet</p>
						{/if}
					</div>
				{/if}
			</div>

			<!-- Types Section -->
			<div class="border border-line rounded-lg bg-surface-muted/30">
				<div class="px-3 py-2 border-b border-line">
					{@render sectionHeader("Types", typesOpen, () => typesOpen = !typesOpen)}
				</div>
				{#if typesOpen}
					<div class="p-2 space-y-1">
						{#each typeFilters as filter (filter.value)}
							<label class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover cursor-pointer
								{itemType === filter.value ? 'bg-accent-soft text-accent-text font-medium' : ''}"
							>
								<input
									type="radio"
									name="item-type"
									value={filter.value}
									checked={itemType === filter.value}
									onchange={() => onItemTypeChange(filter.value)}
									class="w-4 h-4 accent-primary focus:ring-0"
								/>
								<span class="flex w-5 h-5 items-center justify-center">
									<filter.icon class="w-4 h-4" />
								</span>
								<span class="truncate">{filter.label}</span>
							</label>
						{/each}
					</div>
				{/if}
			</div>

			<!-- Favorites Section -->
			<div class="border border-line rounded-lg bg-surface-muted/30">
				<div class="p-3">
					<label class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover cursor-pointer
						{favoritesOnly ? 'bg-accent-soft text-accent-text font-medium' : ''}"
					>
						<input
							type="checkbox"
							checked={favoritesOnly}
							onchange={(e) => onFavoritesOnlyChange(e.target.checked)}
							class="w-4 h-4 accent-primary focus:ring-0"
						/>
						<Star class="w-4 h-4 flex-shrink-0" />
						<span class="truncate">Favorites only</span>
					</label>
				</div>
			</div>

			<!-- Sort Section -->
			<div class="border border-line rounded-lg bg-surface-muted/30">
				<div class="p-3">
					<label class="flex w-full flex-col gap-1.5">
						<span class="text-xs font-medium text-muted-foreground">Sort by</span>
						<select
							bind:value={sort}
							onchange={() => onSortChange(sort)}
							class="w-full px-2 py-1.5 text-sm text-ink bg-surface border border-line rounded control appearance-none cursor-pointer focus:outline-none focus:ring-1 focus:ring-primary"
						>
							{#each sortOptions as option (option.value)}
								<option value={option.value}>{option.label}</option>
							{/each}
						</select>
					</label>
				</div>
			</div>
		</div>

		<!-- Collapse toggle at bottom -->
		<button
			class="mx-2 mb-2 flex w-full items-center justify-center gap-2 px-3 py-2 text-sm font-medium text-muted-foreground hover:text-ink hover:bg-surface-hover rounded-lg transition-colors"
			onclick={toggleCollapse}
			aria-label={collapsed ? "Expand filters" : "Collapse filters"}
			aria-expanded={!collapsed}
		>
			{#if collapsed}
				<ChevronRight class="w-4 h-4 shrink-0" />
			{:else}
				<ChevronLeft class="w-4 h-4 shrink-0" />
			{/if}
			{#if !collapsed}
				<span class="truncate">{collapsed ? "Expand" : "Collapse"}</span>
			{/if}
		</button>
	</nav>
{:else}
	<!-- Overlay mode: fixed sidebar for mobile drawer -->
	<nav
		aria-label="Vault filters"
		class="fixed left-0 top-0 z-50 h-full flex flex-col bg-surface border-r border-line transition-all duration-200 ease-out
			{collapsed ? 'w-16' : 'w-64'}
			{drawerOpen ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}"
		style:--filter-width={collapsed ? "64px" : "240px"}
	>
		<div class="flex h-14 items-center px-3 border-b border-line shrink-0">
			<div class="flex items-center gap-2">
				<Funnel class="w-5 h-5 text-muted-foreground" />
				{#if !collapsed}
					<h2 class="text-base font-semibold text-ink truncate">Filters</h2>
				{/if}
			</div>
		</div>

		<div class="flex-1 overflow-y-auto p-2 space-y-3">
			<!-- Folders Section -->
			<div class="border border-line rounded-lg bg-surface-muted/30">
				<div class="px-3 py-2 border-b border-line">
					{@render sectionHeader("Folders", foldersOpen, () => foldersOpen = !foldersOpen, folders.length)}
				</div>
				{#if foldersOpen}
					<div class="p-2 space-y-1 max-h-48 overflow-y-auto">
						<button
							type="button"
							class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover
								{folderFilter === 'all' ? 'bg-accent-soft text-accent-text font-medium' : ''}"
							onclick={() => { onFolderFilterChange("all"); closeDrawer(); }}
						>
							<span class="flex w-5 h-5 items-center justify-center">
								<FolderKanban class="w-4 h-4" />
							</span>
							{#if !collapsed}<span class="truncate">All folders</span>{/if}
						</button>
						<button
							type="button"
							class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover
								{folderFilter === 'unfiled' ? 'bg-accent-soft text-accent-text font-medium' : ''}"
							onclick={() => { onFolderFilterChange("unfiled"); closeDrawer(); }}
						>
							<span class="flex w-5 h-5 items-center justify-center text-muted-foreground">
								<svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path><circle cx="12" cy="7" r="4"></circle></svg>
							</span>
							{#if !collapsed}<span class="truncate">Unfiled</span>{/if}
						</button>
						{#each folders as folder (folder.id)}
							<button
								type="button"
								class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover
									{folderFilter === folder.id ? 'bg-accent-soft text-accent-text font-medium' : ''}"
								onclick={() => { onFolderFilterChange(folder.id); closeDrawer(); }}
							>
								<span class="flex w-5 h-5 items-center justify-center">
									<FolderKanban class="w-4 h-4" />
								</span>
								{#if !collapsed}<span class="truncate">{folder.name}</span>{/if}
							</button>
						{/each}
						{#if folders.length === 0}
							<p class="px-2 py-2 text-xs text-muted-foreground text-center">No folders yet</p>
						{/if}
					</div>
				{/if}
			</div>

			<!-- Types Section -->
			<div class="border border-line rounded-lg bg-surface-muted/30">
				<div class="px-3 py-2 border-b border-line">
					{@render sectionHeader("Types", typesOpen, () => typesOpen = !typesOpen)}
				</div>
				{#if typesOpen}
					<div class="p-2 space-y-1">
						{#each typeFilters as filter (filter.value)}
							<label class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover cursor-pointer
								{itemType === filter.value ? 'bg-accent-soft text-accent-text font-medium' : ''}"
							>
								<input
									type="radio"
									name="item-type"
									value={filter.value}
									checked={itemType === filter.value}
									onchange={() => { onItemTypeChange(filter.value); closeDrawer(); }}
									class="w-4 h-4 accent-primary focus:ring-0"
								/>
								<span class="flex w-5 h-5 items-center justify-center">
									<filter.icon class="w-4 h-4" />
								</span>
								{#if !collapsed}<span class="truncate">{filter.label}</span>{/if}
							</label>
						{/each}
					</div>
				{/if}
			</div>

			<!-- Favorites Section -->
			<div class="border border-line rounded-lg bg-surface-muted/30">
				<div class="p-3">
					<label class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-ink rounded transition-colors hover:bg-surface-hover cursor-pointer
						{favoritesOnly ? 'bg-accent-soft text-accent-text font-medium' : ''}"
					>
						<input
							type="checkbox"
							checked={favoritesOnly}
							onchange={(e) => { onFavoritesOnlyChange(e.target.checked); closeDrawer(); }}
							class="w-4 h-4 accent-primary focus:ring-0"
						/>
						<Star class="w-4 h-4 flex-shrink-0" />
						{#if !collapsed}<span class="truncate">Favorites only</span>{/if}
					</label>
				</div>
			</div>

			<!-- Sort Section -->
			<div class="border border-line rounded-lg bg-surface-muted/30">
				<div class="p-3">
					<label class="flex w-full flex-col gap-1.5">
						{#if !collapsed}<span class="text-xs font-medium text-muted-foreground">Sort by</span>{/if}
						<select
							bind:value={sort}
							onchange={() => { onSortChange(sort); closeDrawer(); }}
							class="w-full px-2 py-1.5 text-sm text-ink bg-surface border border-line rounded control appearance-none cursor-pointer focus:outline-none focus:ring-1 focus:ring-primary"
						>
							{#each sortOptions as option (option.value)}
								<option value={option.value}>{option.label}</option>
							{/each}
						</select>
					</label>
				</div>
			</div>
		</div>

		<!-- Collapse toggle -->
		<button
			class="mx-2 mb-2 flex w-full items-center justify-center gap-2 px-3 py-2 text-sm font-medium text-muted-foreground hover:text-ink hover:bg-surface-hover rounded-lg transition-colors"
			onclick={toggleCollapse}
			aria-label={collapsed ? "Expand filters" : "Collapse filters"}
			aria-expanded={!collapsed}
		>
			{#if collapsed}
				<ChevronRight class="w-4 h-4 shrink-0" />
			{:else}
				<ChevronLeft class="w-4 h-4 shrink-0" />
			{/if}
			{#if !collapsed}
				<span class="truncate">{collapsed ? "Expand" : "Collapse"}</span>
			{/if}
		</button>

		{#if drawerOpen}
			<button
				class="fixed inset-0 z-40 bg-black/50 lg:hidden"
				role="dialog"
				aria-modal="true"
				aria-label="Vault filters"
				onclick={handleOverlayClick}
				onkeydown={handleOverlayKeydown}
				tabindex="-1"
			></button>
		{/if}

		<button
			class="lg:hidden fixed bottom-4 right-4 z-50 p-2 rounded-full bg-surface border border-line shadow-lg"
			onclick={openDrawer}
			aria-label="Open filters"
			aria-expanded={drawerOpen}
		>
			<Funnel class="w-6 h-6" />
		</button>
	</nav>
{/if}
