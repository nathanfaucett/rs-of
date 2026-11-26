<script lang="ts">
import { ChevronLeft, ChevronRight, Key, Lock, LogIn, Menu, Settings, Trash2, Vault, X } from "@lucide/svelte";
import { onMount, tick } from "svelte";
import { afterNavigate } from "$app/navigation";
import { resolve } from "$app/paths";
import { page } from "$app/state";
import { authState } from "$lib/auth/auth.svelte";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import NavSidebarItem from "./NavSidebarItem.svelte";

type NavigationView = "vault" | "generator" | "settings" | "trash";

let { onNavigate }: { onNavigate: (view: NavigationView) => void } = $props();

const destinations = [
	{ view: "vault", label: "Vault", icon: vaultIcon },
	{ view: "generator", label: "Generator", icon: generatorIcon },
	{ view: "settings", label: "Settings", icon: settingsIcon },
	{ view: "trash", label: "Trash", icon: trashIcon },
] as const;
const currentView = $derived(page.url.pathname.split("/")[1] || "vault");
const isUnlocked = $derived(vaultStateModule.vaultData !== null);

let collapsed = $state(false);
let drawerOpen = $state(false);
let drawer: HTMLDialogElement;
let desktopNav: HTMLElement;

onMount(() => {
	const desktop = window.matchMedia("(min-width: 64rem)");
	const stored = localStorage.getItem("pm:nav:collapsed");
	collapsed = stored === null ? window.innerWidth < 1024 : stored === "true";

	function handleBreakpoint(event: MediaQueryListEvent) {
		if (!event.matches || !drawerOpen) return;
		closeDrawer();
		(desktopNav.querySelector<HTMLElement>('[aria-current="page"]') ??
			desktopNav.querySelector('button'))?.focus();
	}

	desktop.addEventListener("change", handleBreakpoint);
	return () => desktop.removeEventListener("change", handleBreakpoint);
});

afterNavigate(closeDrawer);

function toggleCollapse() {
	collapsed = !collapsed;
	localStorage.setItem("pm:nav:collapsed", String(collapsed));
}

function handleNavigate(view: NavigationView) {
	closeDrawer();
	onNavigate(view);
}

function handleKeydown(event: KeyboardEvent, index: number) {
	const buttons = (event.currentTarget as HTMLButtonElement).closest("ul")?.querySelectorAll("button");
	let next: number;
	switch (event.key) {
		case "ArrowDown": next = index + 1; break;
		case "ArrowUp": next = index - 1; break;
		case "Home": next = 0; break;
		case "End": next = destinations.length - 1; break;
		default: return;
	}
	event.preventDefault();
	buttons?.[next]?.focus();
}

async function openDrawer(event: MouseEvent & { currentTarget: HTMLButtonElement }) {
	// Native dialog restores the previously focused element, even after a pointer open.
	event.currentTarget.focus({ preventScroll: true });
	drawerOpen = true;
	await tick();
	if (!drawerOpen) return;
	drawer.showModal();
	drawer.querySelector<HTMLButtonElement>('button[aria-current="page"]')?.focus();
}

function closeDrawer() {
	drawer?.close();
	drawerOpen = false;
}

function wrapDrawerTab(event: KeyboardEvent) {
	if (event.key !== "Tab") return;
	const controls = drawer.querySelectorAll<HTMLElement>("button, a[href]");
	const first = controls[0];
	const last = controls[controls.length - 1];
	if (event.target !== (event.shiftKey ? first : last)) return;
	event.preventDefault();
	(event.shiftKey ? last : first)?.focus();
}
</script>

{#snippet vaultIcon()}<Vault class="w-5 h-5" />{/snippet}
{#snippet generatorIcon()}<Key class="w-5 h-5" />{/snippet}
{#snippet settingsIcon()}<Settings class="w-5 h-5" />{/snippet}
{#snippet trashIcon()}<Trash2 class="w-5 h-5" />{/snippet}

{#snippet navigation(compact: boolean, mobile: boolean)}
	<div class="flex h-16 items-center justify-between gap-2 px-4 border-b border-line shrink-0">
		{#if compact}
			<span class="text-lg font-semibold text-ink" aria-hidden="true">PM</span>
		{:else if mobile}
			<h2 class="text-lg font-semibold text-ink truncate">Password Manager</h2>
		{:else}
			<h1 class="text-lg font-semibold text-ink truncate">Password Manager</h1>
		{/if}
		{#if mobile}
			<button class="btn btn-ghost btn-md shrink-0" type="button" aria-label="Close navigation" onclick={closeDrawer}>
				<X class="w-5 h-5" />
			</button>
		{/if}
	</div>

	<ul class="flex-1 flex flex-col gap-1 p-3 overflow-y-auto">
		{#each destinations as { view, label, icon }, index (view)}
			<li>
				<NavSidebarItem {icon} {label} active={currentView === view} collapsed={compact}
					onclick={() => handleNavigate(view)} onkeydown={(event) => handleKeydown(event, index)} />
			</li>
		{/each}
	</ul>

	{#if isUnlocked}
		<div class="grid gap-2 border-t border-line p-3">
			{#if !authState.isAuthenticated && !vaultStateModule.vaultData?.isSynced}
				<a class={['btn btn-secondary btn-md w-full min-w-0', compact && 'px-0']}
					href={resolve("/login")} aria-label="Sign in to sync" title={compact ? "Sign in to sync" : undefined}>
					<LogIn class="w-5 h-5 shrink-0" />
					{#if !compact}<span class="truncate">Sign in to sync</span>{/if}
				</a>
			{/if}
			<button class={['btn btn-secondary btn-md w-full min-w-0', compact && 'px-0']} type="button"
				onclick={() => void vaultStateModule.lockVault()} aria-label="Lock vault" title={compact ? "Lock vault" : undefined}>
				<Lock class="w-5 h-5 shrink-0" />
				{#if !compact}<span class="truncate">Lock vault</span>{/if}
			</button>
		</div>
	{/if}
{/snippet}

<nav aria-label="Main navigation" bind:this={desktopNav}
	class={['hidden lg:flex fixed left-0 top-0 z-50 h-full flex-col bg-surface border-r border-line transition-all duration-200 ease-out', collapsed ? 'w-16' : 'w-64']}>
	{@render navigation(collapsed, false)}
	<button
		class="mx-3 mb-3 flex items-center justify-center gap-2 px-3 py-2 text-sm font-medium text-muted-foreground hover:text-ink hover:bg-surface-hover rounded-lg transition-colors"
		type="button" onclick={toggleCollapse} aria-label={collapsed ? "Expand navigation" : "Collapse navigation"} aria-expanded={!collapsed}>
		{#if collapsed}
			<ChevronRight class="w-5 h-5 shrink-0" />
		{:else}
			<ChevronLeft class="w-5 h-5 shrink-0" />
			<span class="truncate">Collapse</span>
		{/if}
	</button>
</nav>

<header class="lg:hidden flex items-center border-b border-line bg-surface px-4 py-2">
	<button class="btn btn-secondary btn-md" type="button" onclick={openDrawer}
		aria-label="Open navigation" aria-haspopup="dialog" aria-controls="mobile-navigation" aria-expanded={drawerOpen}>
		<Menu class="w-6 h-6" />
	</button>
</header>

<dialog id="mobile-navigation" bind:this={drawer} aria-label="Main navigation"
	class="fixed inset-y-0 left-0 m-0 h-dvh max-h-none w-64 max-w-full border-r border-line bg-surface text-ink backdrop:bg-black/50"
	onclose={() => { if (!drawer.open) drawerOpen = false; }}
	onclick={(event) => { if (event.target === drawer) closeDrawer(); }}
	onkeydown={wrapDrawerTab}>
	{#if drawerOpen}
		<nav aria-label="Main navigation" class="flex h-full flex-col">
			{@render navigation(false, true)}
		</nav>
	{/if}
</dialog>
