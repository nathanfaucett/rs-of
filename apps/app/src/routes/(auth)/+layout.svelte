<script lang="ts">
    import AppWindow from "@lucide/svelte/icons/app-window";
    import Laptop from "@lucide/svelte/icons/laptop";
    import LayoutDashboard from "@lucide/svelte/icons/layout-dashboard";
    import LogOut from "@lucide/svelte/icons/log-out";
    import PanelLeftClose from "@lucide/svelte/icons/panel-left-close";
    import PanelLeftOpen from "@lucide/svelte/icons/panel-left-open";
    import Users from "@lucide/svelte/icons/users";

    import type { Component } from "svelte";
    import { goto } from "$app/navigation";
    import { resolve } from "$app/paths";
    import { page } from "$app/state";
    import { clearOidcSession } from "$lib/common/state/oidc.svelte";
    import type { LayoutProps } from "./$types";

    let { children }: LayoutProps = $props();

    let collapsed = $state(false);

    const links: {
        label: string;
        href: string;
        path: string;
        icon: Component;
    }[] = [
        {
            label: "Dashboard",
            href: resolve("/"),
            path: "/",
            icon: LayoutDashboard,
        },
        {
            label: "Applications",
            href: resolve("/applications"),
            path: "/applications",
            icon: AppWindow,
        },
        {
            label: "Devices",
            href: resolve("/devices"),
            path: "/devices",
            icon: Laptop,
        },
        {
            label: "Users",
            href: resolve("/users"),
            path: "/users",
            icon: Users,
        },
    ];

    function isActive(path: string): boolean {
        if (path === "/") return page.url.pathname === "/";
        return (
            page.url.pathname === path ||
            page.url.pathname.startsWith(`${path}/`)
        );
    }

    async function onSignOut() {
        clearOidcSession();
        await goto(resolve("/signin"));
    }

    function toggleSidebar() {
        collapsed = !collapsed;
    }
</script>

<div class="flex h-full w-full grow overflow-hidden">
    <aside
        class="flex shrink-0 flex-col border-r border-line bg-surface transition-[width] duration-200 ease-out
           {collapsed ? 'w-17' : 'w-60'}"
    >
        <div
            class="flex h-14 items-center border-b border-line px-3
             {collapsed ? 'justify-center' : 'justify-between gap-2'}"
        >
            {#if !collapsed}
                <span class="truncate text-sm font-semibold tracking-tight">
                    Management
                </span>
            {/if}

            <button
                type="button"
                class="btn btn-ghost btn-icon shrink-0"
                onclick={toggleSidebar}
                aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
                title={collapsed ? "Expand" : "Collapse"}
            >
                {#if collapsed}
                    <PanelLeftOpen size={18} aria-hidden="true" />
                {:else}
                    <PanelLeftClose size={18} aria-hidden="true" />
                {/if}
            </button>
        </div>

        <nav class="flex flex-1 flex-col gap-1 p-2">
            {#each links as link (link.path)}
                {@const active = isActive(link.path)}
                <a
                    href={link.href}
                    class="flex items-center gap-3 rounded-control px-3 py-2 text-sm font-medium no-underline transition-colors
                 {active
                        ? 'bg-accent-soft text-accent-text'
                        : 'text-ink-muted hover:bg-surface-hover hover:text-ink'}
                 {collapsed ? 'justify-center px-0' : ''}"
                    title={collapsed ? link.label : undefined}
                    aria-current={active ? "page" : undefined}
                >
                    <link.icon size={18} class="shrink-0" aria-hidden="true" />
                    {#if !collapsed}
                        <span class="truncate">{link.label}</span>
                    {/if}
                </a>
            {/each}
        </nav>

        <div class="border-t border-line p-2">
            <button
                type="button"
                class="flex w-full cursor-pointer items-center gap-3 rounded-control px-3 py-2 text-sm font-medium text-ink-muted transition-colors
               hover:bg-danger-soft hover:text-danger-text
               {collapsed ? 'justify-center px-0' : ''}"
                onclick={onSignOut}
                title={collapsed ? "Sign out" : undefined}
            >
                <LogOut size={18} class="shrink-0" aria-hidden="true" />
                {#if !collapsed}
                    <span>Sign out</span>
                {/if}
            </button>
        </div>
    </aside>

    <section class="flex min-w-0 grow flex-col overflow-hidden">
        <div class="min-h-0 grow overflow-auto">
            {@render children()}
        </div>
    </section>
</div>
