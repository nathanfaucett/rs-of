<script lang="ts" module>
    import "./layout.css";
</script>

<script lang="ts">
    import Settings from "@lucide/svelte/icons/settings";
    import { getTheme } from "@ofsdk/svelte-headless";
    import { isTauri } from "@tauri-apps/api/core";
    import { onMount } from "svelte";
    import { resolve } from "$app/paths";
    import favicon from "$lib/assets/favicon.svg";
    import ConfigForm from "$lib/common/components/ConfigForm.svelte";
    import Modal from "$lib/common/components/Modal.svelte";
    import Notifications from "$lib/common/components/Notifications.svelte";
    import { handleDeepLink } from "$lib/common/util/handleDeepLink";
    import { m } from "$lib/paraglide/messages";
    import type { LayoutProps } from "./$types";

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
    <link rel="icon" href={favicon} />
    <link
        rel="manifest"
        crossorigin="use-credentials"
        href={resolve("/manifest.json")}
    />
</svelte:head>

{@render children()}

<div class="flex shrink-0 justify-end border-t border-line bg-surface px-3 py-1">
    <button
        type="button"
        class="btn btn-secondary btn-icon"
        aria-label={m.env_config()}
        title={m.env_config()}
        onclick={showConfig}
    >
        <Settings aria-hidden="true" />
    </button>
</div>

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
