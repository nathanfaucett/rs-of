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

function handleSave() {
    void goto(resolve("/vault"));
}

function handleCancel() {
    void goto(resolve("/vault"));
}</script>

<div class="flex min-h-0 grow overflow-auto p-6">
    {#if vaultData}
        <div class="w-full max-w-3xl mx-auto">
            <PageHeader title="New secret" description="Create a new secret in your vault.">
                {#snippet actions()}
                    <button class="btn btn-ghost btn-md" type="button" onclick={handleCancel} aria-label="Back to vault">
                        <ArrowLeft class="w-4 h-4 mr-1" /> Back
                    </button>
                {/snippet}
            </PageHeader>
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
