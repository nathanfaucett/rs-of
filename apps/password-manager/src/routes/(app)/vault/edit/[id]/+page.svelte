<script lang="ts">
import { goto } from "$app/navigation";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import SecretEditor from "$lib/vault/secrets/components/SecretEditor.svelte";
import PageHeader from "$lib/ui/PageHeader.svelte";
import { ArrowLeft } from "@lucide/svelte";
import type { Secret } from "$lib/vault/secrets/secret";

let { data, params } = $props();
let vaultData = $derived(vaultStateModule.vaultData);
let recordId = $derived<string | null>(data?.id ?? params.id);

function handleSave() {
    goto("/vault");
}

function handleCancel() {
    goto("/vault");
}</script>

<div class="flex min-h-0 grow overflow-auto p-6">
    {#if vaultData}
        <div class="w-full max-w-3xl mx-auto">
            <PageHeader title="Edit secret" description="Changes are encrypted and stored when you save.">
                {#snippet actions()}
                    <button class="btn btn-ghost btn-md" type="button" onclick={handleCancel} aria-label="Back to vault">
                        <ArrowLeft class="w-4 h-4 mr-1" /> Back
                    </button>
                {/snippet}
            </PageHeader>
            {#if recordId}
                <SecretEditor
                    vault={vaultData.vault}
                    recordId={recordId}
                    onSave={handleSave}
                    onCancel={handleCancel}
                />
            {:else}
                <div class="flex items-center justify-center h-64">
                    <p class="text-muted-foreground">Loading secret...</p>
                </div>
            {/if}
        </div>
    {:else}
        <p class="text-center text-muted-foreground">Loading vault...</p>
    {/if}
</div>
