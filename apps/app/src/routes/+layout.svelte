<script lang="ts" module>
    import "./layout.css";
</script>

<script lang="ts">
    import Settings from "@lucide/svelte/icons/settings";
    import { getTheme } from "@ofsdk/svelte-headless";
    import { isTauri } from "@tauri-apps/api/core";
    import { onMount } from "svelte";
    import { resolve } from "$app/paths";
    import { page } from "$app/state";
    import type { RouteId } from "$app/types";
    import favicon from "$lib/assets/favicon.svg";
    import ConfigForm from "$lib/common/components/ConfigForm.svelte";
    import Modal from "$lib/common/components/Modal.svelte";
    import Notifications from "$lib/common/components/Notifications.svelte";
    import { handleDeepLink } from "$lib/common/util/handleDeepLink";
    import { m } from "$lib/paraglide/messages";
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
    let configForm = $state<ConfigForm>();
    let configModal = $state<Modal>();

    function showConfig() {
        configForm?.reset();
        configModal?.show();
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

<button
    type="button"
    class="btn btn-secondary btn-icon fixed right-4 bottom-4 z-10 rounded-full shadow-lg"
    aria-label={m.env_config()}
    title={m.env_config()}
    onclick={showConfig}
>
    <Settings aria-hidden="true" />
</button>

<Modal bind:this={configModal} title={m.env_config()}>
    <ConfigForm bind:this={configForm} onSaved={() => configModal?.close()} />
    {#if isTauri()}
        <a
            class="btn btn-secondary"
            href={resolve("/maintenance")}
            onclick={() => configModal?.close()}>Local Node maintenance</a
        >
    {/if}
</Modal>

<Notifications />
