<script lang="ts" module>
    import { createStorage } from "@ofsdk/svelte-headless";

    const sidebar = createStorage("management-sidebar-collapsed", false);
    const mobileMenu = createStorage("management-navigation-open", false);
</script>

<script lang="ts">
    import AppWindow from "@lucide/svelte/icons/app-window";
    import Laptop from "@lucide/svelte/icons/laptop";
    import LayoutDashboard from "@lucide/svelte/icons/layout-dashboard";
    import LogOut from "@lucide/svelte/icons/log-out";
    import Menu from "@lucide/svelte/icons/menu";
    import PanelLeftClose from "@lucide/svelte/icons/panel-left-close";
    import PanelLeftOpen from "@lucide/svelte/icons/panel-left-open";
    import Users from "@lucide/svelte/icons/users";

    import type { Component } from "svelte";
    import { afterNavigate, goto } from "$app/navigation";
    import { resolve } from "$app/paths";
    import { page } from "$app/state";
    import { clearOidcSession } from "$lib/common/state/oidc.svelte";
    import type { LayoutProps } from "./$types";

    let { children }: LayoutProps = $props();

    let menuButton = $state<HTMLButtonElement>();
    let sidebarPanel = $state<HTMLElement>();
    let main = $state<HTMLElement>();

    afterNavigate(({ from, to }) => {
        if (from && from.url.pathname !== to?.url.pathname) {
            main?.scrollTo(0, 0);
            main?.focus({ preventScroll: true });
        }
    });

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

    function closeMenuOnEscape(event: KeyboardEvent) {
        if (
            event.key === "Escape" &&
            mobileMenu.item &&
            menuButton?.getClientRects().length &&
            (event.target === menuButton ||
                (event.target instanceof Node && sidebarPanel?.contains(event.target)))
        ) {
            mobileMenu.item = false;
            menuButton.focus();
        }
    }
</script>

<svelte:window onkeydown={closeMenuOnEscape} />

<div class="flex min-h-0 w-full flex-1 flex-col lg:flex-row">
    <div class="flex shrink-0 items-center justify-between border-b border-line bg-surface px-3 py-2 lg:hidden">
        <span class="text-sm font-semibold tracking-tight">Management</span>
        <button
            bind:this={menuButton}
            type="button"
            class="btn btn-ghost btn-md"
            aria-controls="management-sidebar"
            aria-expanded={mobileMenu.item}
            onclick={() => (mobileMenu.item = !mobileMenu.item)}
        >
            <Menu size={18} aria-hidden="true" />
            Navigation
        </button>
    </div>

    <aside
        bind:this={sidebarPanel}
        id="management-sidebar"
        class={[
            "max-h-[40vh] shrink-0 flex-col overflow-y-auto border-b border-line bg-surface lg:flex lg:max-h-none lg:border-r lg:border-b-0",
            mobileMenu.item ? "flex" : "hidden",
            sidebar.item ? "lg:w-17" : "lg:w-60",
        ]}
    >
        <div
            class={[
                "hidden h-14 shrink-0 items-center border-b border-line px-3 lg:flex",
                sidebar.item ? "justify-center" : "justify-between gap-2",
            ]}
        >
            {#if !sidebar.item}
                <span class="truncate text-sm font-semibold tracking-tight">Management</span>
            {/if}

            <button
                type="button"
                class="btn btn-ghost btn-icon shrink-0"
                onclick={() => (sidebar.item = !sidebar.item)}
                aria-label={sidebar.item ? "Expand sidebar" : "Collapse sidebar"}
                title={sidebar.item ? "Expand" : "Collapse"}
            >
                {#if sidebar.item}
                    <PanelLeftOpen size={18} aria-hidden="true" />
                {:else}
                    <PanelLeftClose size={18} aria-hidden="true" />
                {/if}
            </button>
        </div>

        <nav aria-label="Management" class="grid flex-1 grid-cols-2 content-start gap-1 p-2 lg:grid-cols-1">
            {#each links as link (link.path)}
                {@const active = isActive(link.path)}
                <a
                    href={link.href}
                    class={[
                        "flex items-center gap-3 rounded-control px-3 py-2 text-sm font-medium no-underline transition-colors",
                        active
                            ? "bg-accent-soft text-accent-text"
                            : "text-ink-muted hover:bg-surface-hover hover:text-ink",
                        sidebar.item && "lg:justify-center lg:px-0",
                    ]}
                    aria-label={link.label}
                    title={sidebar.item ? link.label : undefined}
                    aria-current={active ? "page" : undefined}
                >
                    <link.icon size={18} class="shrink-0" aria-hidden="true" />
                    <span class={["truncate", sidebar.item && "lg:hidden"]}>{link.label}</span>
                </a>
            {/each}
        </nav>

        <div class="border-t border-line p-2">
            <button
                type="button"
                class={[
                    "flex w-full cursor-pointer items-center gap-3 rounded-control px-3 py-2 text-sm font-medium text-ink-muted transition-colors hover:bg-danger-soft hover:text-danger-text",
                    sidebar.item && "lg:justify-center lg:px-0",
                ]}
                onclick={onSignOut}
                aria-label="Sign out"
                title={sidebar.item ? "Sign out" : undefined}
            >
                <LogOut size={18} class="shrink-0" aria-hidden="true" />
                <span class={sidebar.item ? "lg:hidden" : undefined}>Sign out</span>
            </button>
        </div>
    </aside>

    <main bind:this={main} tabindex="-1" class="@container/management min-h-0 min-w-0 flex-1 overflow-auto py-4 wrap-anywhere focus-visible:-outline-offset-2">
        {@render children()}
    </main>
</div>
