<script lang="ts">
import { ArrowLeft } from "@lucide/svelte";
import { goto } from "$app/navigation";
import { resolve } from "$app/paths";
import { page } from "$app/state";
import { vaultStateModule } from "$lib/shared/state/vault.svelte";
import PageHeader from "$lib/ui/PageHeader.svelte";
import SecretEditor from "$lib/vault/secrets/components/SecretEditor.svelte";
import type { EditableKind } from "$lib/vault/secrets/secret-draft";

let vaultData = $derived(vaultStateModule.vaultData);

// Get kind from URL search params (e.g., /vault/new?kind=login)
let kind = $derived<EditableKind>((page.url.searchParams.get("kind") as EditableKind) ?? "login");

function returnToVault() {
    void goto(resolve("/vault"));
}
</script>

<svelte:head><title>New item | Password Manager</title></svelte:head>

<div class="flex min-h-0 grow overflow-auto px-3 pb-6 pt-16 sm:px-6 lg:pt-6">
    {#if vaultData}
        <div class="w-full max-w-3xl mx-auto">
            <PageHeader title="New item" description="Create a new item in your vault.">
                {#snippet actions()}
                    <button class="btn btn-ghost btn-md" type="button" onclick={returnToVault} aria-label="Back to vault">
                        <ArrowLeft class="w-4 h-4 mr-1" /> Back
                    </button>
                {/snippet}
            </PageHeader>
            {#key kind}
                <SecretEditor vault={vaultData.vault} {kind} onExit={returnToVault} />
            {/key}
        </div>
    {:else}
        <p class="text-center text-muted-foreground">Loading vault...</p>
    {/if}
</div>
