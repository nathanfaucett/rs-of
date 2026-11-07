<script lang="ts">
import { onMount } from "svelte";
import { browser } from "$app/environment";
import FilterSidebarItem from "./FilterSidebarItem.svelte";
import type { Snippet } from "svelte";
import { FolderKanban, Grid, Star, ChevronLeft, ChevronRight, Filter } from "@lucide/svelte";

let { onFilterChange }: { onFilterChange: (filter: string) => void } = $props();

let collapsed = $state(false);
let drawerOpen = $state(false);

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

function handleFilterChange(filter: string) {
	onFilterChange(filter);
	if (drawerOpen) drawerOpen = false;
}

function handleKeydown(event: KeyboardEvent, index: number) {
	switch (event.key) {
		case "ArrowDown":
			event.preventDefault();
			if (index < 2) {
				(event.target as HTMLElement).parentElement?.querySelectorAll("button")?.[index + 1]?.focus();
			}
			break;
		case "ArrowUp":
			event.preventDefault();
			if (index > 0) {
				(event.target as HTMLElement).parentElement?.querySelectorAll("button")?.[index - 1]?.focus();
			}
			break;
		case "Home":
			event.preventDefault();
			(event.target as HTMLElement).parentElement?.querySelector("button")?.focus();
			break;
		case "End":
			event.preventDefault();
			(event.target as HTMLElement).parentElement?.querySelectorAll("button")?.[2]?.focus();
			break;
		case "Escape":
			if (drawerOpen) {
				event.preventDefault();
				closeDrawer();
			}
			break;
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

{#snippet foldersIcon()}
	<FolderKanban class="w-5 h-5" />
{/snippet}

{#snippet typesIcon()}
	<Grid class="w-5 h-5" />
{/snippet}

{#snippet favoritesIcon()}
	<Star class="w-5 h-5" />
{/snippet}

<nav
	aria-label="Vault filters"
	class="fixed left-0 top-0 z-50 h-full flex flex-col bg-surface border-r border-line transition-all duration-200 ease-out
		{collapsed ? 'w-16' : 'w-64'}
		{drawerOpen ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}"
	style:--filter-width={collapsed ? "64px" : "240px"}
>
	<div class="flex h-16 items-center px-4 border-b border-line shrink-0">
		{#if !collapsed}
			<h2 class="text-lg font-semibold text-ink truncate">Filters</h2>
		{:else}
			<span class="text-lg font-semibold text-ink" aria-hidden="true">F</span>
		{/if}
	</div>

	<ul role="menu" class="flex-1 flex flex-col gap-1 p-3 overflow-y-auto" tabindex="0">
		<li>
			<FilterSidebarItem
				icon={foldersIcon}
				label="Folders"
				{collapsed}
				onclick={() => handleFilterChange("folders")}
				onkeydown={(e) => handleKeydown(e, 0)}
			/>
		</li>
		<li>
			<FilterSidebarItem
				icon={typesIcon}
				label="Types"
				{collapsed}
				onclick={() => handleFilterChange("types")}
				onkeydown={(e) => handleKeydown(e, 1)}
			/>
		</li>
		<li>
			<FilterSidebarItem
				icon={favoritesIcon}
				label="Favorites"
				{collapsed}
				onclick={() => handleFilterChange("favorites")}
				onkeydown={(e) => handleKeydown(e, 2)}
			/>
		</li>
	</ul>

	<button
		class="mx-3 mb-3 flex w-full items-center justify-center gap-2 px-3 py-2 text-sm font-medium text-muted-foreground hover:text-ink hover:bg-surface-hover rounded-lg transition-colors"
		onclick={toggleCollapse}
		aria-label={collapsed ? "Expand filters" : "Collapse filters"}
		aria-expanded={!collapsed}
	>
		{#if collapsed}
			<ChevronRight class="w-5 h-5 shrink-0" />
		{:else}
			<ChevronLeft class="w-5 h-5 shrink-0" />
		{/if}
		{#if !collapsed}
			<span class="truncate">
				{collapsed ? "Expand" : "Collapse"}
			</span>
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
		<Filter class="w-6 h-6" />
	</button>
</nav>
