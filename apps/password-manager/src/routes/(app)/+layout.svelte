<script lang="ts">
import { page } from "$app/state";
import NavSidebar from "./layout/NavSidebar.svelte";
import FilterSidebar from "./layout/FilterSidebar.svelte";
import type { Snippet } from "svelte";

interface Props {
	children: Snippet;
}

let { children }: Props = $props();

// Determine if we're in vault view for filter sidebar
const isVault = $derived(page.url.pathname.startsWith("/vault"));

function setActiveView(view: string) {
	location.href = `/${view}`;
}

function setActiveFilter(filter: string) {
	// Filter is handled via URL search params or component state
	console.log("Filter:", filter);
}
</script>

<div class="flex min-h-screen bg-canvas">
	<NavSidebar onNavigate={setActiveView} />

	<main class="flex-1 min-h-0 flex flex-col lg:ml-64">
		<div class="flex-1 flex flex-col min-h-0">
			{#if isVault}
				<div class="lg:hidden">
					<FilterSidebar onFilterChange={setActiveFilter} />
				</div>
			{/if}
			<div class="flex-1 min-h-0 overflow-auto flex">
				{#if isVault}
					<FilterSidebar onFilterChange={setActiveFilter} />
				{/if}
				<div class="flex-1 min-h-0 overflow-auto">
					{@render children()}
				</div>
			</div>
		</div>
	</main>
</div>
