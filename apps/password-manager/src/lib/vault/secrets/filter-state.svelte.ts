import type { ItemTypeFilter, ItemSort } from "./search";

export interface FilterState {
	searchText: string;
	itemType: ItemTypeFilter;
	favoritesOnly: boolean;
	sort: ItemSort;
}

export function createFilterState(initial?: Partial<FilterState>) {
	let searchText = $state(initial?.searchText ?? "");
	let itemType = $state<ItemTypeFilter>(initial?.itemType ?? "all");
	let favoritesOnly = $state(initial?.favoritesOnly ?? false);
	let sort = $state<ItemSort>(initial?.sort ?? "favorites");

	function clearFilters() {
		searchText = "";
		itemType = "all";
		favoritesOnly = false;
		sort = "favorites";
	}

	return {
		get searchText() { return searchText; },
		set searchText(v: string) { searchText = v; },
		get itemType() { return itemType; },
		set itemType(v: ItemTypeFilter) { itemType = v; },
		get favoritesOnly() { return favoritesOnly; },
		set favoritesOnly(v: boolean) { favoritesOnly = v; },
		get sort() { return sort; },
		set sort(v: ItemSort) { sort = v; },
		clearFilters,
	};
}
