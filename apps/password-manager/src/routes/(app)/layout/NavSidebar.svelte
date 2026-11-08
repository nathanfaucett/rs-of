<script lang="ts">
import { onMount } from "svelte";
import { browser } from "$app/environment";
import { page } from "$app/state";
import { goto } from "$app/navigation";
import NavSidebarItem from "./NavSidebarItem.svelte";
import type { Snippet } from "svelte";
import { Vault, Key, Settings, Trash2, ChevronLeft, ChevronRight, Menu, Lock } from "@lucide/svelte";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";

let { onNavigate }: { onNavigate: (view: string) => void } = $props();

const currentView = $derived(page.url.pathname.split("/")[1] || "vault");
const isUnlocked = $derived(vaultStateModule.vaultData !== null);

let collapsed = $state(false);
let drawerOpen = $state(false);

async function handleLockVault() {
    await vaultStateModule.lockVault();
}

function initCollapsed() {
	if (!browser) return;
	const stored = localStorage.getItem("pm:nav:collapsed");
	if (stored !== null) {
		collapsed = stored === "true";
	} else {
		collapsed = window.innerWidth < 1024;
	}
}

function toggleCollapse() {
	collapsed = !collapsed;
	if (browser) {
		localStorage.setItem("pm:nav:collapsed", String(collapsed));
	}
}

function handleNavigate(view: string) {
	onNavigate(view);
	if (drawerOpen) drawerOpen = false;
}

function handleKeydown(event: KeyboardEvent, index: number) {
	switch (event.key) {
		case "ArrowDown":
			event.preventDefault();
			if (index < 3) {
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
			(event.target as HTMLElement).parentElement?.querySelectorAll("button")?.[3]?.focus();
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

{#snippet vaultIcon()}
	<Vault class="w-5 h-5" />
{/snippet}

{#snippet generatorIcon()}
	<Key class="w-5 h-5" />
{/snippet}

{#snippet settingsIcon()}
	<Settings class="w-5 h-5" />
{/snippet}

{#snippet trashIcon()}
	<Trash2 class="w-5 h-5" />
{/snippet}

<nav
	aria-label="Main navigation"
	class="fixed left-0 top-0 z-50 h-full flex flex-col bg-surface border-r border-line transition-all duration-200 ease-out
		{collapsed ? 'w-16' : 'w-64'}
		{drawerOpen ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}"
>
	<div class="flex h-16 items-center px-4 border-b border-line shrink-0">
		{#if !collapsed}
			<h1 class="text-lg font-semibold text-ink truncate">Password Manager</h1>
		{:else}
			<span class="text-lg font-semibold text-ink" aria-hidden="true">PM</span>
		{/if}
	</div>

	<ul role="menu" class="flex-1 flex flex-col gap-1 p-3 overflow-y-auto" tabindex="0">
	            <li>
	                <NavSidebarItem
	                    icon={vaultIcon}
	                    label="Vault"
	                    active={currentView === "vault"}
	                    {collapsed}
	                    onclick={() => handleNavigate("vault")}
	                    onkeydown={(e) => handleKeydown(e, 0)}
	                />
	            </li>
	            <li>
	                <NavSidebarItem
	                    icon={generatorIcon}
	                    label="Generator"
	                    active={currentView === "generator"}
	                    {collapsed}
	                    onclick={() => handleNavigate("generator")}
	                    onkeydown={(e) => handleKeydown(e, 1)}
	                />
	            </li>
	            <li>
	                <NavSidebarItem
	                    icon={settingsIcon}
	                    label="Settings"
	                    active={currentView === "settings"}
	                    {collapsed}
	                    onclick={() => handleNavigate("settings")}
	                    onkeydown={(e) => handleKeydown(e, 2)}
	                />
	            </li>
	            <li>
	                <NavSidebarItem
	                    icon={trashIcon}
	                    label="Trash"
	                    active={currentView === "trash"}
	                    {collapsed}
	                    onclick={() => handleNavigate("trash")}
	                    onkeydown={(e) => handleKeydown(e, 3)}
	                />
	            </li>
	        </ul>

	        {#if isUnlocked}
	            <div class="px-3 pb-3 border-t border-line">
	                <button
	                    class="w-full flex items-center justify-center gap-2 px-3 py-2 text-sm font-medium text-destructive hover:text-destructive/80 hover:bg-destructive/10 rounded-lg transition-colors"
	                    onclick={handleLockVault}
	                    aria-label="Lock vault"
	                >
	                    <Lock class="w-5 h-5 shrink-0" />
	                    {#if !collapsed}
	                        <span class="truncate">Lock vault</span>
	                    {/if}
	                </button>
	            </div>
	        {/if}

	        <button
		class="mx-3 mb-3 flex w-full items-center justify-center gap-2 px-3 py-2 text-sm font-medium text-muted-foreground hover:text-ink hover:bg-surface-hover rounded-lg transition-colors"
		onclick={toggleCollapse}
		aria-label={collapsed ? "Expand navigation" : "Collapse navigation"}
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
			aria-label="Navigation menu"
			onclick={handleOverlayClick}
			onkeydown={handleOverlayKeydown}
			tabindex="-1"
		></button>
	{/if}

	<button
		class="lg:hidden fixed bottom-4 left-4 z-50 p-2 rounded-full bg-surface border border-line shadow-lg"
		onclick={openDrawer}
		aria-label="Open navigation"
		aria-expanded={drawerOpen}
	>
		<Menu class="w-6 h-6" />
	</button>
</nav>
