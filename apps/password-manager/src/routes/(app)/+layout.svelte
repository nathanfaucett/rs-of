<script lang="ts">
import { onMount } from "svelte";
import { goto } from "$app/navigation";
import { resolve } from "$app/paths";
import { page } from "$app/state";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import type { LayoutProps } from "./$types";
import NavSidebar from "./layout/NavSidebar.svelte";

type NavigationView = "vault" | "generator" | "settings" | "trash";

let { children }: LayoutProps = $props();
let vaultData = $derived(vaultStateModule.vaultData);
let vaultStatus = $derived(vaultStateModule.state.status);

// Initialize vault state if not already initialized
onMount(() => {
    if (vaultStatus === "loading") {
        void vaultStateModule.init();
    }
});

// Redirect to root if vault is locked and accessing protected route
$effect(() => {
    const isProtected = page.url.pathname.startsWith("/vault") ||
        page.url.pathname.startsWith("/settings") ||
        page.url.pathname.startsWith("/trash") ||
        page.url.pathname.startsWith("/generator");
    if (!isProtected) return;
    // Don't redirect while loading - wait for vault to initialize
    if (vaultStatus === "loading") return;
    // Redirect if vault is not unlocked (select-vault, locked, create-vault, etc.)
    if (!vaultData) {
        // Only redirect if we're not already on the root
        if (page.url.pathname !== "/") {
            void goto(resolve("/"));
        }
    }
});</script>

<div class="flex min-h-screen bg-canvas">
    <NavSidebar onNavigate={(view: NavigationView) => void goto(resolve(`/${view}`))} />

    <main class="flex-1 min-h-0 flex flex-col lg:ml-64">
        <div class="flex-1 flex flex-col min-h-0">
            <div class="flex-1 min-h-0 overflow-auto flex">
                <div class="flex-1 min-h-0 overflow-auto">
                    {@render children()}
                </div>
            </div>
        </div>
    </main>
</div>
