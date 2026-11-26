<script lang="ts" module>
    import "./layout.css";
</script>

<script lang="ts">
    import { getTheme } from "@ofsdk/svelte-headless";
    import { isTauri } from "@tauri-apps/api/core";
    import { onMount } from "svelte";
    import { resolve } from "$app/paths";
    import { page } from "$app/state";
    import type { RouteId } from "$app/types";
    import favicon from "$lib/assets/favicon.svg";
    import ConfigButton from "$lib/common/components/ConfigButton.svelte";
    import Notifications from "$lib/common/components/Notifications.svelte";
    import { createdClients } from "$lib/common/state/createdClients.svelte";
    import { handleDeepLink } from "$lib/common/util/handleDeepLink";
    import type { LayoutProps } from "./$types";

    const routeTitles: Partial<Record<RouteId, string>> = {
        "/(auth)": "Dashboard",
        "/(auth)/applications": "Applications",
        "/(auth)/authorize": "Authorize",
        "/(auth)/clients": "Clients",
        "/(auth)/devices": "Devices",
        "/(auth)/register": "Register Client",
        "/(auth)/roles": "Roles",
        "/(auth)/users": "Users",
        "/callback": "Complete sign-in",
        "/config": "Configuration",
        "/logout": "Sign out",
        "/maintenance": "Local Node maintenance",
        "/native-callback": "Complete native sign-in",
        "/policy": "Privacy policy",
        "/setup": "Node setup",
        "/setup/installation": "Installation setup",
        "/setup/join": "Join Installation",
        "/signin": "Sign in",
        "/terms": "Terms of service",
    };
    const title = $derived(page.route.id && routeTitles[page.route.id]);

    let { children }: LayoutProps = $props();

    function onSessionStorage({ key, storageArea }: StorageEvent) {
        if (
            storageArea === localStorage &&
            (key === null ||
                (key.startsWith("oidc.token:") && key.endsWith(":response")))
        ) {
            createdClients.clear();
        }
    }

    $effect(() => {
        if (getTheme() === "dark") {
            document.documentElement.classList.add("dark");
            return;
        }

        document.documentElement.classList.remove("dark");
    });

    onMount(() => {
        if (!isTauri()) {
            return;
        }

        let onOpenUrlUnlistenFn: (() => void) | undefined;

        import("@tauri-apps/plugin-deep-link").then(({ onOpenUrl }) =>
            onOpenUrl(handleDeepLink).then((unlisten) => {
                onOpenUrlUnlistenFn = unlisten;
            }),
        );

        return () => {
            onOpenUrlUnlistenFn?.();
        };
    });
</script>

<svelte:window onstorage={onSessionStorage} />

<svelte:head>
    {#if title && !page.error}
        <title>{title} — of</title>
    {/if}
    <link rel="icon" href={favicon} />
    <link
        rel="manifest"
        crossorigin="use-credentials"
        href={resolve("/manifest.json")}
    />
</svelte:head>

{@render children()}

{#if page.error || !page.route.id?.startsWith("/(auth)")}
    <ConfigButton floating />
{/if}

<Notifications />
