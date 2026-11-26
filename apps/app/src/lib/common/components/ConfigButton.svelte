<script lang="ts">
    import Settings from "@lucide/svelte/icons/settings";
    import { isTauri } from "@tauri-apps/api/core";
    import { resolve } from "$app/paths";
    import ConfigForm from "$lib/common/components/ConfigForm.svelte";
    import Modal from "$lib/common/components/Modal.svelte";
    import { m } from "$lib/paraglide/messages";

    interface Props {
        floating?: boolean;
        collapsed?: boolean;
    }

    let { floating = false, collapsed = false }: Props = $props();
    let configForm = $state<ConfigForm>();
    let configModal = $state<Modal>();

    function showConfig() {
        configForm?.reset();
        configModal?.show();
    }
</script>

<button
    type="button"
    class={floating
        ? "btn btn-secondary fixed right-4 bottom-4 z-40 size-11 rounded-full p-0 shadow-lg"
        : [
              "flex w-full cursor-pointer items-center gap-3 rounded-control px-3 py-2 text-sm font-medium text-ink-muted transition-colors hover:bg-surface-hover hover:text-ink",
              collapsed && "lg:justify-center lg:px-0",
          ]}
    aria-label={m.env_config()}
    title={m.env_config()}
    onclick={showConfig}
>
    <Settings size={18} class="shrink-0" aria-hidden="true" />
    {#if !floating}
        <span class={collapsed ? "lg:hidden" : undefined}>{m.env_config()}</span>
    {/if}
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
