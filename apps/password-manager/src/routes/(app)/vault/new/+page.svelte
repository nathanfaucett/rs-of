<script lang="ts">
import { goto } from "$app/navigation";
import { page } from "$app/state";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import SecretEditor from "$lib/vault/secrets/components/SecretEditor.svelte";
import type { EditableKind } from "$lib/vault/secrets/secret-draft";

let { params } = $props();
let vaultData = $derived(vaultStateModule.vaultData);

// Get kind from URL search params (e.g., /vault/new?kind=login)
let kind = $derived<EditableKind>((page.url.searchParams.get("kind") as EditableKind) ?? "login");

function handleSave() {
    goto("/vault");
}

function handleCancel() {
    goto("/vault");
}
</script>

<div class="flex min-h-0 grow overflow-auto p-6">
    {#if vaultData}
        <div class="w-full max-w-3xl mx-auto">
            <SecretEditor
                vault={vaultData.vault}
                kind={kind}
                record={null}
                onSave={handleSave}
                onCancel={handleCancel}
            />
        </div>
    {:else}
        <p class="text-center text-muted-foreground">Loading vault...</p>
    {/if}
</div>
